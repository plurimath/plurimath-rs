use super::*;
use num_traits::{One, ToPrimitive};
use std::collections::{BTreeMap, BTreeSet};

const MODULI: [u64; 4] = [2_147_483_579, 2_147_483_543, 2_147_483_423, 2_147_483_123];
const MULTIPLIERS: [u64; 4] = [11_600, 47_003, 23_000, 33_000];

fn numerical(message: &'static str) -> Error {
    Error {
        code: "NumericalFailure",
        message,
    }
}

struct Generator {
    initial: [u64; 4],
    state: [u64; 4],
    draws: u64,
    normal_spare: Option<f64>,
}

impl Generator {
    fn new(value: &J) -> Result<Self> {
        let refuse = || invalid("four nonzero WH seeds below their respective moduli are required");
        let seeds = value
            .as_array()
            .filter(|seeds| seeds.len() == 4)
            .ok_or_else(refuse)?;
        let mut initial = [0; 4];
        for (i, seed) in seeds.iter().enumerate() {
            initial[i] = seed
                .as_u64()
                .filter(|n| *n > 0 && *n < MODULI[i])
                .ok_or_else(refuse)?;
        }
        Ok(Self {
            initial,
            state: initial,
            draws: 0,
            normal_spare: None,
        })
    }

    fn uniform(&mut self) -> Result<f64> {
        let mut sum = 0.0;
        for i in 0..4 {
            self.state[i] = (MULTIPLIERS[i] * self.state[i]) % MODULI[i];
            sum += self.state[i] as f64 / MODULI[i] as f64;
        }
        self.draws += 1;
        let draw = sum - sum.floor();
        if !(draw > 0.0 && draw < 1.0) {
            return Err(numerical("WH uniform draw rounded to an endpoint"));
        }
        Ok(draw)
    }

    fn normal(&mut self) -> Result<f64> {
        if let Some(value) = self.normal_spare.take() {
            return Ok(value);
        }
        let radius = (-2.0 * self.uniform()?.ln()).sqrt();
        let angle = 2.0 * std::f64::consts::PI * self.uniform()?;
        self.normal_spare = Some(radius * angle.sin());
        Ok(radius * angle.cos())
    }

    fn student(&mut self, nu: f64) -> Result<f64> {
        for _ in 0..10_000 {
            let first = self.uniform()?;
            let second = self.uniform()?;
            let (t, v) = if first < 0.5 {
                let denominator = 4.0 * first - 1.0;
                if denominator == 0.0 {
                    continue;
                }
                let t = 1.0 / denominator;
                (t, second / (t * t))
            } else {
                (4.0 * first - 3.0, second)
            };
            if v < 1.0 - t.abs() / 2.0 || v < (1.0 + t * t / nu).powf(-(nu + 1.0) / 2.0) {
                return Ok(t);
            }
        }
        Err(numerical("Student-t rejection limit exceeded"))
    }
}

#[derive(Clone, Copy)]
enum Pdf {
    Normal(f64),
    Student(f64, f64),
    Rectangular(f64),
    Triangular(f64),
    Arcsine(f64),
    Trapezoid(f64, f64),
}

fn binary64(value: &Q) -> Result<f64> {
    value
        .to_f64()
        .filter(|x| x.is_finite())
        .ok_or_else(|| invalid("PDF parameter exceeds binary64 range"))
}

