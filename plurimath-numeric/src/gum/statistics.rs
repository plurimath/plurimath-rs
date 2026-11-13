use super::*;

struct Sample {
    count: usize,
    mean: Q,
    sample_variance: Q,
    variance: Q,
    minimum: Q,
    maximum: Q,
}

impl Sample {
    fn new(values: &[Q]) -> Result<Self> {
        let count = values.len();
        if count < 2 {
            return Err(invalid("Type A needs at least two observations"));
        }
        let mean = values.iter().sum::<Q>() / number(count);
        let sample_variance =
            values.iter().map(|value| (value - &mean).pow(2)).sum::<Q>() / number(count - 1);
        let variance = &sample_variance / number(count);
        Ok(Self {
            count,
            mean,
            sample_variance,
            variance,
            minimum: values.iter().min().expect("nonempty sample").clone(),
            maximum: values.iter().max().expect("nonempty sample").clone(),
        })
    }

    fn wire(&self) -> J {
        json!({
            "count":self.count,"mean":tagged(&self.mean),"minimum":tagged(&self.minimum),
            "maximum":tagged(&self.maximum),"sample_variance":tagged(&self.sample_variance),
            "variance":tagged(&self.variance),"degrees_of_freedom":self.count-1,
            "distribution":symbol("unspecified"),
        })
    }
}

pub(super) fn type_a_request(params: &J) -> Result<J> {
    fields(params, &["observations"])?;
    let values = vector(&params["observations"], "observations must be an Array")?;
    Ok(Sample::new(&values)?.wire())
}

