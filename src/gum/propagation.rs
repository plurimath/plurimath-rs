use super::*;

const COMPONENT_FIELDS: &[&str] = &[
    "distribution",
    "half_width",
    "standard_uncertainty",
    "scale",
    "degrees_of_freedom",
    "boundary_half_width",
    "pdf_degrees_of_freedom",
    "variance_degrees_of_freedom",
];

pub(super) fn component(params: &J) -> Result<J> {
    fields(params, COMPONENT_FIELDS)?;
    let distribution = symbol_name(&params["distribution"])
        .ok_or_else(|| unsupported("unsupported component distribution"))?;
    let degrees = &params["degrees_of_freedom"];
    let pdf_degrees = &params["pdf_degrees_of_freedom"];
    let variance_degrees = params.get("variance_degrees_of_freedom");
    if distribution != "student_t" {
        if !pdf_degrees.is_null() {
            return Err(invalid("PDF degrees of freedom apply only to Student-t"));
        }
        if !degrees.is_null() && variance_degrees.is_some() {
            return Err(invalid(
                "provide only one variance degrees-of-freedom argument",
            ));
        }
    }
    let mut student_nu = None;
    let variance = match distribution {
        "rectangular" | "triangular" | "arcsine" => {
            let width = exact(&params["half_width"])?;
            if width.is_negative() {
                return Err(invalid("half-width must be nonnegative"));
            }
            let divisor = match distribution {
                "rectangular" => 3,
                "triangular" => 6,
                _ => 2,
            };
            width.pow(2) / number(divisor)
        }
        "curvilinear_trapezoid" => {
            let width = exact(&params["half_width"])?;
            let boundary = exact(&params["boundary_half_width"])?;
            if !boundary.is_positive() || width <= boundary {
                return Err(invalid(
                    "curvilinear trapezoid needs 0 < boundary half-width < nominal half-width",
                ));
            }
            width.pow(2) / number(3) + boundary.pow(2) / number(9)
        }
        "normal" => {
            let uncertainty = exact(&params["standard_uncertainty"])?;
            if uncertainty.is_negative() {
                return Err(invalid("uncertainty must be nonnegative"));
            }
            uncertainty.pow(2)
        }
        "student_t" => {
            let scale = exact(&params["scale"])?;
            if !pdf_degrees.is_null() && !degrees.is_null() {
                return Err(invalid(
                    "use variance_degrees_of_freedom separately from Student-t PDF degrees",
                ));
            }
            let nu = exact(if pdf_degrees.is_null() {
                degrees
            } else {
                pdf_degrees
            })?;
            let difference = &nu - Q::from_integer(number(2));
            if scale.is_negative() || !difference.is_positive() {
                return Err(invalid("Student-t needs nonnegative scale and nu > 2"));
            }
            let value = scale.pow(2) * &nu / difference;
            student_nu = Some(nu);
            value
        }
        _ => return Err(unsupported("unsupported component distribution")),
    };
    let mut dof = variance_degrees.unwrap_or(degrees).clone();
    if student_nu.is_some() && variance_degrees.is_none() {
        dof = symbol("unknown");
    }
    if symbol_name(&dof) == Some("unknown") {
        dof = symbol("unknown");
    } else if !dof.is_null() && !exact(&dof)?.is_positive() {
        return Err(invalid(
            "variance degrees of freedom must be positive, nil or unknown",
        ));
    }
    let mut result = json!({"variance":tagged(&variance),"distribution":symbol(distribution),"degrees_of_freedom":dof});
    if let Some(nu) = student_nu {
        result["pdf_degrees_of_freedom"] = tagged(&nu);
    }
    Ok(result)
}

fn order(params: &J) -> Result<u8> {
    match params.get("order").map_or(Some(1), J::as_u64) {
        Some(1) => Ok(1),
        Some(2) => Ok(2),
        _ => Err(invalid("order must be 1 or 2")),
    }
}

type Hessian = Vec<Vec<Q>>;
type Third = Vec<Vec<Vec<Q>>>;

fn tensors(params: &J, size: usize, incomplete: &'static str) -> Result<(Hessian, Third)> {
    if params["derivatives_complete"] != true
        || params["hessian"].is_null()
        || params["third"].is_null()
    {
        return Err(unsupported(incomplete));
    }
    let shape = || invalid("derivative tensor shape mismatch");
    fn layer(value: &J, size: usize) -> Result<&[J]> {
        value
            .as_array()
            .filter(|items| items.len() == size)
            .map(Vec::as_slice)
            .ok_or_else(|| invalid("derivative tensor shape mismatch"))
    }
    if size > 16 {
        return Err(shape());
    }
    let row = |value: &J| {
        layer(value, size)?
            .iter()
            .map(exact)
            .collect::<Result<Vec<Q>>>()
    };
    let matrix = |value: &J| {
        layer(value, size)?
            .iter()
            .map(row)
            .collect::<Result<Hessian>>()
    };
    let h = matrix(&params["hessian"])?;
    let t = layer(&params["third"], size)?
        .iter()
        .map(matrix)
        .collect::<Result<Third>>()?;
    for i in 0..size {
        for j in 0..size {
            if h[i][j] != h[j][i] {
                return Err(invalid("Hessian must be symmetric"));
            }
            for k in 0..size {
                if t[i][j][k] != t[j][i][k] || t[i][j][k] != t[i][k][j] {
                    return Err(invalid("third derivative tensor must be symmetric"));
                }
            }
        }
    }
    Ok((h, t))
}

