use super::*;
use crate::scalar::Scalar;

/// `-C_Y^{-1} C_X` by Gauss-Jordan elimination on `[C_Y | -C_X]`, pivoting
/// on the first nonzero entry of each column. `None` when `C_Y` is singular.
fn sensitivities<S: Scalar>(cy: Vec<Vec<S>>, cx: Vec<Vec<S>>) -> Option<Vec<Vec<S>>> {
    let n = cy.len();
    let mut augmented: Vec<Vec<S>> = cy
        .into_iter()
        .zip(cx)
        .map(|(mut output, input)| {
            output.extend(input.iter().map(S::negate));
            output
        })
        .collect();
    for column in 0..n {
        let pivot = (column..n).find(|&row| !augmented[row][column].s_is_zero())?;
        augmented.swap(column, pivot);
        let divisor = augmented[column][column].clone();
        augmented[column]
            .iter_mut()
            .for_each(|value| *value = value.div(&divisor).expect("nonzero pivot"));
        let pivot_row = augmented[column].clone();
        for (row, values) in augmented.iter_mut().enumerate() {
            if row == column {
                continue;
            }
            let factor = values[column].clone();
            for (value, pivot) in values.iter_mut().zip(&pivot_row) {
                *value = value.sub(&factor.mul(pivot));
            }
        }
    }
    Some(
        augmented
            .into_iter()
            .map(|mut row| row.split_off(n))
            .collect(),
    )
}

pub(super) fn propagate(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "estimates",
            "output_jacobian",
            "input_jacobian",
            "covariance",
            "residuals",
            "residual_tolerance",
            "guard_digits",
        ],
    )?;
    let estimates = vector(
        &params["estimates"],
        "implicit model needs output estimates",
    )?;
    let n = estimates.len();
    if n == 0 || n > 64 {
        return Err(invalid("implicit model needs output estimates"));
    }
    let cy = matrix(
        &params["output_jacobian"],
        "implicit Jacobian shape mismatch",
        n,
    )?;
    let covariance = params["covariance"]
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 64)
        .ok_or_else(|| invalid("implicit model needs input covariance"))?;
    let m = covariance.len();
    let cx = matrix(
        &params["input_jacobian"],
        "implicit Jacobian shape mismatch",
        n,
    )?;
    if cy.len() != n || cy[0].len() != n || cx.len() != n || cx[0].len() != m {
        return Err(invalid("implicit Jacobian shape mismatch"));
    }
    let tolerance = exact(&params["residual_tolerance"])?;
    let residuals = vector(
        &params["residuals"],
        "provide residuals and a nonnegative residual tolerance",
    )?;
    if tolerance.is_negative() || residuals.len() != n {
        return Err(invalid(
            "provide residuals and a nonnegative residual tolerance",
        ));
    }
    if residuals.iter().any(|value| value.abs() > tolerance) {
        return Err(unsupported(
            "supplied root residual exceeds declared tolerance",
        ));
    }
    let solved =
        sensitivities(cy, cx).ok_or_else(|| unsupported("implicit output Jacobian is singular"))?;
    let sensitivities: Vec<_> = solved
        .iter()
        .map(|row| row.iter().map(tagged).collect::<Vec<_>>())
        .collect();
    let mut result = super::propagation::transform(&json!({
        "estimates":params["estimates"],"jacobian":sensitivities,
        "covariance":params["covariance"],"guard_digits":params["guard_digits"],
    }))?;
    // Ruby BigDecimal's fixed representation retains `.0` for exact integers.
    // Keep this new method's wire policy local; existing vector wires are sealed.
    if let Some(values) = result["standard_uncertainties"].as_array_mut() {
        for value in values {
            if let Some(text) = value["decimal"].as_str() {
                *value = decimal_payload(text.to_owned());
            }
        }
    }
    result["input_sensitivities"] = json!(sensitivities);
    result["root_residuals"] = json!(residuals.iter().map(tagged).collect::<Vec<_>>());
    result["residual_tolerance"] = tagged(&tolerance);
    result["root_solver"] = symbol("caller_supplied");
    result["engine"] = engine("implicit-Jacobian-LPU-v1");
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same elimination runs on `Q` and `f64`.
    #[test]
    fn sensitivities_agree_for_q_and_f64() {
        let int = |n: i64| Q::from_integer(n.into());
        let exact = sensitivities(vec![vec![int(2)]], vec![vec![int(4), int(1)]]);
        assert_eq!(
            exact,
            Some(vec![vec![int(-2), Q::new((-1).into(), 2.into())]])
        );
        let binary = sensitivities(vec![vec![2.0]], vec![vec![4.0, 1.0]]);
        assert_eq!(binary, Some(vec![vec![-2.0, -0.5]]));
        assert_eq!(sensitivities(vec![vec![0.0]], vec![vec![1.0]]), None);
    }
}