pub(super) fn covariance(params: &J) -> Result<J> {
    fields(params, &["observations"])?;
    let rows = matrix(
        &params["observations"],
        "paired Type-A observations need a rectangular matrix with at least two rows",
        2,
    )?;
    let n = rows.len();
    let width = rows[0].len();
    let means: Vec<_> = (0..width)
        .map(|i| rows.iter().map(|row| &row[i]).sum::<Q>() / number(n))
        .collect();
    let covariance: Vec<Vec<Q>> = (0..width)
        .map(|i| {
            (0..width)
                .map(|j| {
                    rows.iter()
                        .map(|row| (&row[i] - &means[i]) * (&row[j] - &means[j]))
                        .sum::<Q>()
                        / number(n - 1)
                })
                .collect()
        })
        .collect();
    Ok(json!({
        "count":n,"means":means.iter().map(tagged).collect::<Vec<_>>(),
        "sample_covariance":covariance.iter().map(|row| row.iter().map(tagged).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "covariance_of_mean":covariance.iter().map(|row| row.iter().map(|x| tagged(&(x / number(n)))).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "degrees_of_freedom":n-1,"distribution":symbol("unspecified"),
        "engine":engine("paired-TypeA-covariance-v1"),
    }))
}

fn window(input: &J, length: usize) -> Result<(usize, usize)> {
    if input.is_null() {
        return Ok((0, length));
    }
    let refuse = || invalid("window must select at least two existing observations");
    let fields = input
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(refuse)?;
    let start = fields[0]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(refuse)?;
    let count = fields[1]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(refuse)?;
    let end = start
        .checked_add(count)
        .filter(|&end| count >= 2 && end <= length)
        .ok_or_else(refuse)?;
    Ok((start, end))
}

fn series_sample(values: &[Q], axis: usize, window: &J, digits: u32) -> Result<J> {
    let sample = Sample::new(values)?;
    let mut out = sample.wire();
    for (name, value) in [
        ("sample_standard_deviation", &sample.sample_variance),
        ("standard_uncertainty_of_mean", &sample.variance),
    ] {
        let rounded = root_decimal(value, digits + 4)?;
        out[name] = json!(fmt_decimal(roundq(&exact_text(&rounded)?, digits)));
    }
    out["axis"] = json!(axis);
    out["window"] = window.clone();
    out["guard_digits"] = json!(digits);
    out["engine"] = engine("sample-series-v1");
    Ok(out)
}

pub(super) fn series(params: &J) -> Result<J> {
    fields(params, &["observations", "axis", "window", "guard_digits"])?;
    let digits = guard(
        &params["guard_digits"],
        12,
        "binary64 statistical outputs support 1..12 declared digits",
    )?;
    let refuse = || invalid("series needs an Array and an explicit axis0 or1");
    let axis = params["axis"]
        .as_u64()
        .filter(|&i| i <= 1)
        .ok_or_else(refuse)? as usize;
    let rows = params["observations"].as_array().ok_or_else(refuse)?;
    let range = &params["window"];
    if rows.first().is_some_and(J::is_array) {
        let values = matrix(
            &params["observations"],
            "matrix series must be nonempty and rectangular",
            1,
        )?;
        let (start, end) = window(
            range,
            if axis == 0 {
                values.len()
            } else {
                values[0].len()
            },
        )?;
        let output = if axis == 0 {
            (0..values[0].len())
                .map(|i| {
                    let column: Vec<_> = values[start..end]
                        .iter()
                        .map(|row| row[i].clone())
                        .collect();
                    series_sample(&column, axis, range, digits)
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            values
                .iter()
                .map(|row| series_sample(&row[start..end], axis, range, digits))
                .collect::<Result<Vec<_>>>()?
        };
        Ok(json!(output))
    } else {
        if axis != 0 {
            return Err(invalid("a vector has only axis0"));
        }
        let (start, end) = window(range, rows.len())?;
        let values = vector(&params["observations"], "observations must be an Array")?;
        series_sample(&values[start..end], axis, range, digits)
    }
}

pub(super) fn grouped(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "group_means",
            "group_sample_variances",
            "observations_per_group",
            "variance_model",
            "guard_digits",
        ],
    )?;
    let refuse = || invalid("balanced groups need paired summaries, J>=2 and K>=2");
    let means = vector(&params["group_means"], refuse().message)?;
    let within = vector(&params["group_sample_variances"], refuse().message)?;
    let j = means.len();
    let k = params["observations_per_group"]
        .as_u64()
        .filter(|&v| (2..=4096).contains(&v))
        .ok_or_else(refuse)? as usize;
    if j < 2 || j != within.len() {
        return Err(refuse());
    }
    let model = symbol_name(&params["variance_model"])
        .filter(|&s| ["random_group_effect", "no_group_effect"].contains(&s))
        .ok_or_else(|| unsupported("declare random_group_effect or no_group_effect"))?;
    let digits = guard(
        &params["guard_digits"],
        1000,
        "guard_digits must be positive",
    )?;
    if within.iter().any(Q::is_negative) {
        return Err(invalid("negative within-group variance"));
    }
    let sample = Sample::new(&means)?;
    let between_square = &sample.sample_variance * number(k);
    let within_square = within.iter().sum::<Q>() / number(j);
    let between = (&between_square - &within_square) / number(k);
    let random = model == "random_group_effect";
    if random && between.is_negative() {
        return Err(unsupported("negative estimated group component requires an explicit constrained model; no clipping is inferred"));
    }
    let all = (&between_square * number(j - 1) + &within_square * number(j * (k - 1)))
        / number(j * k - 1);
    let variance = if random {
        sample.variance
    } else {
        &all / number(j * k)
    };
    Ok(json!({
        "mean":tagged(&sample.mean),"groups":j,"observations_per_group":k,"count":j*k,
        "within_group_variance":tagged(&within_square),"group_mean_variance":tagged(&sample.sample_variance),
        "between_group_component":tagged(&between),"between_mean_square":tagged(&between_square),
        "within_mean_square":tagged(&within_square),"all_observations_sample_variance":tagged(&all),
        "f_statistic":(!within_square.is_zero()).then(|| tagged(&(&between_square / &within_square))),
        "f_degrees_of_freedom":[j-1,j*(k-1)],"variance":tagged(&variance),
        "standard_uncertainty":{"decimal":root_decimal(&variance,digits)?},
        "degrees_of_freedom":if random { j-1 } else { j*k-1 },"variance_model":symbol(model),
        "adequacy":symbol("not_assessed"),"guard_digits":digits,"engine":engine("GUM100-H5-balanced-groups-v1"),
    }))
}
