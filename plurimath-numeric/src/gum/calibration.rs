use super::*;

pub(super) fn fit(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "temperatures",
            "corrections",
            "reference_temperature",
            "abscissa_uncertainty",
            "residual_model",
            "guard_digits",
        ],
    )?;
    if symbol_name(&params["abscissa_uncertainty"]) != Some("negligible")
        || symbol_name(&params["residual_model"]) != Some("independent_common_variance")
    {
        return Err(unsupported("calibration requires negligible abscissa uncertainty and independent common-variance residuals"));
    }
    let shape = "linear calibration needs at least three paired observations";
    let x = vector(&params["temperatures"], shape)?;
    let y = vector(&params["corrections"], shape)?;
    let n = x.len();
    if n < 3 || n != y.len() {
        return Err(invalid(shape));
    }
    let digits = guard(
        &params["guard_digits"],
        1000,
        "guard_digits must be positive",
    )?;
    let t0 = exact(&params["reference_temperature"])?;
    let theta: Vec<_> = x.iter().map(|t| t - &t0).collect();
    let sum = theta.iter().sum::<Q>();
    let squares = theta.iter().map(|t| t.pow(2)).sum::<Q>();
    let cross = theta.iter().zip(&y).map(|(a, b)| a * b).sum::<Q>();
    let total = y.iter().sum::<Q>();
    let determinant = &squares * number(n) - sum.pow(2);
    if !determinant.is_positive() {
        return Err(invalid("calibration design is singular"));
    }
    let intercept = (&total * &squares - &cross * &sum) / &determinant;
    let slope = (&cross * number(n) - &total * &sum) / &determinant;
    let residuals: Vec<_> = theta
        .iter()
        .zip(y)
        .map(|(t, response)| response - &intercept - &slope * t)
        .collect();
    let variance = residuals.iter().map(|r| r.pow(2)).sum::<Q>() / number(n - 2);
    let covariance = [
        [
            &variance * &squares / &determinant,
            -(&variance * &sum / &determinant),
        ],
        [
            -(&variance * &sum / &determinant),
            &variance * number(n) / &determinant,
        ],
    ];
    Ok(json!({
        "intercept":tagged(&intercept),"slope":tagged(&slope),
        "covariance":covariance.iter().map(|row|row.iter().map(tagged).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "reference_temperature":tagged(&t0),"temperature_range":[tagged(x.iter().min().expect("three points")),tagged(x.iter().max().expect("three points"))],
        "count":n,"degrees_of_freedom":n-2,"residuals":residuals.iter().map(tagged).collect::<Vec<_>>(),
        "residual_variance":tagged(&variance),"residual_standard_deviation":decimal_payload(root_decimal(&variance,digits)?),
        "abscissa_uncertainty":symbol("negligible"),"residual_model":symbol("independent_common_variance"),
        "residual_distribution":symbol("unspecified"),"adequacy":symbol("not_assessed"),"guard_digits":digits,
        "engine":engine("GUM100-H3-unweighted-OLS-v1"),
    }))
}

pub(super) fn predict(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "calibration",
            "temperature",
            "target",
            "temperature_uncertainty",
            "guard_digits",
        ],
    )?;
    let calibration = &params["calibration"];
    if !calibration.is_object()
        || calibration.pointer("/engine/method").and_then(J::as_str)
            != Some("GUM100-H3-unweighted-OLS-v1")
    {
        return Err(invalid("provide a linear calibration result"));
    }
    let target = symbol_name(&params["target"])
        .filter(|target| ["mean_correction", "new_observation"].contains(target));
    if target.is_none() || symbol_name(&params["temperature_uncertainty"]) != Some("negligible") {
        return Err(unsupported(
            "prediction requires an explicit target and negligible temperature uncertainty",
        ));
    }
    let target = target.expect("validated target");
    let digits = guard(
        &params["guard_digits"],
        1000,
        "guard_digits must be positive",
    )?;
    let temperature = exact(&params["temperature"])?;
    let delta = &temperature - exact(&calibration["reference_temperature"])?;
    let estimate = exact(&calibration["intercept"])? + exact(&calibration["slope"])? * &delta;
    let covariance = matrix(
        &calibration["covariance"],
        "provide a linear calibration result",
        2,
    )?;
    let range = vector(
        &calibration["temperature_range"],
        "provide a linear calibration result",
    )?;
    if covariance.len() != 2 || covariance[0].len() != 2 || range.len() != 2 {
        return Err(invalid("provide a linear calibration result"));
    }
    let mut variance = &covariance[0][0]
        + &delta * number(2) * &covariance[0][1]
        + delta.pow(2) * &covariance[1][1];
    if target == "new_observation" {
        variance += exact(&calibration["residual_variance"])?;
    }
    if variance.is_negative() {
        return Err(invalid("negative prediction variance"));
    }
    let degrees = calibration["degrees_of_freedom"]
        .as_u64()
        .ok_or_else(|| invalid("provide a linear calibration result"))?;
    Ok(json!({
        "estimate":tagged(&estimate),"variance":tagged(&variance),
        "standard_uncertainty":decimal_payload(root_decimal(&variance,digits)?),
        "target":symbol(target),"outside_calibration_range":temperature<range[0] || temperature>range[1],
        "degrees_of_freedom":degrees,"adequacy":symbol("not_assessed"),"guard_digits":digits,
        "engine":engine("GUM100-H3-calibration-prediction-v1"),
    }))
}