fn normal_components(components: &[J]) -> bool {
    components
        .iter()
        .all(|item| symbol_name(&item["distribution"]) == Some("normal"))
}

fn components<'a>(params: &'a J, mismatch: &'static str) -> Result<(&'a [J], Vec<Q>, Vec<Q>)> {
    let components = params["components"]
        .as_array()
        .filter(|entries| {
            entries.len() <= 64 && entries.iter().all(|v| v.get("variance").is_some())
        })
        .ok_or_else(|| invalid(mismatch))?;
    let gradient = vector(&params["gradient"], mismatch)?;
    if gradient.len() != components.len() {
        return Err(invalid(mismatch));
    }
    let variances = components
        .iter()
        .map(|entry| {
            let variance = exact(&entry["variance"])?;
            if variance.is_negative() {
                return Err(invalid("negative variance"));
            }
            Ok(variance)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((components, gradient, variances))
}

pub(super) fn independent(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "estimate",
            "components",
            "gradient",
            "independent",
            "guard_digits",
            "order",
            "hessian",
            "third",
            "derivatives_complete",
        ],
    )?;
    if params["independent"] != true {
        return Err(unsupported("joint dependence is unsupported"));
    }
    let order = order(params)?;
    let digits = guard(
        &params["guard_digits"],
        1000,
        "guard_digits must be positive",
    )?;
    let (components, gradient, variances) = components(params, "gradient shape mismatch")?;
    let contributions: Vec<_> = gradient
        .iter()
        .zip(&variances)
        .map(|(g, v)| g.pow(2) * v)
        .collect();
    let mut variance = contributions.iter().sum::<Q>();
    let mut estimate = exact(&params["estimate"])?;
    if order == 2 {
        if !normal_components(components) {
            return Err(unsupported(
                "GUF2 requires normal inputs; use distribution propagation",
            ));
        }
        let n = components.len();
        let (h, t) = tensors(
            params,
            n,
            "GUF2 requires complete analytic Hessian and third derivatives",
        )?;
        for i in 0..n {
            estimate += &h[i][i] * &variances[i] / number(2);
            for j in 0..n {
                variance += (h[i][j].pow(2) / number(2) + &gradient[i] * &t[i][j][j])
                    * &variances[i]
                    * &variances[j];
            }
        }
        if variance.is_negative() {
            return Err(unsupported("Taylor approximation gives negative variance"));
        }
        return Ok(json!({
            "estimate":tagged(&estimate),"variance":tagged(&variance),
            "standard_uncertainty":{"decimal":root_decimal(&variance,digits)?},
            "guard_digits":digits,"method":symbol("guf2"),"adequacy":symbol("not_assessed"),
            "engine":engine("independent-gum-v1"),
        }));
    }
    let unknown = components
        .iter()
        .zip(&contributions)
        .any(|(component, contribution)| {
            !contribution.is_zero()
                && symbol_name(&component["degrees_of_freedom"]) == Some("unknown")
        });
    let mut denominator = Q::zero();
    if !unknown {
        for (component, contribution) in components.iter().zip(&contributions) {
            let degrees = &component["degrees_of_freedom"];
            if contribution.is_zero() || degrees.is_null() {
                continue;
            }
            let degrees = exact(degrees)?;
            if !degrees.is_positive() {
                return Err(invalid("degrees of freedom must be positive"));
            }
            denominator += contribution.pow(2) / degrees;
        }
    }
    let (effective, status) = if variance.is_zero() {
        (J::Null, "not_applicable")
    } else if unknown {
        (J::Null, "unknown")
    } else if denominator.is_zero() {
        (J::Null, "infinite")
    } else {
        (tagged(&(variance.pow(2) / denominator)), "finite")
    };
    Ok(json!({
        "estimate":tagged(&estimate),"variance":tagged(&variance),
        "standard_uncertainty":{"decimal":root_decimal(&variance,digits)?},
        "guard_digits":digits,"method":symbol("lpu"),"adequacy":symbol("not_assessed"),
        "engine":engine("independent-gum-v1"),"effective_degrees_of_freedom":effective,
        "degrees_of_freedom_status":symbol(status),
    }))
}

fn checked(value: &J) -> Result<Vec<Vec<Q>>> {
    let invalid_shape = "covariance must be a nonempty square matrix";
    let matrix = matrix(value, invalid_shape, 1)?;
    let n = matrix.len();
    if matrix[0].len() != n {
        return Err(invalid(invalid_shape));
    }
    for i in 0..n {
        if (0..i).any(|j| matrix[i][j] != matrix[j][i]) {
            return Err(invalid("covariance must be symmetric"));
        }
    }
    let mut remainder = matrix.clone();
    let not_psd = || invalid("covariance is not positive semidefinite");
    for k in 0..n {
        let pivot = remainder[k][k].clone();
        if pivot.is_negative() {
            return Err(not_psd());
        }
        if pivot.is_zero() {
            if (k + 1..n).any(|i| !remainder[i][k].is_zero()) {
                return Err(not_psd());
            }
            continue;
        }
        for i in k + 1..n {
            for j in i..n {
                let decrement = &remainder[i][k] * &remainder[j][k] / &pivot;
                remainder[i][j] -= decrement;
                remainder[j][i] = remainder[i][j].clone();
            }
        }
    }
    Ok(matrix)
}

