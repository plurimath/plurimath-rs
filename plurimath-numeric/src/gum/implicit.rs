use super::*;

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
    let mut augmented: Vec<Vec<Q>> = cy
        .into_iter()
        .zip(cx)
        .map(|(mut output, input)| {
            output.extend(input.into_iter().map(|value| -value));
            output
        })
        .collect();
    for column in 0..n {
        let pivot = (column..n)
            .find(|&row| !augmented[row][column].is_zero())
            .ok_or_else(|| unsupported("implicit output Jacobian is singular"))?;
        augmented.swap(column, pivot);
        let divisor = augmented[column][column].clone();
        augmented[column]
            .iter_mut()
            .for_each(|value| *value /= &divisor);
        let pivot_row = augmented[column].clone();
        for (row, values) in augmented.iter_mut().enumerate() {
            if row == column {
                continue;
            }
            let factor = values[column].clone();
            for (value, pivot) in values.iter_mut().zip(&pivot_row) {
                *value -= &factor * pivot;
            }
        }
    }
    let sensitivities: Vec<_> = augmented
        .into_iter()
        .map(|row| row[n..].iter().map(tagged).collect::<Vec<_>>())
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