impl Pdf {
    fn build(params: &J) -> Result<(Q, Self)> {
        let distribution = symbol_name(&params["distribution"])
            .ok_or_else(|| invalid("each input needs an explicit PDF declaration"))?;
        if distribution == "normal" {
            fields(params, &["distribution", "location", "sd"])?;
            let sd = binary64(&exact(&params["sd"])?)?;
            if sd <= 0.0 {
                return Err(invalid("normal sd must be positive"));
            }
            return Ok((exact(&params["location"])?, Self::Normal(sd)));
        }
        if distribution == "student_t" {
            fields(
                params,
                &["distribution", "location", "scale", "degrees_of_freedom"],
            )?;
            let scale = binary64(&exact(&params["scale"])?)?;
            let nu = binary64(&exact(&params["degrees_of_freedom"])?)?;
            if scale <= 0.0 || nu <= 2.0 {
                return Err(invalid("Student-t needs positive scale and nu > 2"));
            }
            return Ok((exact(&params["location"])?, Self::Student(scale, nu)));
        }
        if ![
            "rectangular",
            "triangular",
            "arcsine",
            "curvilinear_trapezoid",
        ]
        .contains(&distribution)
        {
            return Err(unsupported("unsupported sampling PDF"));
        }
        fields(
            params,
            &["distribution", "lower", "upper", "boundary_half_width"],
        )?;
        let lower = exact(&params["lower"])?;
        let upper = exact(&params["upper"])?;
        if lower >= upper {
            return Err(invalid("PDF bounds must increase"));
        }
        let width = binary64(&(&upper - &lower))?;
        if width / 2.0 <= 0.0 {
            return Err(invalid("PDF bounds collapse in binary64"));
        }
        let pdf = match distribution {
            "rectangular" => Self::Rectangular(width),
            "triangular" => Self::Triangular(width),
            "arcsine" => Self::Arcsine(width),
            _ => {
                let d = binary64(&exact(&params["boundary_half_width"])?)?;
                if d <= 0.0 || 2.0 * d >= width {
                    return Err(invalid("curvilinear bounds need 0 < d < half-width"));
                }
                Self::Trapezoid(width, d)
            }
        };
        Ok(((lower + upper) / number(2), pdf))
    }

    fn draw(self, generator: &mut Generator) -> Result<f64> {
        let value = match self {
            Self::Normal(sd) => 0.0 + sd * generator.normal()?,
            Self::Student(scale, nu) => 0.0 + scale * generator.student(nu)?,
            Self::Rectangular(width) => -width / 2.0 + width * generator.uniform()?,
            Self::Triangular(width) => {
                -width / 2.0 + width * (generator.uniform()? + generator.uniform()?) / 2.0
            }
            Self::Arcsine(width) => {
                width / 2.0 * (2.0 * std::f64::consts::PI * generator.uniform()?).sin()
            }
            Self::Trapezoid(width, d) => {
                let a = -width / 2.0 - d + 2.0 * d * generator.uniform()?;
                let b = -a;
                a + (b - a) * generator.uniform()?
            }
        };
        if !value.is_finite() {
            return Err(numerical("non-finite PDF draw"));
        }
        Ok(value)
    }
}

#[derive(Default)]
struct Moments {
    count: usize,
    mean: f64,
    squares: f64,
}

impl Moments {
    fn append(&mut self, values: &[f64]) {
        for value in values {
            self.count += 1;
            let delta = value - self.mean;
            self.mean += delta / self.count as f64;
            self.squares += delta * (value - self.mean);
        }
    }

    fn uncertainty(&self) -> f64 {
        (self.squares / (self.count - 1) as f64).sqrt()
    }
}

#[derive(Clone, Copy)]
enum Interval {
    Symmetric,
    Shortest,
    Both,
}

fn summarize(
    values: &mut [f64],
    probability: &Q,
    interval: Interval,
    moments: &Moments,
) -> Result<BTreeMap<&'static str, f64>> {
    let mut result = BTreeMap::from([
        ("estimate", moments.mean),
        ("standard_uncertainty", moments.uncertainty()),
    ]);
    values.sort_unstable_by(f64::total_cmp);
    let n = values.len();
    let q = (probability * number(n) + Q::new(number(1), number(2)))
        .floor()
        .to_integer()
        .to_usize()
        .filter(|q| *q > 0 && *q < n)
        .ok_or_else(|| invalid("too few samples for the requested coverage"))?;
    let remaining = n - q;
    if matches!(interval, Interval::Symmetric | Interval::Both) {
        let first = remaining.div_ceil(2) - 1;
        result.insert("symmetric_lower", values[first]);
        result.insert("symmetric_upper", values[first + q]);
    }
    if matches!(interval, Interval::Shortest | Interval::Both) {
        let mut first = 0;
        let mut width = values[q] - values[0];
        for i in 1..remaining {
            let candidate = values[i + q] - values[i];
            if candidate < width {
                first = i;
                width = candidate;
            }
        }
        result.insert("shortest_lower", values[first]);
        result.insert("shortest_upper", values[first + q]);
    }
    Ok(result)
}

