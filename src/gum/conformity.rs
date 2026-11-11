use super::*;
use num_integer::Integer;
use num_traits::{One, ToPrimitive};

const RULES: &[&str] = &[
    "simple",
    "guarded_acceptance",
    "guarded_rejection",
    "iso14253_style",
    "three_zone",
];

struct Limits {
    lower: Option<Q>,
    upper: Option<Q>,
}

impl Limits {
    fn contains(&self, value: &Q) -> bool {
        self.lower.as_ref().is_none_or(|limit| value >= limit)
            && self.upper.as_ref().is_none_or(|limit| value <= limit)
    }

    fn possible(&self) -> bool {
        !matches!((&self.lower,&self.upper),(Some(a),Some(b)) if a > b)
    }

    fn shift(&self, band: &Q) -> Self {
        Self {
            lower: self.lower.as_ref().map(|lower| lower + band),
            upper: self.upper.as_ref().map(|upper| upper - band),
        }
    }

    fn wire(&self) -> J {
        json!([
            self.lower.as_ref().map(fraction),
            self.upper.as_ref().map(fraction)
        ])
    }

    fn boundary(&self, value: &Q) -> bool {
        self.lower.as_ref() == Some(value) || self.upper.as_ref() == Some(value)
    }

    fn probabilities(&self, mean: &Q, uncertainty: &Q) -> Result<(f64, f64)> {
        let normal = super::coverage::normal_cdf;
        let z_lower = self.lower.as_ref().map_or(f64::NEG_INFINITY, |limit| {
            binary(&((limit - mean) / uncertainty))
        });
        let z_upper = self.upper.as_ref().map_or(f64::INFINITY, |limit| {
            binary(&((limit - mean) / uncertainty))
        });
        let outside = normal(z_lower) + normal(-z_upper);
        let inside = if z_lower > 0.0 {
            normal(-z_lower) - normal(-z_upper)
        } else if z_upper < 0.0 {
            normal(z_upper) - normal(z_lower)
        } else {
            1.0 - outside
        };
        Ok((clamp(inside)?, clamp(outside)?))
    }
}

fn binary(value: &Q) -> f64 {
    value.to_f64().unwrap_or_else(|| {
        if value.is_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    })
}

fn fraction(value: &Q) -> String {
    format!("{}/{}", value.numer(), value.denom())
}

fn clamp(value: f64) -> Result<f64> {
    if !(-1e-12..=1.0 + 1e-12).contains(&value) {
        return Err(Error {
            code: "NumericalFailure",
            message: "normal probability outside numerical range",
        });
    }
    Ok(value.clamp(0.0, 1.0))
}

fn probability(value: f64, digits: u32) -> String {
    let value = Q::from_float(value).expect("checked finite probability");
    let (mut scaled, remainder) = (value.numer() * crate::pow10(digits)).div_rem(value.denom());
    if remainder * 2 >= *value.denom() {
        scaled += 1;
    }
    let mut output = format!("{:0width$}", scaled, width = digits as usize + 1);
    output.insert(output.len() - digits as usize, '.');
    output
}

