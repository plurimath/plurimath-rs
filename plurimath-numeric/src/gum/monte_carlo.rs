use super::*;
use crate::prng::Philox4x32;
use crate::scalar::Scalar;
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

/// Counters reserved per block for `gum-par`; blocks never share a counter.
const BLOCK_COUNTER_STRIDE: u128 = 1 << 40;

/// Sequential uniform stream over one block's Philox counter range.
struct Stream {
    rng: Philox4x32,
    first: u128,
    next: u128,
    spare: Option<f64>,
}

impl Stream {
    fn draw(&mut self) -> Result<f64> {
        if let Some(value) = self.spare.take() {
            return Ok(value);
        }
        if self.next - self.first >= BLOCK_COUNTER_STRIDE {
            return Err(numerical("Philox block counter range exhausted"));
        }
        let [a, b] = self.rng.uniform_pair(self.next);
        self.next += 1;
        self.spare = Some(b);
        Ok(a)
    }
}

struct Generator {
    initial: [u64; 4],
    state: [u64; 4],
    draws: u64,
    normal_spare: Option<f64>,
    philox: Option<Stream>,
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
            philox: None,
        })
    }

    /// Generator for one `gum-par` block: counters `block * STRIDE ..`.
    fn philox(rng: Philox4x32, block: u64) -> Self {
        let first = u128::from(block) * BLOCK_COUNTER_STRIDE;
        Self {
            initial: [0; 4],
            state: [0; 4],
            draws: 0,
            normal_spare: None,
            philox: Some(Stream {
                rng,
                first,
                next: first,
                spare: None,
            }),
        }
    }

    fn uniform(&mut self) -> Result<f64> {
        if let Some(stream) = &mut self.philox {
            let draw = stream.draw()?;
            self.draws += 1;
            if !(draw > 0.0 && draw < 1.0) {
                return Err(numerical("Philox uniform draw rounded to an endpoint"));
            }
            return Ok(draw);
        }
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
        let radius = libm::sqrt(-2.0 * libm::log(self.uniform()?));
        let angle = 2.0 * std::f64::consts::PI * self.uniform()?;
        self.normal_spare = Some(radius * libm::sin(angle));
        Ok(radius * libm::cos(angle))
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
            if v < 1.0 - t.abs() / 2.0 || v < libm::pow(1.0 + t * t / nu, -(nu + 1.0) / 2.0) {
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
                width / 2.0 * libm::sin(2.0 * std::f64::consts::PI * generator.uniform()?)
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

    /// Block moments from fixed-tree pairwise sums (no data-order dependence).
    fn pairwise(values: &[f64]) -> Self {
        let count = values.len();
        let mean = pairwise_sum(values) / count as f64;
        let squares = pairwise_map_sum(values, &|v| (v - mean) * (v - mean));
        Self {
            count,
            mean,
            squares,
        }
    }

    /// Chan et al. combination, applied in block order.
    fn merge(&mut self, other: &Moments) {
        if self.count == 0 {
            self.count = other.count;
            self.mean = other.mean;
            self.squares = other.squares;
            return;
        }
        let (a, b) = (self.count as f64, other.count as f64);
        let total = a + b;
        let delta = other.mean - self.mean;
        self.squares += other.squares + delta * delta * a * b / total;
        self.mean += delta * b / total;
        self.count += other.count;
    }

    fn uncertainty(&self) -> f64 {
        libm::sqrt(self.squares / (self.count - 1) as f64)
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

const PAIRWISE_LEAF: usize = 8;

fn pairwise_sum(values: &[f64]) -> f64 {
    pairwise_map_sum(values, &|v| v)
}

/// Sum over a fixed balanced tree: split at `len / 2`, sequential leaves of 8.
fn pairwise_map_sum(values: &[f64], f: &dyn Fn(f64) -> f64) -> f64 {
    if values.len() <= PAIRWISE_LEAF {
        return values.iter().fold(0.0, |sum, v| sum + f(*v));
    }
    let (left, right) = values.split_at(values.len() / 2);
    pairwise_map_sum(left, f) + pairwise_map_sum(right, f)
}

type BlockResult = Result<(Vec<f64>, (u128, u128), u64)>;

fn fill_block(
    batch: usize,
    joint: &Option<JointNormal>,
    distributions: &[Pdf],
    model: &super::sampling_model::Model,
    generator: &mut Generator,
    deviations: &mut [f64],
    values: &mut Vec<f64>,
) -> Result<()> {
    for _ in 0..batch {
        if let Some(joint) = joint {
            joint.draw(generator, deviations)?;
        } else {
            for (value, pdf) in deviations.iter_mut().zip(distributions) {
                *value = pdf.draw(generator)?;
            }
        }
        let value = model.evaluate(deviations)?;
        if !value.is_finite() {
            return Err(numerical("non-finite sampled model value"));
        }
        values.push(value);
    }
    Ok(())
}

/// One `gum-par` block: a pure function of (seed, block index).
fn par_block(
    rng: Philox4x32,
    index: u64,
    batch: usize,
    joint: &Option<JointNormal>,
    distributions: &[Pdf],
    model: &super::sampling_model::Model,
) -> BlockResult {
    let mut generator = Generator::philox(rng, index);
    let mut deviations = vec![0.0; distributions.len()];
    let mut values = Vec::with_capacity(batch);
    fill_block(
        batch,
        joint,
        distributions,
        model,
        &mut generator,
        &mut deviations,
        &mut values,
    )?;
    let stream = generator.philox.as_ref().expect("philox stream");
    Ok((values, (stream.first, stream.next), generator.draws))
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
    exact_tolerance(&exact_text(&uncertainty.to_string())?, u64::from(digits))
}

pub(super) fn exact_tolerance(uncertainty: &Q, digits: u64) -> Result<Q> {
    if !(1..=12).contains(&digits) {
        return Err(invalid("tolerance significant_digits must lie in1..12"));
    }
    if !uncertainty.is_positive() {
        return Err(numerical("adaptive tolerance needs positive uncertainty"));
    }
    let rounded = roundq(uncertainty, digits as u32)
        .q()
        .expect("finite rounded uncertainty");
    let place = crate::exponent(&rounded) - digits as i64 + 1;
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
            "engine",
            "seed",
            "threads",
        ],
    )?;
    let par = match params["engine"].as_str() {
        None if params["engine"].is_null() => None,
        Some("gum-ref") => None,
        Some("gum-par") => {
            Some(Philox4x32::new(params["seed"].as_u64().ok_or_else(
                || invalid("gum-par requires an unsigned integer seed"),
            )?))
        }
        _ => return Err(unsupported("engine must be gum-ref or gum-par")),
    };
    if par.is_some() != params["seeds"].is_null() {
        return Err(invalid(
            "gum-ref takes four WH seeds; gum-par takes a single seed and no WH seeds",
        ));
    }
    if par.is_none() && !(params["seed"].is_null() && params["threads"].is_null()) {
        return Err(invalid("seed and threads apply only to gum-par"));
    }
    let threads = match &params["threads"] {
        J::Null => std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(8),
        value => value
            .as_u64()
            .filter(|n| (1..=64).contains(n))
            .ok_or_else(|| invalid("threads must lie in 1..64"))? as usize,
    };
    let independent = match params["independent"].as_bool() {
        Some(true) => {
            if !params["joint_covariance"].is_null() || !params["joint_distribution"].is_null() {
                return Err(invalid(
                    "independent sampling cannot also declare a joint covariance or law",
                ));
            }
            true
        }
        Some(false)
            if symbol_name(&params["joint_distribution"]) == Some("multivariate_normal")
                && !params["joint_covariance"].is_null() =>
        {
            false
        }
        _ => {
            return Err(unsupported(
                "joint sampling requires explicit multivariate_normal knowledge and covariance",
            ))
        }
    };
    let inputs = params["inputs"]
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 16)
        .ok_or_else(|| invalid("inputs must be a nonempty ordered PDF mapping"))?;
    let mut seen = BTreeSet::new();
    let mut variables = Vec::new();
    let mut centers = Vec::new();
    let mut definitions = Vec::new();
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
        definitions.push(&input["pdf"]);
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
    let mut generator = if par.is_some() {
        Generator::philox(Philox4x32::new(0), 0)
    } else {
        Generator::new(&params["seeds"])?
    };
    let model =
        super::sampling_model::Model::build(&params["ast"], &variables, centers, &definitions)?;
    let joint = if independent {
        None
    } else {
        Some(JointNormal::new(&definitions, &params["joint_covariance"])?)
    };
    for (pdf, degree) in distributions.iter().zip(model.degrees()) {
        if matches!(pdf, Pdf::Student(_, nu) if *nu <= 2.0 * *degree as f64) {
            return Err(unsupported("finite polynomial output variance requires Student-t nu greater than twice its variable degree"));
        }
    }
    let mut all = Vec::new();
    let mut summaries = Vec::new();
    let mut overall = Moments::default();
    let mut deviations = vec![0.0; inputs.len()];
    let mut values = Vec::with_capacity(batch);
    let blocks = max_samples / batch;
    let mut pending: std::collections::VecDeque<BlockResult> = Default::default();
    let mut ranges: Vec<(u128, u128)> = Vec::new();
    let mut par_draws = 0_u64;
    for block in 0..blocks {
        values.clear();
        let batch_moments = if let Some(rng) = par {
            if pending.is_empty() {
                let count = threads.min(blocks - block);
                let (joint, distributions, model) = (&joint, &distributions, &model);
                pending.extend(std::thread::scope(|scope| {
                    let handles: Vec<_> = (block..block + count)
                        .map(|index| {
                            scope.spawn(move || {
                                par_block(rng, index as u64, batch, joint, distributions, model)
                            })
                        })
                        .collect();
                    handles
                        .into_iter()
                        .map(|handle| handle.join().expect("gum-par block thread"))
                        .collect::<Vec<_>>()
                }));
            }
            let (block_values, range, draws) = pending.pop_front().expect("pending block")?;
            values = block_values;
            ranges.push(range);
            par_draws += draws;
            let moments = Moments::pairwise(&values);
            overall.merge(&moments);
            moments
        } else {
            fill_block(
                batch,
                &joint,
                &distributions,
                &model,
                &mut generator,
                &mut deviations,
                &mut values,
            )?;
            overall.append(&values);
            let mut moments = Moments::default();
            moments.append(&values);
            moments
        };
        all.extend_from_slice(&values);
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
                (key, 2.0 * libm::sqrt(squares / (n * (n - 1)) as f64))
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
                        decimal(*low, guard_digits, Some(model.offset()))?,
                        decimal(*high, guard_digits, Some(model.offset()))?,
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
        let mut stamp = if par.is_some() {
            json!({
                "name":"plurimath-rust","version":env!("CARGO_PKG_VERSION"),
                "method":"JCGM101-2008-7.9.4-v1","id":"gum-par","accuracy":"limited",
                "generator":"philox4x32-10","seed":params["seed"],
                "counter_stride":BLOCK_COUNTER_STRIDE.to_string(),
                "counter_ranges":ranges.iter().enumerate().map(|(block,(first,end))| json!({
                    "block":block,"first":first.to_string(),"end":end.to_string(),
                })).collect::<Vec<_>>(),
                "uniform_draws":par_draws,"reduction":"pairwise-tree-leaf8-chan-merge-v1",
            })
        } else {
            json!({
                "name":"plurimath-rust","version":env!("CARGO_PKG_VERSION"),
                "method":"JCGM101-2008-7.9.4-v1",
                "generator":"enhanced-Wichmann-Hill-C.3.3.3","initial_seeds":generator.initial,
                "final_state":generator.state,"uniform_draws":generator.draws,
            })
        };
        stamp.as_object_mut().expect("engine object").extend(
            model
                .engine_fields()
                .as_object()
                .expect("model engine object")
                .clone(),
        );
        if let Some(joint) = &joint {
            stamp.as_object_mut().expect("engine object").extend(
                joint
                    .engine_fields()
                    .as_object()
                    .expect("joint engine object")
                    .clone(),
            );
        }
        return Ok(json!({
            "estimate":decimal(statistics["estimate"],guard_digits,Some(model.offset()))?,
            "standard_uncertainty":decimal(statistics["standard_uncertainty"],guard_digits,None)?,
            "intervals":intervals,"probability":format!("{}/{}",probability.numer(),probability.denom()),
            "numerical_tolerance":delta,"twice_standard_errors":errors,
            "samples":all.len(),"requested_max_samples":max_samples,"batches":n,"batch_size":batch,
            "significant_digits":significant,"guard_digits":guard_digits,"precision_convention":symbol("significant_digits"),
            "stabilized":true,"method":symbol("adaptive_mcm"),"variables":variables,
            "engine":stamp,
        }));
    }
    Err(Error {
        code: "ConvergenceError",
        message: "adaptive MCM did not stabilize within the declared sample budget",
    })
}