fn product(row: &[Q], matrix: &[Vec<Q>]) -> Vec<Q> {
    (0..row.len())
        .map(|j| row.iter().zip(matrix).map(|(a, m)| a * &m[j]).sum())
        .collect()
}

fn dot(a: &[Q], b: &[Q]) -> Q {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

pub(super) fn correlated(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "estimate",
            "components",
            "gradient",
            "covariance",
            "joint_distribution",
            "guard_digits",
            "order",
            "hessian",
            "third",
            "derivatives_complete",
        ],
    )?;
    let covariance = checked(&params["covariance"])?;
    let mismatch = "components/gradient/covariance diagonal mismatch";
    let (components, gradient, variances) = components(params, mismatch)?;
    if variances.len() != covariance.len()
        || variances
            .iter()
            .enumerate()
            .any(|(i, v)| v != &covariance[i][i])
    {
        return Err(invalid(mismatch));
    }
    let order = order(params)?;
    let digits = guard(
        &params["guard_digits"],
        1000,
        "guard_digits must be positive",
    )?;
    let mut estimate = exact(&params["estimate"])?;
    let projected = product(&gradient, &covariance);
    let mut variance = dot(&projected, &gradient);
    let law = &params["joint_distribution"];
    if order == 2 {
        if symbol_name(law) != Some("multivariate_normal") || !normal_components(components) {
            return Err(unsupported(
                "GUF2 covariance requires an explicitly joint multivariate normal law",
            ));
        }
        let n = covariance.len();
        let (h, t) = tensors(
            params,
            n,
            "complete analytic Hessian and third derivatives are required",
        )?;
        let hs: Vec<_> = h.iter().map(|row| product(row, &covariance)).collect();
        for i in 0..n {
            let mut contracted_third = Q::zero();
            for j in 0..n {
                estimate += &h[i][j] * &covariance[i][j] / number(2);
                variance += &hs[i][j] * &hs[j][i] / number(2);
                contracted_third += dot(&t[i][j], &covariance[j]);
            }
            variance += &projected[i] * contracted_third;
        }
        if variance.is_negative() {
            return Err(unsupported(
                "Taylor covariance approximation gives negative variance",
            ));
        }
    }
    let law = if law.is_null() {
        J::Null
    } else {
        symbol(symbol_name(law).ok_or_else(|| invalid("joint_distribution must be a symbol"))?)
    };
    Ok(json!({
        "estimate":tagged(&estimate),"variance":tagged(&variance),
        "standard_uncertainty":{"decimal":root_decimal(&variance,digits)?},
        "guard_digits":digits,"method":symbol(if order == 1 {"lpu_covariance"} else {"guf2_covariance"}),"joint_distribution":law,
        "adequacy":symbol("not_assessed"),"effective_degrees_of_freedom":null,
        "degrees_of_freedom_status":symbol("not_evaluated_for_correlated_inputs"),
        "engine":engine("Gaussian-covariance-contraction-v1"),
    }))
}

pub(super) fn transform(params: &J) -> Result<J> {
    fields(
        params,
        &["estimates", "jacobian", "covariance", "guard_digits"],
    )?;
    let covariance = checked(&params["covariance"])?;
    let shape = "estimate/Jacobian shape mismatch";
    let estimates = vector(&params["estimates"], shape)?;
    let jacobian = matrix(&params["jacobian"], shape, 1)?;
    if estimates.len() != jacobian.len()
        || estimates.len() > 64
        || jacobian[0].len() != covariance.len()
    {
        return Err(invalid(shape));
    }
    let digits = guard(
        &params["guard_digits"],
        1000,
        "guard_digits must be positive",
    )?;
    let output: Vec<Vec<_>> = jacobian
        .iter()
        .map(|row| {
            let projected = product(row, &covariance);
            jacobian.iter().map(|b| dot(&projected, b)).collect()
        })
        .collect();
    let roots = output
        .iter()
        .enumerate()
        .map(|(i, row)| Ok(json!({"decimal":root_decimal(&row[i],digits)?})))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "estimates":estimates.iter().map(tagged).collect::<Vec<_>>(),
        "covariance":output.iter().map(|row| row.iter().map(tagged).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "standard_uncertainties":roots,"guard_digits":digits,"method":symbol("lpu_covariance"),
        "adequacy":symbol("not_assessed"),"effective_degrees_of_freedom":null,
        "degrees_of_freedom_status":symbol("not_evaluated_for_correlated_inputs"),
        "engine":engine("JCGM100-2008-equation13-v1"),
    }))
}