pub(super) fn evaluate(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "estimate",
            "standard_uncertainty",
            "distribution",
            "knowledge",
            "tolerance",
            "rule",
            "coverage_factor",
            "guard_ratio",
            "guard_digits",
            "risk_scope",
            "consumer_risk_ceiling",
        ],
    )?;
    if symbol_name(&params["distribution"]) != Some("normal")
        || symbol_name(&params["knowledge"]) != Some("measurement_only")
    {
        return Err(unsupported(
            "decision capability requires explicit measurement-only normal knowledge",
        ));
    }
    if params
        .get("risk_scope")
        .is_some_and(|scope| symbol_name(scope) != Some("specific"))
    {
        return Err(unsupported(
            "global risks require a process/measurement joint PDF",
        ));
    }
    let digits = guard(
        &params["guard_digits"],
        12,
        "probabilities support 1..12 decimal places",
    )?;
    let resolution = Q::new(BigInt::one(), crate::pow10(digits));
    let estimate = exact(&params["estimate"])?;
    let uncertainty = exact(&params["standard_uncertainty"])?;
    if !uncertainty.is_positive() {
        return Err(invalid("normal uncertainty must be positive"));
    }
    let tolerance = params["tolerance"]
        .as_array()
        .filter(|limits| limits.len() == 2 && limits.iter().any(|limit| !limit.is_null()))
        .ok_or_else(|| invalid("provide lower/upper tolerance limits; nil denotes an open side"))?;
    let optional = |value: &J| {
        if value.is_null() {
            Ok(None)
        } else {
            exact(value).map(Some)
        }
    };
    let tolerance = Limits {
        lower: optional(&tolerance[0])?,
        upper: optional(&tolerance[1])?,
    };
    if matches!((&tolerance.lower,&tolerance.upper),(Some(a),Some(b)) if a >= b) {
        return Err(invalid("tolerance limits must increase"));
    }
    let rule = symbol_name(&params["rule"])
        .filter(|name| RULES.contains(name))
        .ok_or_else(|| invalid("unknown decision rule"))?;
    let k = exact(&params["coverage_factor"])?;
    let ratio = exact(&params["guard_ratio"])?;
    if !k.is_positive() || ratio.is_negative() {
        return Err(invalid(
            "coverage factor must be positive and guard ratio nonnegative",
        ));
    }
    if rule == "simple" && !ratio.is_zero() {
        return Err(invalid("simple acceptance requires zero guard ratio"));
    }
    if rule == "iso14253_style" && (k != Q::from_integer(number(2)) || ratio != Q::one()) {
        return Err(invalid("JCGM106 ISO14253-style rule requires k=2 and r=1"));
    }
    let expanded = &k * &uncertainty;
    let band = if rule == "guarded_rejection" {
        -(&ratio * &expanded)
    } else {
        &ratio * &expanded
    };
    let acceptance = tolerance.shift(&band);
    let possible = acceptance.possible();
    let ceiling = optional(&params["consumer_risk_ceiling"])?;
    if ceiling
        .as_ref()
        .is_some_and(|value| value <= &resolution || value >= &Q::one())
    {
        return Err(unsupported(
            "risk ceiling must exceed the declared probability resolution and be below one",
        ));
    }
    let (pc, nonconformance) = tolerance.probabilities(&estimate, &uncertainty)?;
    let outer = tolerance.shift(&-&band);
    let decision = if possible && acceptance.contains(&estimate) {
        "accept"
    } else if rule == "three_zone" && outer.contains(&estimate) {
        "conditional"
    } else {
        "reject"
    };
    let maximum_risk = if possible {
        let edge = acceptance
            .lower
            .as_ref()
            .or(acceptance.upper.as_ref())
            .expect("finite tolerance side");
        Some(tolerance.probabilities(edge, &uncertainty)?.1)
    } else {
        None
    };
    if ceiling.as_ref().is_some_and(|value| {
        maximum_risk.is_none_or(|risk| risk + binary(&resolution) > binary(value))
    }) {
        return Err(unsupported(
            "declared guard geometry does not establish the requested consumer risk ceiling",
        ));
    }
    let boundary = tolerance.boundary(&estimate)
        || acceptance.boundary(&estimate)
        || (rule == "three_zone" && outer.boundary(&estimate));
    let fmt = |value| probability(value, digits);
    let capability = tolerance
        .lower
        .as_ref()
        .zip(tolerance.upper.as_ref())
        .map(|(lower, upper)| fraction(&((upper - lower) / (&uncertainty * number(4)))));
    let mut stamp = engine("JCGM106-2012-normal-v1");
    stamp["numeric_backend"] = json!("binary64");
    Ok(json!({
        "decision":symbol(decision),"boundary":boundary,
        "estimate":fraction(&estimate),"standard_uncertainty":fraction(&uncertainty),
        "coverage_factor":fraction(&k),"guard_ratio":fraction(&ratio),
        "required_consumer_risk_ceiling":ceiling.as_ref().map(fraction),
        "conformance_probability":fmt(pc),"nonconformance_probability":fmt(nonconformance),
        "specific_consumer_risk":(decision=="accept").then(||fmt(nonconformance)),
        "specific_producer_risk":(decision=="reject").then(||fmt(pc)),
        "maximum_specific_consumer_risk_if_accepted":maximum_risk.map(fmt),
        "iso_2p3_percent_ceiling_holds":maximum_risk.filter(|_|rule=="iso14253_style").map(|risk|risk<=0.023),
        "acceptance_possible":possible,"tolerance":tolerance.wire(),"acceptance_interval":acceptance.wire(),
        "signed_guard_band":fraction(&band),"expanded_uncertainty":fraction(&expanded),
        "lower_residual":tolerance.lower.as_ref().map(|limit|fraction(&(&estimate-limit))),
        "upper_residual":tolerance.upper.as_ref().map(|limit|fraction(&(limit-&estimate))),
        "measurement_capability_index":capability,"rule":symbol(rule),"distribution":symbol("normal"),
        "knowledge":symbol("measurement_only"),"risk_scope":symbol("specific"),
        "guard_digits":digits,"precision_convention":symbol("decimal_places"),
        "probability_resolution":fraction(&resolution),"engine":stamp,
    }))
}