/// Exact `L D L^T` factorization of a symmetric covariance, refusing a
/// matrix that is not positive semidefinite.
fn ldl<S: Scalar>(covariance: &[Vec<S>]) -> Result<(Vec<Vec<S>>, Vec<S>)> {
    let size = covariance.len();
    let mut lower = vec![vec![S::s_zero(); size]; size];
    let mut diagonal = vec![S::s_zero(); size];
    let not_psd = || invalid("joint covariance is not positive semidefinite");
    for j in 0..size {
        lower[j][j] = S::s_one();
        let reduced = (0..j).fold(S::s_zero(), |acc, k| {
            acc.add(&lower[j][k].mul(&lower[j][k]).mul(&diagonal[k]))
        });
        diagonal[j] = covariance[j][j].sub(&reduced);
        if diagonal[j].s_is_negative() {
            return Err(not_psd());
        }
        for i in j + 1..size {
            let reduced = (0..j).fold(S::s_zero(), |acc, k| {
                acc.add(&lower[i][k].mul(&lower[j][k]).mul(&diagonal[k]))
            });
            let numerator = covariance[i][j].sub(&reduced);
            match numerator.div(&diagonal[j]) {
                Some(value) => lower[i][j] = value,
                None if !numerator.s_is_zero() => return Err(not_psd()),
                None => {}
            }
        }
    }
    Ok((lower, diagonal))
}

