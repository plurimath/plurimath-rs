use super::{ast, binding_decimal, keys, name, text, E, J, Q};
use asciiunits::{Error as ParserError, Parser};
use num_traits::{One, ToPrimitive};
use serde_json::json;
use std::{collections::BTreeMap, sync::OnceLock};
use unitsml_rs::{
    combine_kinds, decode_register, rational, terms_from_asciiunits, Error as UnitError, KindError,
    Quantity, QuantityKind, Register, Scale, Unit, WireError,
};

/// A walked value and its optional data-declared quantity kind (F40).
type Kinded = (Quantity, Option<QuantityKind>);

const MAX_UNIT_BYTES: usize = 4096;
const MAX_BINDINGS: usize = 256;
const MAX_EXPONENT: u32 = 4096;
const MAX_POWER_BITS: u64 = 1_000_000;

enum Error {
    Request(E),
    Unit(UnitError),
    Parser(&'static str, J),
    Kind(KindError),
}

impl From<KindError> for Error {
    fn from(error: KindError) -> Self {
        Self::Kind(error)
    }
}

impl From<E> for Error {
    fn from(error: E) -> Self {
        Self::Request(error)
    }
}

impl From<UnitError> for Error {
    fn from(error: UnitError) -> Self {
        Self::Unit(error)
    }
}

impl Error {
    fn wire(self) -> J {
        match self {
            Self::Request(code) => json!({"code":code}),
            Self::Parser(code, details) => json!({"code":code,"parser":details}),
            Self::Kind(error) => json!({"code":error.code(),"detail":error.to_string()}),
            Self::Unit(error) => {
                let code = match error {
                    UnitError::UnsupportedOperand => "UnsupportedUnitExpressionError",
                    UnitError::UnknownUnit(_) => "UnknownUnitError",
                    UnitError::UnknownPrefix(_) => "UnknownPrefixError",
                    UnitError::MissingConversion(_) => "MissingUnitConversionError",
                    UnitError::InvalidRegister(_) => "InvalidUnitRegisterError",
                    UnitError::InconsistentDimensions(_) => "InconsistentUnitDimensionsError",
                    UnitError::UnsupportedScale(_) => "UnsupportedUnitScaleError",
                    UnitError::ScaleOperation { .. } => "ScaleOperationError",
                    UnitError::DimensionMismatch => "DimensionMismatchError",
                    UnitError::AffineComposition => "AffineCompositionError",
                    UnitError::InexactPower => "InexactUnitPowerError",
                    UnitError::InvalidNumber(_) => "InvalidBindingError",
                    UnitError::ResourceLimit => "ResourceLimitError",
                    UnitError::DivisionByZero => "DivisionByZeroError",
                };
                json!({"code":code,"class":error.result_class(),"detail":error.to_string()})
            }
        }
    }
}

fn parser_error(error: &ParserError) -> Error {
    let code = match error {
        ParserError::Syntax { .. } => "UnitSyntaxError",
        ParserError::RecursionLimit { .. } | ParserError::CallDepthLimit { .. } => {
            "ResourceLimitError"
        }
        ParserError::Artifact { .. } | ParserError::Runtime { .. } => "UnitParserRuntimeError",
        ParserError::InvalidArgument { .. } => "InvalidRequest",
    };
    Error::Parser(
        code,
        serde_json::to_value(error).expect("serializable AsciiUnits error"),
    )
}

fn parse_unit(register: &Register, expression: &str) -> Result<Unit, Error> {
    if expression.is_empty() {
        return Err("InvalidRequest".into());
    }
    if expression.len() > MAX_UNIT_BYTES {
        return Err(UnitError::ResourceLimit.into());
    }
    static PARSER: OnceLock<Result<Parser, ParserError>> = OnceLock::new();
    let parser = PARSER
        .get_or_init(Parser::new)
        .as_ref()
        .map_err(parser_error)?;
    let model = parser
        .parse(expression)
        .map_err(|error| parser_error(&error))?;
    Ok(register.resolve(&terms_from_asciiunits(&model)?)?)
}

fn binding(register: &Register, value: &J) -> Result<Kinded, Error> {
    if let Some(scalar) = value.as_str() {
        return Ok((Quantity::scalar(binding_magnitude(scalar)?), None));
    }
    keys(
        value,
        &["value", "unit", "difference", "kind"],
        &["value", "unit"],
    )?;
    let magnitude = binding_magnitude(text(value, "value")?)?;
    let unit = parse_unit(register, text(value, "unit")?)?;
    let difference = value
        .get("difference")
        .map_or(Some(false), J::as_bool)
        .ok_or("InvalidRequest")?;
    let kind = match value.get("kind") {
        None | Some(J::Null) => None,
        Some(kind) => Some(binding_kind(&unit, kind.as_str().ok_or("InvalidRequest")?)?),
    };
    let quantity = if difference {
        Quantity::difference(magnitude, unit)
    } else {
        Quantity::new(magnitude, unit)
    };
    Ok((quantity, kind))
}

/// `kind` is a UnitsDB quantity identifier or name, or the reserved token
/// `"primary"`: the flagged primary quantity of a single unit at power 1
/// (unitsml-ruby `kind: :primary`). The kind's dimension must match the unit's.
fn binding_kind(unit: &Unit, kind: &str) -> Result<QuantityKind, Error> {
    let resolved = if kind == "primary" {
        let single = match unit.terms() {
            [term] if term.power.is_one() => Some(term.unit.as_str()),
            _ => None,
        };
        single
            .map(QuantityKind::primary_for_unit)
            .transpose()?
            .flatten()
            .ok_or_else(|| KindError::Unknown("primary".to_owned()))?
    } else {
        QuantityKind::resolve(kind)?
    };
    resolved.check_dimension(unit.dimension_id())?;
    Ok(resolved)
}

fn binding_magnitude(value: &str) -> Result<Q, Error> {
    if !binding_decimal(value) {
        return Err("InvalidBindingError".into());
    }
    Ok(rational(value)?)
}

fn power(base: Quantity, exponent: &Quantity) -> Result<Quantity, Error> {
    if !exponent.unit().dimensions().is_empty() {
        return Err("UnsupportedExpressionError".into());
    }
    let exponent = exponent.canonical_magnitude();
    if !exponent.is_integer() {
        return Err("UnsupportedExpressionError".into());
    }
    let n = exponent
        .to_integer()
        .to_i32()
        .filter(|n| n.unsigned_abs() <= MAX_EXPONENT)
        .ok_or(UnitError::ResourceLimit)?;
    if base.scale() == Scale::Interval {
        return Err(UnitError::ScaleOperation {
            scale: base.scale(),
            operation: "power",
        }
        .into());
    }
    let bits = |value: &Q| value.numer().bits().max(value.denom().bits());
    let width = bits(base.magnitude()).max(bits(base.unit().factor()));
    if width.saturating_mul(u64::from(n.unsigned_abs())) > MAX_POWER_BITS {
        return Err(UnitError::ResourceLimit.into());
    }
    let mut result = Quantity::scalar(Q::one());
    let mut base = base;
    let mut remaining = n.unsigned_abs();
    while remaining != 0 {
        if remaining & 1 != 0 {
            result = result.multiply(&base)?;
        }
        remaining >>= 1;
        if remaining != 0 {
            base = base.multiply(&base)?;
        }
    }
    if n < 0 {
        Ok(Quantity::scalar(Q::one()).divide(&result)?)
    } else {
        Ok(result)
    }
}

/// Kinds follow unitsml-ruby: `+`/`-` refuse incompatible kinds and keep
/// `left || right`; negation keeps the kind; `*`, `/` and `^` drop it.
fn walk(node: &J, bindings: &BTreeMap<String, Kinded>) -> Result<Kinded, Error> {
    match text(node, "type")? {
        "number" => Ok((Quantity::scalar(rational(text(node, "text")?)?), None)),
        "variable" => bindings
            .get(text(node, "name")?)
            .cloned()
            .ok_or("MissingVariableError".into()),
        "unary" => {
            let (operand, kind) = walk(&node["operand"], bindings)?;
            if node["operator"] == "-" {
                Ok((operand.scale_by(&-Q::one())?, kind))
            } else {
                Ok((operand, kind))
            }
        }
        "binary" => {
            let (left, left_kind) = walk(&node["left"], bindings)?;
            let (right, right_kind) = walk(&node["right"], bindings)?;
            match text(node, "operator")? {
                "+" => {
                    let kind = combine_kinds(left_kind.as_ref(), right_kind.as_ref())?;
                    Ok((left.add(&right)?, kind))
                }
                "-" => {
                    let kind = combine_kinds(left_kind.as_ref(), right_kind.as_ref())?;
                    Ok((left.subtract(&right)?, kind))
                }
                "*" => Ok((left.multiply(&right)?, None)),
                "/" => Ok((left.divide(&right)?, None)),
                "^" => Ok((power(left, &right)?, None)),
                _ => Err("InvalidRequest".into()),
            }
        }
        "call" => Err("UnsupportedExpressionError".into()),
        _ => Err("InvalidRequest".into()),
    }
}

fn snapshot((quantity, kind): (&Quantity, Option<&QuantityKind>)) -> J {
    let unit = quantity.unit();
    let dimensions: BTreeMap<_, _> = unit
        .dimensions()
        .iter()
        .map(|(dimension, exponent)| (dimension, exponent.to_string()))
        .collect();
    let terms: Vec<_> = unit
        .terms()
        .iter()
        .map(|term| json!({"unit":term.unit,"prefix":term.prefix,"power":term.power.to_string()}))
        .collect();
    let scale = match quantity.scale() {
        Scale::Ratio => "ratio",
        Scale::Interval => "interval",
        Scale::Difference => "difference",
    };
    let mut wire = json!({
        "magnitude": quantity.magnitude().to_string(),
        "canonical_magnitude": quantity.canonical_magnitude().to_string(),
        "dimensions": dimensions,
        "dimension_id": unit.dimension_id(),
        "scale": scale,
        "terms": terms,
    });
    // Only kinded results carry the field, so untyped wires are unchanged.
    if let Some(kind) = kind {
        wire["kind"] = json!(kind.id());
    }
    wire
}

fn evaluate(request: &J) -> Result<J, Error> {
    keys(
        request,
        &[
            "schema",
            "ast",
            "bindings",
            "mode",
            "convert_to",
            "register",
        ],
        &["schema", "ast"],
    )?;
    if request.get("mode").is_some_and(|mode| mode != "strict") {
        return Err("InvalidRequest".into());
    }
    ast(&request["ast"], 0)?;
    let register = decode_register(request.get("register")).map_err(|error| match error {
        WireError::Algebra(error) => Error::Unit(error),
        WireError::Invalid(_) => Error::Request("InvalidRequest"),
    })?;
    let mut bindings = BTreeMap::new();
    if let Some(raw) = request.get("bindings") {
        let raw = raw.as_object().ok_or("InvalidRequest")?;
        if raw.len() > MAX_BINDINGS {
            return Err(UnitError::ResourceLimit.into());
        }
        for (key, value) in raw {
            if !name(key) {
                return Err("InvalidRequest".into());
            }
            bindings.insert(key.clone(), binding(&register, value)?);
        }
    }
    let (value, kind) = walk(&request["ast"], &bindings)?;
    let converted = request
        .get("convert_to")
        .map(|target| {
            let target = target.as_str().ok_or("InvalidRequest")?;
            Ok::<_, Error>(value.convert(&parse_unit(&register, target)?)?)
        })
        .transpose()?;
    Ok(json!({
        "value": snapshot((&value, kind.as_ref())),
        "converted": converted.as_ref().map(|c| snapshot((c, kind.as_ref()))),
        "accuracy": {"symbol":"exact"},
    }))
}

pub(crate) fn run(request: &J) -> Result<J, J> {
    evaluate(request).map_err(Error::wire)
}
