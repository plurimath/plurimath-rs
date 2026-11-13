use super::*;

fn fetch<'a>(value: &'a J, field: &str) -> Result<&'a J> {
    value.get(field).ok_or_else(|| {
        invalid(match field {
            "probability" => "missing validation result field: probability",
            "numerical_tolerance" => "missing validation result field: numerical_tolerance",
            "intervals" => "missing validation result field: intervals",
            "shortest" => "missing validation result field: shortest",
            _ => "missing validation result field: symmetric",
        })
    })
}

fn language(code: &'static str, message: &'static str) -> Error {
    Error { code, message }
}

// Preserve the reference method's failure class at its interval-map lookup.
fn bounds<'a>(value: &'a J, kind: &str) -> Result<&'a J> {
    if value.is_object() {
        return fetch(value, kind);
    }
    if value.is_array() {
        return Err(language(
            "TypeError",
            "no implicit conversion of Symbol into Integer",
        ));
    }
    Err(language(
        "NoMethodError",
        match value {
            J::Null => "undefined method 'fetch' for nil",
            J::Bool(false) => "undefined method 'fetch' for false",
            J::Bool(true) => "undefined method 'fetch' for true",
            J::String(_) => "undefined method 'fetch' for an instance of String",
            J::Number(n) if n.is_i64() || n.is_u64() => {
                "undefined method 'fetch' for an instance of Integer"
            }
            _ => "undefined method 'fetch' for an instance of Float",
        },
    ))
}

fn endpoints(value: &J) -> Result<[Q; 2]> {
    if let Some(array) = value.as_array() {
        // Array#zip uses two entries and supplies nil for missing entries.
        return Ok([
            exact(array.first().unwrap_or(&J::Null))?,
            exact(array.get(1).unwrap_or(&J::Null))?,
        ]);
    }
    if value.is_object() {
        // Hash#each yields pairs, which the reference exact-number guard refuses.
        return Err(invalid("invalid exact numeric input"));
    }
    Err(language(
        "TypeError",
        match value {
            J::Null => "wrong argument type NilClass (must respond to :each)",
            J::Bool(false) => "wrong argument type FalseClass (must respond to :each)",
            J::Bool(true) => "wrong argument type TrueClass (must respond to :each)",
            J::String(_) => "wrong argument type String (must respond to :each)",
            J::Number(n) if n.is_i64() || n.is_u64() => {
                "wrong argument type Integer (must respond to :each)"
            }
            _ => "wrong argument type Float (must respond to :each)",
        },
    ))
}

fn fraction(value: &Q) -> String {
    format!("{}/{}", value.numer(), value.denom())
}

pub(super) fn evaluate(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "guf_interval",
            "guf_standard_uncertainty",
            "significant_digits",
            "probability",
            "interval",
            "monte_carlo",
        ],
    )?;
    let supplied = params["guf_interval"].as_array();
    let digits = params["significant_digits"].as_u64().filter(|n| *n > 0);
    if supplied.is_none_or(|v| v.len() != 2) || digits.is_none() {
        return Err(invalid(
            "validation needs two endpoints and positive significant_digits",
        ));
    }
    let kind = symbol_name(&params["interval"]).filter(|s| ["symmetric", "shortest"].contains(s));
    let mc = &params["monte_carlo"];
    if kind.is_none()
        || !mc.is_object()
        || mc["stabilized"] != true
        || symbol_name(&mc["method"]) != Some("adaptive_mcm")
    {
        return Err(unsupported(
            "validation requires stabilized adaptive MCM of the requested interval kind",
        ));
    }
    let probability = exact(&params["probability"])?;
    if probability != exact(fetch(mc, "probability")?)? {
        return Err(invalid("coverage probability mismatch"));
    }
    let target = super::monte_carlo::exact_tolerance(
        &exact(&params["guf_standard_uncertainty"])?,
        digits.expect("positive digits"),
    )?;
    let mc_tolerance = exact(fetch(mc, "numerical_tolerance")?)?;
    if mc_tolerance > &target / number(5) {
        return Err(unsupported(
            "MCM tolerance must be at least five times tighter for validation",
        ));
    }
    let kind = kind.expect("checked interval kind");
    let raw_bounds = bounds(fetch(mc, "intervals")?, kind)?;
    let supplied = supplied.expect("two endpoints");
    let left = exact(&supplied[0])?;
    let right = exact(&supplied[1])?;
    if left >= right {
        return Err(invalid("GUF interval endpoints must increase"));
    }
    let actual = endpoints(raw_bounds)?;
    let differences = [(&left - &actual[0]).abs(), (&right - &actual[1]).abs()];
    Ok(json!({
        "adequacy":symbol(if differences.iter().all(|d| d <= &target) {"validated"} else {"not_validated"}),
        "probability":fraction(&probability),"interval":symbol(kind),"tolerance":fraction(&target),
        "monte_carlo_tolerance":fraction(&mc_tolerance),"endpoint_differences":differences.iter().map(fraction).collect::<Vec<_>>(),
        "engine":engine("JCGM101-2008-8.1-8.2-v1"),
    }))
}