struct JointNormal {
    covariance: Vec<Vec<Q>>,
    factor: Vec<Vec<f64>>,
}

impl JointNormal {
    fn new(definitions: &[&J], covariance: &J) -> Result<Self> {
        let size = definitions.len();
        let covariance = matrix(covariance, "joint covariance shape mismatch", 1)?;
        if covariance.len() != size || covariance[0].len() != size {
            return Err(invalid("joint covariance shape mismatch"));
        }
        if (0..size).any(|i| (0..size).any(|j| covariance[i][j] != covariance[j][i])) {
            return Err(invalid("joint covariance must be symmetric"));
        }
        for (i, definition) in definitions.iter().enumerate() {
            if symbol_name(&definition["distribution"]) != Some("normal") {
                return Err(unsupported(
                    "joint normal sampling requires normal marginal declarations",
                ));
            }
            let sd = exact(&definition["sd"])?;
            if !sd.is_positive() || sd.pow(2) != covariance[i][i] {
                return Err(invalid(
                    "normal marginal variance and joint covariance diagonal disagree",
                ));
            }
        }
        let (lower, diagonal) = ldl(&covariance)?;
        let mut factor = vec![vec![0.0; size]; size];
        let range = || invalid("joint factor exceeds binary64 range");
        for i in 0..size {
            for j in 0..size {
                if diagonal[j].is_zero() || lower[i][j].is_zero() {
                    continue;
                }
                let variance = diagonal[j]
                    .to_f64()
                    .filter(|x| x.is_finite() && *x > 0.0)
                    .ok_or_else(range)?;
                let coefficient = lower[i][j]
                    .to_f64()
                    .filter(|x| x.is_finite())
                    .ok_or_else(range)?;
                let value = coefficient * libm::sqrt(variance);
                if !value.is_finite() || value == 0.0 {
                    return Err(range());
                }
                factor[i][j] = value;
            }
        }
        Ok(Self { covariance, factor })
    }

