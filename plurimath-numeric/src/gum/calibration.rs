use super::statistics::from_count;
use super::*;
use crate::scalar::Scalar;

/// Unweighted least-squares line through centered abscissae `theta`.
struct Ols<S> {
    intercept: S,
    slope: S,
    residuals: Vec<S>,
    variance: S,
    covariance: [[S; 2]; 2],
}

fn sum<S: Scalar>(items: impl Iterator<Item = S>) -> S {
    items.fold(S::s_zero(), |acc, item| acc.add(&item))
}

/// The OLS fit for `n >= 3` points; `None` when the design determinant is
/// not positive.
fn ols<S: Scalar>(theta: &[S], y: &[S]) -> Option<Ols<S>> {
    let n = from_count::<S>(theta.len());
    let sum_theta = sum(theta.iter().cloned());
    let squares = sum(theta.iter().map(|t| t.mul(t)));
    let cross = sum(theta.iter().zip(y).map(|(a, b)| a.mul(b)));
    let total = sum(y.iter().cloned());
    let determinant = squares.mul(&n).sub(&sum_theta.mul(&sum_theta));
    if determinant.s_is_zero() || determinant.s_is_negative() {
        return None;
    }
    let over = |value: S| value.div(&determinant).expect("positive determinant");
    let intercept = over(total.mul(&squares).sub(&cross.mul(&sum_theta)));
    let slope = over(cross.mul(&n).sub(&total.mul(&sum_theta)));
    let residuals: Vec<S> = theta
        .iter()
        .zip(y)
        .map(|(t, response)| response.sub(&intercept).sub(&slope.mul(t)))
        .collect();
    let variance = sum(residuals.iter().map(|r| r.mul(r)))
        .div(&from_count(theta.len() - 2))
        .expect("at least three points");
    let off = over(variance.mul(&sum_theta)).negate();
    let covariance = [
        [over(variance.mul(&squares)), off.clone()],
        [off, over(variance.mul(&n))],
    ];
    Some(Ols {
        intercept,
        slope,
        residuals,
        variance,
        covariance,
    })
}

/// `u^2(a + b delta)` from the fit covariance, plus the residual variance for
/// a new observation.
fn prediction_variance<S: Scalar>(covariance: &[Vec<S>], delta: &S, residual: Option<&S>) -> S {
    let two = from_count::<S>(2);
    let variance = covariance[0][0]
        .add(&delta.mul(&two).mul(&covariance[0][1]))
        .add(&delta.mul(delta).mul(&covariance[1][1]));
    match residual {
        Some(residual) => variance.add(residual),
        None => variance,
    }
}

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
    let Ols {
        intercept,
        slope,
        residuals,
        variance,
        covariance,
    } = ols(&theta, &y).ok_or_else(|| invalid("calibration design is singular"))?;
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
    let residual = if target == "new_observation" {
        Some(exact(&calibration["residual_variance"])?)
    } else {
        None
    };
    let variance = prediction_variance(&covariance, &delta, residual.as_ref());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i64, d: i64) -> Q {
        Q::new(n.into(), d.into())
    }

    /// The same OLS kernel runs on `Q` and `f64`.
    #[test]
    fn ols_agrees_for_q_and_f64() {
        let theta = [q(-1, 1), q(0, 1), q(1, 1)];
        let y = [q(1, 1), q(2, 1), q(4, 1)];
        let exact = ols(&theta, &y).expect("regular design");
        assert_eq!(exact.intercept, q(7, 3));
        assert_eq!(exact.slope, q(3, 2));
        assert_eq!(exact.variance, q(1, 6));
        assert_eq!(exact.covariance[0][1], q(0, 1));
        let binary = ols(&[-1.0, 0.0, 1.0], &[1.0, 2.0, 4.0]).expect("regular design");
        assert!((binary.intercept - 7.0 / 3.0).abs() < 1e-12);
        assert!((binary.slope - 1.5).abs() < 1e-12);
        assert!((binary.variance - 1.0 / 6.0).abs() < 1e-12);
        assert!(ols(&[0.0, 0.0, 0.0], &[1.0, 2.0, 3.0]).is_none());
    }
}