fn compensated_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0_f64;
    let mut correction = 0.0;
    for value in values {
        let next = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - next) + value
        } else {
            (value - next) + sum
        };
        sum = next;
    }
    sum + correction
}

fn tolerance(uncertainty: f64, digits: u32) -> Result<Q> {
    if !uncertainty.is_finite() || uncertainty <= 0.0 {
        return Err(numerical("adaptive tolerance needs positive uncertainty"));
    }
    let rounded = roundq(&exact_text(&uncertainty.to_string())?, digits)
        .q()
        .expect("finite rounded uncertainty");
    let place = crate::exponent(&rounded) - i64::from(digits) + 1;
    Ok(if place >= 0 {
        Q::new(crate::pow10(place as u32), number(2))
    } else {
        Q::new(number(1), number(2) * crate::pow10((-place) as u32))
    })
}

fn decimal(value: f64, digits: u32, offset: Option<&Q>) -> Result<String> {
    let mut ratio =
        Q::from_float(value).ok_or_else(|| numerical("non-finite statistical result"))?;
    if let Some(offset) = offset {
        ratio += offset;
    }
    Ok(fmt_decimal(roundq(&ratio, digits)))
}

pub(super) fn evaluate(params: &J) -> Result<J> {
    fields(
        params,
        &[
            "ast",
            "inputs",
            "independent",
            "seeds",
            "probability",
            "interval",
            "significant_digits",
            "guard_digits",
            "max_samples",
            "joint_covariance",
            "joint_distribution",
        ],
    )?;
    if params["independent"] != true {
        return Err(unsupported("joint sampling is outside this profile"));
    }
    if !params["joint_covariance"].is_null() || !params["joint_distribution"].is_null() {
        return Err(invalid(
            "independent sampling cannot also declare a joint covariance or law",
        ));
    }
    let inputs = params["inputs"]
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 16)
        .ok_or_else(|| invalid("inputs must be a nonempty ordered PDF mapping"))?;
    let mut seen = BTreeSet::new();
    let mut variables = Vec::new();
    let mut centers = Vec::new();
    let mut distributions = Vec::new();
    for input in inputs {
        fields(input, &["name", "pdf"])?;
        let name = input["name"]
            .as_str()
            .filter(|name| crate::name(name))
            .ok_or_else(|| invalid("inputs must be a nonempty ordered PDF mapping"))?;
        if !seen.insert(name) {
            return Err(invalid("duplicate variable names"));
        }
        let (center, distribution) = Pdf::build(&input["pdf"])?;
        variables.push(name);
        centers.push(center);
        distributions.push(distribution);
    }
    let probability = exact(&params["probability"])?;
    if !probability.is_positive() || probability >= Q::one() {
        return Err(invalid(
            "probability must lie strictly between zero and one",
        ));
    }
    let interval = match symbol_name(&params["interval"]) {
        Some("symmetric") => Interval::Symmetric,
        Some("shortest") => Interval::Shortest,
        Some("both") => Interval::Both,
        _ => return Err(invalid("interval must be symmetric, shortest or both")),
    };
    let guard_digits = guard(
        &params["guard_digits"],
        12,
        "binary64 statistical outputs support 1..12 declared digits",
    )?;
    let significant = guard(
        &params["significant_digits"],
        guard_digits,
        "significant_digits must lie in 1..guard_digits",
    )?;
    let budget = || {
        invalid(
            "max_samples must admit at least two normative batches within the 8000000 sample cap",
        )
    };
    let batch = (Q::from_integer(number(100)) / (Q::one() - &probability))
        .ceil()
        .to_integer()
        .to_usize()
        .ok_or_else(budget)?
        .max(10_000);
    let max_samples = params["max_samples"]
        .as_u64()
        .filter(|n| *n <= 8_000_000)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| n / batch >= 2)
        .ok_or_else(budget)?;
    let mut generator = Generator::new(&params["seeds"])?;
    let model = super::taylor::sampling_polynomial(&params["ast"], &variables, centers)?;
    for (pdf, degree) in distributions.iter().zip(&model.degrees) {
        if matches!(pdf, Pdf::Student(_, nu) if *nu <= 2.0 * *degree as f64) {
            return Err(unsupported("finite polynomial output variance requires Student-t nu greater than twice its variable degree"));
        }
    }
    let mut all = Vec::new();
    let mut summaries = Vec::new();
    let mut overall = Moments::default();
    let mut deviations = vec![0.0; inputs.len()];
    let mut values = Vec::with_capacity(batch);
    for _ in 0..max_samples / batch {
        values.clear();
        for _ in 0..batch {
            for (value, pdf) in deviations.iter_mut().zip(&distributions) {
                *value = pdf.draw(&mut generator)?;
            }
            let value = model.centered_value(&deviations);
            if !value.is_finite() {
                return Err(numerical("non-finite sampled model value"));
            }
            values.push(value);
        }
        overall.append(&values);
        all.extend_from_slice(&values);
        let mut batch_moments = Moments::default();
        batch_moments.append(&values);
        summaries.push(summarize(
            &mut values,
            &probability,
            interval,
            &batch_moments,
        )?);
        let n = summaries.len();
        if n < 2 {
            continue;
        }
        let delta = tolerance(overall.uncertainty(), significant)?;
        let stability: BTreeMap<_, _> = summaries[0]
            .keys()
            .map(|&key| {
                let values = summaries.iter().map(|summary| summary[key]);
                let mean = compensated_sum(values.clone()) / n as f64;
                let squares = compensated_sum(values.map(|value| (value - mean).powi(2)));
                (key, 2.0 * (squares / (n * (n - 1)) as f64).sqrt())
            })
            .collect();
        let limit = delta
            .to_f64()
            .ok_or_else(|| numerical("non-finite numerical tolerance"))?;
        if stability
            .values()
            .any(|value| !value.is_finite() || *value > limit)
        {
            continue;
        }
        let statistics = summarize(&mut all, &probability, interval, &overall)?;
        let mut intervals = serde_json::Map::new();
        for (kind, lower, upper) in [
            ("symmetric", "symmetric_lower", "symmetric_upper"),
            ("shortest", "shortest_lower", "shortest_upper"),
        ] {
            if let (Some(low), Some(high)) = (statistics.get(lower), statistics.get(upper)) {
                intervals.insert(
                    kind.to_owned(),
                    json!([
                        decimal(*low, guard_digits, Some(&model.offset))?,
                        decimal(*high, guard_digits, Some(&model.offset))?,
                    ]),
                );
            }
        }
        let errors = stability
            .iter()
            .map(|(name, value)| {
                Ok((
                    name.to_string(),
                    json!(decimal(*value, guard_digits, None)?),
                ))
            })
            .collect::<Result<serde_json::Map<_, _>>>()?;
        let delta = fmt_decimal(roundq(&delta, 12));
        let delta = if delta.contains('.') {
            delta
        } else {
            format!("{delta}.0")
        };
        return Ok(json!({
            "estimate":decimal(statistics["estimate"],guard_digits,Some(&model.offset))?,
            "standard_uncertainty":decimal(statistics["standard_uncertainty"],guard_digits,None)?,
            "intervals":intervals,"probability":format!("{}/{}",probability.numer(),probability.denom()),
            "numerical_tolerance":delta,"twice_standard_errors":errors,
            "samples":all.len(),"requested_max_samples":max_samples,"batches":n,"batch_size":batch,
            "significant_digits":significant,"guard_digits":guard_digits,"precision_convention":symbol("significant_digits"),
            "stabilized":true,"method":symbol("adaptive_mcm"),"variables":variables,
            "engine":{"name":"plurimath-rust","version":env!("CARGO_PKG_VERSION"),
              "method":"JCGM101-2008-7.9.4-v1","numeric_backend":"binary64-centered-exact-polynomial",
              "generator":"enhanced-Wichmann-Hill-C.3.3.3","initial_seeds":generator.initial,
              "final_state":generator.state,"uniform_draws":generator.draws},
        }));
    }
    Err(Error {
        code: "ConvergenceError",
        message: "adaptive MCM did not stabilize within the declared sample budget",
    })
}