    fn draw(&self, generator: &mut Generator, out: &mut [f64]) -> Result<()> {
        let independent = (0..out.len())
            .map(|_| generator.normal().map(|value| 0.0 + 1.0 * value))
            .collect::<Result<Vec<_>>>()?;
        for (row, value) in self.factor.iter().zip(out) {
            *value = compensated_sum(row.iter().zip(&independent).map(|(a, b)| a * b));
        }
        Ok(())
    }

    fn engine_fields(&self) -> J {
        json!({
            "joint_distribution":symbol("multivariate_normal"),
            "covariance":self.covariance.iter().map(|row|row.iter().map(|value|format!("{}/{}",value.numer(),value.denom())).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "joint_sampler":"exact-LDL-binary64-normal-v1",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_singular_joint_factors_and_invalid_covariances() {
        let pdf = json!({"distribution":"normal","location":0,"sd":"0.005"});
        for sign in [1, -1] {
            let off = Q::new(sign.into(), number(40000));
            let covariance = json!([["1/40000", tagged(&off)], [tagged(&off), "1/40000"],]);
            let sampler =
                JointNormal::new(&[&pdf, &pdf], &covariance).unwrap_or_else(|e| panic!("{e:?}"));
            let mut generator = Generator::new(&json!([12345, 23456, 34567, 45678]))
                .unwrap_or_else(|e| panic!("{e:?}"));
            let mut values = [0.0; 2];
            for _ in 0..100 {
                sampler
                    .draw(&mut generator, &mut values)
                    .unwrap_or_else(|e| panic!("{e:?}"));
                assert_eq!(values[1], sign as f64 * values[0]);
            }
            assert_eq!(generator.draws, 200);
        }
        for covariance in [
            json!([["1/40000", "1/20000"], ["1/20000", "1/40000"]]),
            json!([["1/40000", 0], [0, "1/20000"]]),
            json!([["1/40000", 1], [0, "1/40000"]]),
        ] {
            assert!(
                matches!(JointNormal::new(&[&pdf,&pdf],&covariance),Err(error) if error.code=="InvalidInput")
            );
        }
    }

    /// Case 4 of the frozen MCM fixture, switched to `gum-par`.
    fn par_params(threads: u64, max_samples: Option<u64>) -> J {
        let fixture: J = serde_json::from_str(include_str!("../../fixtures/gum-mcm.json")).unwrap();
        let mut params = fixture["cases"][4]["request"]["parameters"].clone();
        let object = params.as_object_mut().unwrap();
        object.remove("seeds");
        object.insert("engine".into(), json!("gum-par"));
        object.insert("seed".into(), json!(20261124));
        object.insert("threads".into(), json!(threads));
        if let Some(max_samples) = max_samples {
            object.insert("max_samples".into(), json!(max_samples));
        }
        params
    }

    fn par_ok(threads: u64, max_samples: Option<u64>) -> J {
        evaluate(&par_params(threads, max_samples)).unwrap_or_else(|e| panic!("{e:?}"))
    }

    /// Everything except the requested budget, which legitimately differs.
    fn without_budget(mut result: J) -> J {
        result
            .as_object_mut()
            .unwrap()
            .remove("requested_max_samples");
        result
    }

    #[test]
    fn gum_par_result_is_invariant_across_thread_counts() {
        let base = par_ok(1, None);
        // 3 and 7 do not divide the batch count; 64 exceeds it.
        for threads in [2, 3, 7, 64] {
            assert_eq!(par_ok(threads, None), base, "threads={threads}");
        }
    }

    #[test]
    fn gum_par_counter_ranges_are_disjoint_and_block_aligned() {
        let result = par_ok(3, None);
        let engine = &result["engine"];
        let stride: u128 = engine["counter_stride"].as_str().unwrap().parse().unwrap();
        assert_eq!(stride, BLOCK_COUNTER_STRIDE);
        let ranges = engine["counter_ranges"].as_array().unwrap();
        assert_eq!(ranges.len() as u64, result["batches"].as_u64().unwrap());
        for (block, range) in ranges.iter().enumerate() {
            let first: u128 = range["first"].as_str().unwrap().parse().unwrap();
            let end: u128 = range["end"].as_str().unwrap().parse().unwrap();
            assert_eq!(first, block as u128 * stride, "block {block} start");
            assert!(end > first && end <= first + stride, "block {block} end");
        }
    }

    #[test]
    fn gum_par_stopping_is_decided_only_at_block_boundaries() {
        let base = par_ok(1, None);
        let batches = base["batches"].as_u64().unwrap();
        let size = base["batch_size"].as_u64().unwrap();
        assert_eq!(base["samples"].as_u64().unwrap(), batches * size);
        let expected = without_budget(base);
        // A budget ending exactly on the stopping block, and one that adds a
        // partial block on top, both stop at the same block with equal output.
        for budget in [batches * size, batches * size + size - 1] {
            for threads in [1, 7, 64] {
                assert_eq!(
                    without_budget(par_ok(threads, Some(budget))),
                    expected,
                    "budget={budget} threads={threads}"
                );
            }
        }
        // One block short of the stopping block: never stabilizes, whatever
        // the thread count or prefetch depth.
        if batches > 2 {
            for threads in [1, 7, 64] {
                let error = evaluate(&par_params(threads, Some((batches - 1) * size)))
                    .expect_err("budget one block short must not stabilize");
                assert_eq!(error.code, "ConvergenceError", "threads={threads}");
            }
        }
    }
}
