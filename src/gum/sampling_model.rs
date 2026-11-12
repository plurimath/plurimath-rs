use super::taylor::{compile_polynomial, sampling_polynomial, Polynomial, Taylor};
use super::*;
use num_traits::{One, ToPrimitive};
use std::collections::BTreeMap;

pub(super) enum Model {
    Polynomial(Polynomial),
    Rational {
        offset: Q,
        numerator: Polynomial,
        denominator: Polynomial,
        center: f64,
        certificate: Box<[Q; 2]>,
    },
}

impl Model {
    pub(super) fn build(
        ast: &J,
        names: &[&str],
        centers: Vec<Q>,
        definitions: &[&J],
    ) -> Result<Self> {
        match sampling_polynomial(ast, names, centers.clone()) {
            Ok(polynomial) => Ok(Self::Polynomial(polynomial)),
            Err(error)
                if error.message == "rational sampling models are outside this profile"
                    || error.code == "DivisionByZeroError" =>
            {
                let variables = names
                    .iter()
                    .copied()
                    .zip(&centers)
                    .enumerate()
                    .map(|(i, (name, center))| {
                        (name, Fraction::new(Taylor::variable(center.clone(), i)))
                    })
                    .collect();
                let fraction = walk(ast, &variables)?;
                let supports = definitions
                    .iter()
                    .zip(centers)
                    .map(|(definition, center)| {
                        if symbol_name(&definition["distribution"]) == Some("rectangular") {
                            Ok(Some([
                                exact(&definition["lower"])? - &center,
                                exact(&definition["upper"])? - center,
                            ]))
                        } else {
                            Ok(None)
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                let certificate = interval(&fraction.denominator, &supports)?;
                if !(certificate[0].is_positive() || certificate[1].is_negative()) {
                    return Err(unsupported("denominator support certificate includes zero"));
                }
                let divisor = fraction.denominator.derivative(&[]);
                if divisor.is_zero() {
                    return Err(unsupported("denominator center outside binary64 range"));
                }
                let offset = fraction.numerator.derivative(&[]) / divisor;
                let residual = fraction
                    .numerator
                    .add(fraction.denominator.clone().scale(&-&offset));
                let numerator = compile_polynomial(residual, names.len())?;
                let denominator = compile_polynomial(fraction.denominator, names.len())?;
                let center = denominator
                    .offset
                    .to_f64()
                    .filter(|v| v.is_finite() && *v != 0.0)
                    .ok_or_else(|| unsupported("denominator center outside binary64 range"))?;
                if numerator.degrees.iter().all(|n| *n == 0) {
                    return Err(unsupported("constant models need exact analysis rather than adaptive uncertainty tolerance"));
                }
                Ok(Self::Rational {
                    offset,
                    numerator,
                    denominator,
                    center,
                    certificate: Box::new(certificate),
                })
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn offset(&self) -> &Q {
        match self {
            Self::Polynomial(p) => &p.offset,
            Self::Rational { offset, .. } => offset,
        }
    }

    pub(super) fn degrees(&self) -> &[usize] {
        match self {
            Self::Polynomial(p) => &p.degrees,
            Self::Rational { numerator, .. } => &numerator.degrees,
        }
    }

    pub(super) fn evaluate(&self, deviations: &[f64]) -> Result<f64> {
        match self {
            Self::Polynomial(p) => Ok(p.centered_value(deviations)),
            Self::Rational {
                numerator,
                denominator,
                center,
                ..
            } => {
                let divisor = center + denominator.centered_value(deviations);
                if !divisor.is_finite() || divisor == 0.0 {
                    return Err(Error {
                        code: "NumericalFailure",
                        message: "sampled denominator lost its nonzero certificate in binary64",
                    });
                }
                Ok(numerator.centered_value(deviations) / divisor)
            }
        }
    }

    pub(super) fn engine_fields(&self) -> J {
        match self {
            Self::Polynomial(_) => json!({"numeric_backend":"binary64-centered-exact-polynomial"}),
            Self::Rational { certificate, .. } => json!({
                "numeric_backend":"binary64-centered-certified-rational",
                "denominator_interval":certificate.iter().map(|q|format!("{}/{}",q.numer(),q.denom())).collect::<Vec<_>>(),
                "certificate":"exact-rectangular-support-interval-v1",
            }),
        }
    }
}

#[derive(Clone)]
struct Fraction {
    numerator: Taylor,
    denominator: Taylor,
}

impl Fraction {
    fn new(numerator: Taylor) -> Self {
        Self {
            numerator,
            denominator: Taylor::constant(Q::one()),
        }
    }

    fn add(self, rhs: Self) -> Result<Self> {
        Ok(Self {
            numerator: self
                .numerator
                .multiply::<false>(&rhs.denominator)?
                .add(rhs.numerator.multiply::<false>(&self.denominator)?),
            denominator: self.denominator.multiply::<false>(&rhs.denominator)?,
        })
    }

    fn multiply(self, rhs: Self) -> Result<Self> {
        Ok(Self {
            numerator: self.numerator.multiply::<false>(&rhs.numerator)?,
            denominator: self.denominator.multiply::<false>(&rhs.denominator)?,
        })
    }

    fn reciprocal(self) -> Result<Self> {
        if self.numerator.0.is_empty() {
            return Err(Error {
                code: "DivisionByZeroError",
                message: "sampling denominator is identically zero",
            });
        }
        Ok(Self {
            numerator: self.denominator,
            denominator: self.numerator,
        })
    }

    fn power(self, rhs: Self) -> Result<Self> {
        if rhs
            .numerator
            .0
            .keys()
            .chain(rhs.denominator.0.keys())
            .any(|key| !key.is_empty())
        {
            return Err(unsupported(
                "finite output moments are not established for non-polynomial sampling models",
            ));
        }
        let divisor = rhs.denominator.derivative(&[]);
        if divisor.is_zero() {
            return Err(invalid("zero sampling exponent denominator"));
        }
        let power = rhs.numerator.derivative(&[]) / divisor;
        let power = power
            .to_integer()
            .to_i32()
            .filter(|n| power.is_integer() && n.unsigned_abs() <= 32)
            .ok_or_else(|| unsupported("sampling requires integer powers within -32..32"))?;
        let positive = Taylor::constant(Q::from_integer(power.unsigned_abs().into()));
        let fraction = Self {
            numerator: self.numerator.power::<false>(positive.clone())?,
            denominator: self.denominator.power::<false>(positive)?,
        };
        if power < 0 {
            fraction.reciprocal()
        } else {
            Ok(fraction)
        }
    }
}

fn walk(node: &J, variables: &BTreeMap<&str, Fraction>) -> Result<Fraction> {
    match node["type"].as_str() {
        Some("number") => Ok(Fraction::new(Taylor::constant(exact(&node["text"])?))),
        Some("variable") => variables
            .get(node["name"].as_str().unwrap_or(""))
            .cloned()
            .ok_or(Error {
                code: "MissingVariableError",
                message: "missing analytic variable binding",
            }),
        Some("unary") => {
            let mut value = walk(&node["operand"], variables)?;
            if node["operator"] == "-" {
                value.numerator = value.numerator.scale(&-Q::one());
            }
            Ok(value)
        }
        Some("binary") => {
            let left = walk(&node["left"], variables)?;
            let mut right = walk(&node["right"], variables)?;
            match node["operator"].as_str() {
                Some("+") => left.add(right),
                Some("-") => {
                    right.numerator = right.numerator.scale(&-Q::one());
                    left.add(right)
                }
                Some("*") => left.multiply(right),
                Some("/") => left.multiply(right.reciprocal()?),
                Some("^") => left.power(right),
                _ => Err(invalid("invalid sampling AST")),
            }
        }
        _ => Err(unsupported("analytic derivatives unavailable for calls")),
    }
}

fn interval(polynomial: &Taylor, supports: &[Option<[Q; 2]>]) -> Result<[Q; 2]> {
    let mut total = [Q::zero(), Q::zero()];
    for (indices, coefficient) in &polynomial.0 {
        let mut term = [coefficient.clone(), coefficient.clone()];
        let mut powers = BTreeMap::<usize, i32>::new();
        for &index in indices {
            *powers.entry(index).or_default() += 1;
        }
        for (index, power) in powers {
            let [lower, upper] = supports[index].as_ref().ok_or_else(|| {
                unsupported(
                "finite output moments require finite certified rectangular denominator support")
            })?;
            let a = lower.pow(power);
            let b = upper.pow(power);
            let low = if power % 2 == 0 && lower <= &Q::zero() && upper >= &Q::zero() {
                Q::zero()
            } else {
                a.clone().min(b.clone())
            };
            let high = a.max(b);
            let products = [
                &term[0] * &low,
                &term[0] * &high,
                &term[1] * &low,
                &term[1] * &high,
            ];
            term = [
                products.iter().min().expect("four bounds").clone(),
                products.iter().max().expect("four bounds").clone(),
            ];
        }
        total[0] += &term[0];
        total[1] += &term[1];
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reciprocal() -> J {
        json!({"type":"binary","operator":"/","left":{"type":"number","text":"1"},"right":{"type":"variable","name":"x"}})
    }

    #[test]
    fn denominator_certificates_refuse_zero_and_unbounded_support() {
        for (center, pdf, message) in [
            (
                Q::zero(),
                json!({"distribution":"rectangular","lower":-1,"upper":1}),
                "denominator support certificate includes zero",
            ),
            (
                Q::one(),
                json!({"distribution":"normal","location":1,"sd":"0.01"}),
                "finite output moments require finite certified rectangular denominator support",
            ),
        ] {
            let result = Model::build(&reciprocal(), &["x"], vec![center], &[&pdf]);
            assert!(
                matches!(result,Err(error) if error.code=="UnsupportedModel" && error.message==message)
            );
        }
        let pdf = json!({"distribution":"rectangular","lower":1,"upper":2});
        let model = Model::build(
            &reciprocal(),
            &["x"],
            vec![Q::new(number(3), number(2))],
            &[&pdf],
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(
            model.engine_fields()["denominator_interval"],
            json!(["1/1", "2/1"])
        );
    }
}
