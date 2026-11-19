//! Source `Unitsml#to_mathml` with `unitsml: {xml: true}`: the formula row gets
//! an `xref` and the UnitsML records that unitsml-ruby `Formula#to_xml` builds.
//! Only a single unit with an optional prefix and no power is ported; every
//! other staged shape stays an unported capability.
use super::{attrs, text, Xml};
use serde_json::Value;
use unitsdb_rs::{records, Collection, Lookup};

const NS: &str = "https://schema.unitsml.org/unitsml/1.0";
const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";
// Unitsml::Utility::U2D: unit symbol, dimension, dimension symbol, order.
const U2D: [(&str, &str, &str, usize); 10] = [
    ("m", "Length", "L", 1),
    ("g", "Mass", "M", 2),
    ("kg", "Mass", "M", 2),
    ("s", "Time", "T", 3),
    ("A", "ElectricCurrent", "I", 4),
    ("K", "ThermodynamicTemperature", "Theta", 5),
    ("degK", "ThermodynamicTemperature", "Theta", 5),
    ("mol", "AmountOfSubstance", "N", 6),
    ("cd", "LuminousIntensity", "J", 7),
    ("deg", "PlaneAngle", "phi", 8),
];
// Utility.dim_id vector order.
const VECTOR: [&str; 8] = [
    "ThermodynamicTemperature",
    "AmountOfSubstance",
    "LuminousIntensity",
    "ElectricCurrent",
    "PlaneAngle",
    "Length",
    "Mass",
    "Time",
];
// Attribute order of the unitsml-ruby Dimension model.
const MODEL_ORDER: [&str; 8] = [
    "Length",
    "Mass",
    "Time",
    "ElectricCurrent",
    "ThermodynamicTemperature",
    "AmountOfSubstance",
    "LuminousIntensity",
    "PlaneAngle",
];

fn unported(message: impl std::fmt::Display) -> String {
    format!("unported promised UnitsML XML {message}")
}
fn str_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
fn identifiers(record: &Value) -> impl Iterator<Item = &str> {
    record["identifiers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|identifier| str_of(identifier, "id"))
}
fn nist(record: &Value) -> Option<&str> {
    record["identifiers"]
        .as_array()?
        .iter()
        .find(|identifier| identifier["type"] == "nist")
        .and_then(|identifier| str_of(identifier, "id"))
}
fn english(record: &Value) -> Option<&str> {
    record["names"]
        .as_array()?
        .iter()
        .find(|name| name["lang"] == "en")
        .and_then(|name| str_of(name, "value"))
}
fn unit_by_symbol(symbol: &str) -> Option<&'static Value> {
    unitsdb_rs::find(Lookup::UnitSymbol, symbol)
}
fn first_with_id(collection: Collection, id: &str) -> Option<&'static Value> {
    records(collection)
        .iter()
        .find(|record| str_of(record, "id") == Some(id) || identifiers(record).any(|i| i == id))
}
fn prefix_by_symbol(symbol: &str) -> Option<&'static Value> {
    records(Collection::Prefixes).iter().find(|prefix| {
        prefix["symbols"].as_array().is_some_and(|symbols| {
            symbols
                .iter()
                .any(|s| str_of(s, "ascii") == Some(symbol) || str_of(s, "id") == Some(symbol))
        })
    })
}
fn system_id(unit: &Value) -> String {
    unit["unit_system_reference"]
        .get(0)
        .and_then(|reference| str_of(reference, "id"))
        .unwrap_or("")
        .to_lowercase()
        .replace('-', "_")
}
fn integer(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}
fn decode(value: &str) -> Result<String, String> {
    crate::xml_parse::source_entity_decode(value).map_err(unported)
}
fn escape_amp(value: &str) -> String {
    value.replace('&', "&amp;")
}
fn leaf(name: &str, attributes: &[(&str, &str)], content: Vec<Xml>) -> Xml {
    attrs(
        name,
        attributes
            .iter()
            .map(|(k, v)| ((*k).to_owned(), escape_amp(v)))
            .collect(),
        content,
    )
}
fn text_node(value: &str) -> Xml {
    text(&escape_amp(value))
}

// An open element while reading a symbol fragment: name, attributes, children.
type Frame = (String, Vec<(String, String)>, Vec<Xml>);

/// A tag/attribute/text reader for the small UnitsDB HTML and MathML symbols.
fn fragment(input: &str) -> Result<Vec<Xml>, String> {
    let mut stack: Vec<Frame> = vec![(String::new(), vec![], vec![])];
    let mut rest = input;
    while !rest.is_empty() {
        if let Some(tag) = rest.strip_prefix('<') {
            let end = tag
                .find('>')
                .ok_or_else(|| unported(format!("symbol markup {input}")))?;
            let body = &tag[..end];
            rest = &tag[end + 1..];
            if let Some(name) = body.strip_prefix('/') {
                let (open, attributes, content) = stack
                    .pop()
                    .filter(|_| !stack.is_empty())
                    .ok_or_else(|| unported(format!("symbol markup {input}")))?;
                if open != name.trim() {
                    return Err(unported(format!("symbol markup {input}")));
                }
                stack
                    .last_mut()
                    .unwrap()
                    .2
                    .push(attrs(&open, attributes, content));
                continue;
            }
            let closed = body.ends_with('/');
            let body = body.trim_end_matches('/').trim();
            let (name, mut tail) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
            let mut attributes = vec![];
            loop {
                tail = tail.trim_start();
                if tail.is_empty() {
                    break;
                }
                let (key, value) = tail
                    .split_once('=')
                    .ok_or_else(|| unported(format!("symbol markup {input}")))?;
                let value = value.trim_start();
                let quote = value
                    .chars()
                    .next()
                    .filter(|c| matches!(*c, '"' | '\''))
                    .ok_or_else(|| unported(format!("symbol markup {input}")))?;
                let value = &value[1..];
                let close = value
                    .find(quote)
                    .ok_or_else(|| unported(format!("symbol markup {input}")))?;
                attributes.push((key.trim().to_owned(), escape_amp(&decode(&value[..close])?)));
                tail = &value[close + 1..];
            }
            if closed {
                stack
                    .last_mut()
                    .unwrap()
                    .2
                    .push(attrs(name, attributes, vec![]));
            } else {
                stack.push((name.to_owned(), attributes, vec![]));
            }
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let value = decode(&rest[..end])?;
            stack.last_mut().unwrap().2.push(text_node(&value));
            rest = &rest[end..];
        }
    }
    if stack.len() != 1 {
        return Err(unported(format!("symbol markup {input}")));
    }
    Ok(stack.pop().unwrap().2)
}

struct Base {
    dimension: &'static str,
    symbol: &'static str,
    exponent: i64,
    order: usize,
}
type Components = Vec<(String, String, i64)>;

// Utility.combine_prefixes; Err is the source UNKNOWN prefix.
fn combine_prefixes(
    a: Option<&'static Value>,
    b: Option<&'static Value>,
) -> Result<Option<&'static Value>, ()> {
    let (a, b) = match (a, b) {
        (None, other) | (other, None) => return Ok(other),
        (Some(a), Some(b)) => (a, b),
    };
    if a["base"] != b["base"] {
        return Err(());
    }
    let power = integer(&a["power"]).zip(integer(&b["power"])).ok_or(())?;
    records(Collection::Prefixes)
        .iter()
        .find(|p| p["base"] == a["base"] && integer(&p["power"]) == Some(power.0 + power.1))
        .map(Some)
        .ok_or(())
}
// Unitsml::Formula#units dimension list for one unit; None when no vector.
// Utility.decompose_unit then gather_units: repeated bases add their
// exponents and combine their prefixes.
fn bases(
    name: &str,
    unit: &Value,
    prefix: Option<&'static Value>,
) -> Result<Option<Vec<Base>>, String> {
    type Part = (String, i64, Result<Option<&'static Value>, ()>);
    let mut parts: Vec<Part> = vec![];
    if name == "g" || system_id(unit) == "si_base" {
        parts.push((name.to_owned(), 1, Ok(prefix)));
    } else if let Some(derived) = unit["si_derived_bases"]
        .as_array()
        .filter(|bases| !bases.is_empty())
    {
        for base in derived {
            let own = if base["prefix_reference"].is_null() {
                Ok(prefix)
            } else {
                str_of(&base["prefix_reference"], "id")
                    .and_then(|id| first_with_id(Collection::Prefixes, id))
                    .ok_or(())
                    .and_then(|own| combine_prefixes(Some(own), prefix))
            };
            let symbol = str_of(&base["unit_reference"], "id")
                .and_then(|id| unitsdb_rs::find(Lookup::UnitId, id))
                .and_then(|record| record["symbols"].get(0))
                .and_then(|symbol| str_of(symbol, "id"))
                .ok_or_else(|| unported(format!("derived base of {name}")))?;
            let exponent = if base["power"].is_null() {
                1
            } else {
                integer(&base["power"]).ok_or_else(|| unported(format!("base power of {name}")))?
            };
            parts.push((symbol.to_owned(), exponent, own));
        }
    } else {
        return Ok(None);
    }
    parts.sort_by(|(a, ..), (b, ..)| a.cmp(b));
    let mut gathered: Vec<Part> = vec![];
    for part in parts {
        match gathered.last_mut() {
            Some(last) if last.0 == part.0 => {
                last.1 += part.1;
                last.2 = match (last.2, part.2) {
                    (Ok(a), Ok(b)) => combine_prefixes(a, b),
                    _ => Err(()),
                };
            }
            _ => gathered.push(part),
        }
    }
    if gathered.iter().any(|part| part.2.is_err()) {
        return Ok(None);
    }
    let mut out = vec![];
    for (symbol, exponent, _) in gathered {
        let &(_, dimension, letter, order) = U2D
            .iter()
            .find(|row| row.0 == symbol)
            .ok_or_else(|| unported(format!("dimension of base {symbol}")))?;
        out.push(Base {
            dimension,
            symbol: letter,
            exponent,
            order,
        });
    }
    out.sort_by_key(|base| base.order);
    Ok(Some(out))
}
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
fn vector(power: impl Fn(&str) -> Option<i64>) -> String {
    VECTOR
        .iter()
        .map(|&name| power(name).map(|p| p.to_string()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(":")
}
fn dimension_id(bases: &[Base]) -> String {
    let wanted = vector(|name| {
        bases
            .iter()
            .find(|base| base.dimension == name)
            .map(|base| base.exponent)
    });
    for record in records(Collection::Dimensions) {
        if vector(|name| integer(&record[snake(name).as_str()]["power"])) == wanted {
            if let Some(id) = str_of(record, "id").or_else(|| nist(record)) {
                return id.to_owned();
            }
        }
    }
    let mut id = String::from("D_");
    for base in bases {
        id.push_str(base.symbol);
        if base.exponent != 1 {
            id.push_str(&base.exponent.to_string());
        }
    }
    id
}
fn unit_dimension(unit: Option<&Value>) -> Option<&'static str> {
    let quantity = str_of(unit?["quantity_references"].get(0)?, "id")?;
    let record = first_with_id(Collection::Quantities, quantity)?;
    str_of(&record["dimension_reference"], "id")
}
fn dimension_node(id: &str, components: &Components) -> Xml {
    let children = MODEL_ORDER
        .iter()
        .filter_map(|name| components.iter().find(|c| c.0 == *name))
        .map(|(name, symbol, power)| {
            leaf(
                name,
                &[
                    ("symbol", symbol.as_str()),
                    ("powerNumerator", power.to_string().as_str()),
                ],
                vec![],
            )
        })
        .collect();
    leaf("Dimension", &[("xmlns", NS), ("id", id)], children)
}
fn record_components(id: &str) -> Result<Components, String> {
    let record = first_with_id(Collection::Dimensions, id)
        .and_then(Value::as_object)
        .ok_or_else(|| unported(format!("dimension {id}")))?;
    let mut components = vec![];
    for (key, value) in record {
        let name: String = key
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect();
        if !value.is_object() || !VECTOR.contains(&name.as_str()) {
            continue;
        }
        let symbol = str_of(value, "symbol").ok_or_else(|| unported(format!("dimension {id}")))?;
        let power = integer(&value["power"]).ok_or_else(|| unported(format!("dimension {id}")))?;
        components.push((name, symbol.to_owned(), power));
    }
    Ok(components)
}

/// The `xref` value and the UnitsML nodes appended after the formula row.
pub(super) fn nodes(input: &str) -> Result<(String, Vec<Xml>), String> {
    let staged = crate::units_parse::staged(input).map_err(unported)?;
    let fields = staged
        .as_object()
        .filter(|fields| fields.keys().all(|k| k == "unitName" || k == "prefix"))
        .ok_or_else(|| unported("for more than one bare or prefixed unit"))?;
    let name = fields
        .get("unitName")
        .and_then(Value::as_str)
        .ok_or_else(|| unported("without a unit"))?;
    let prefix_name = match fields.get("prefix") {
        None | Some(Value::Null) => None,
        Some(prefix) => {
            Some(str_of(prefix, "prefixName").ok_or_else(|| unported("prefix without a name"))?)
        }
    };
    let prefix = match prefix_name {
        Some(p) => Some(prefix_by_symbol(p).ok_or_else(|| unported(format!("prefix {p}")))?),
        None => None,
    };
    let unit = unit_by_symbol(name).ok_or_else(|| unported(format!("unit {name}")))?;
    let norm = format!("{}{name}", prefix_name.unwrap_or(""));
    let bases = bases(name, unit, prefix)?;
    let named = unit_by_symbol(&norm);
    let stripped = norm.replace(['(', ')'], "");
    let id = match unit_by_symbol(&stripped).and_then(nist) {
        Some(id) => format!("U_{}", id.replace('\'', "_")),
        None => format!("U_{}", stripped.replace('*', ".").replace('^', "")),
    };
    let system = system_id(unit);
    let unit_system = if !["si_base", "si_derived_special", "si_derived_non_special"]
        .contains(&system.as_str())
    {
        [("name", "not_SI"), ("type", "not_SI"), ("lang", "en")]
    } else if system == "si_base" || (name == "g" && prefix_name == Some("k")) {
        [("name", "SI"), ("type", "SI_base"), ("lang", "en")]
    } else {
        [("name", "SI"), ("type", "SI_derived"), ("lang", "en")]
    };
    let symbol = unit["symbols"]
        .as_array()
        .and_then(|symbols| symbols.iter().find(|s| str_of(s, "id") == Some(name)))
        .ok_or_else(|| unported(format!("symbol {name}")))?;
    let prefix_symbol = match prefix {
        Some(p) => Some(
            p["symbols"]
                .as_array()
                .and_then(|symbols| symbols.last())
                .ok_or_else(|| unported("prefix without symbols"))?,
        ),
        None => None,
    };
    let prefix_html = prefix_symbol.and_then(|s| str_of(s, "html")).unwrap_or("");
    let html = fragment(&format!(
        "{prefix_html}{}",
        str_of(symbol, "html").unwrap_or("")
    ))?;
    let mut mathml = fragment(str_of(symbol, "mathml").unwrap_or(""))?;
    if prefix.is_some() {
        let [Xml::El(tag, attributes, content)] = mathml.as_slice() else {
            return Err(unported(format!("prefixed composite symbol {name}")));
        };
        let mut value = decode(prefix_html)?;
        for child in content {
            let Xml::Text(t) = child else {
                return Err(unported(format!("prefixed composite symbol {name}")));
            };
            value.push_str(&t.replace("&amp;", "&"));
        }
        let replaced = vec![attrs(tag, attributes.clone(), vec![text_node(&value)])];
        mathml = replaced;
    }
    let vector_id = bases.as_deref().map(dimension_id);
    let mut unit_attributes = vec![("xmlns", NS)];
    let url = vector_id.as_ref().map(|d| format!("#{d}"));
    if let Some(url) = &url {
        unit_attributes.push(("dimensionURL", url.as_str()));
    }
    unit_attributes.push(("id", id.as_str()));
    let mut children = vec![
        leaf("UnitSystem", &unit_system, vec![]),
        leaf(
            "UnitName",
            &[("lang", "en")],
            vec![text_node(named.and_then(english).unwrap_or(&norm))],
        ),
        leaf("UnitSymbol", &[("type", "HTML")], html),
        leaf(
            "UnitSymbol",
            &[("type", "MathMl")],
            vec![leaf(
                "math",
                &[("xmlns", MATHML_NS), ("display", "block")],
                mathml,
            )],
        ),
    ];
    if let Some(prefix_name) = prefix_name {
        children.push(leaf(
            "RootUnits",
            &[],
            vec![leaf(
                "EnumeratedRootUnit",
                &[
                    ("unit", english(unit).unwrap_or("")),
                    ("prefix", prefix_name),
                ],
                vec![],
            )],
        ));
    }
    let mut out = vec![leaf("Unit", &unit_attributes, children)];
    if let (Some(p), Some(s)) = (prefix, prefix_symbol) {
        let base = p["base"].to_string();
        let power = p["power"].to_string();
        let mut symbols = vec![leaf(
            "PrefixName",
            &[("lang", "en")],
            vec![text_node(english(p).unwrap_or(""))],
        )];
        for (kind, key) in [
            ("ASCII", "ascii"),
            ("unicode", "unicode"),
            ("LaTeX", "latex"),
            ("HTML", "html"),
        ] {
            let content = match str_of(s, key).filter(|v| !v.is_empty()) {
                Some(v) => vec![text_node(&decode(v)?)],
                None => vec![],
            };
            symbols.push(leaf("PrefixSymbol", &[("type", kind)], content));
        }
        out.push(leaf(
            "Prefix",
            &[
                ("xmlns", NS),
                ("prefixBase", base.as_str()),
                ("prefixPower", power.as_str()),
                ("id", nist(p).unwrap_or("")),
            ],
            symbols,
        ));
    }
    let mut dimensions: Vec<(String, Components)> = vec![];
    let norm_dimension = unit_dimension(named);
    if let Some(dimension) = norm_dimension {
        dimensions.push((dimension.to_owned(), record_components(dimension)?));
    }
    if let (Some(bases), Some(vector_id)) = (&bases, &vector_id) {
        let components: Components = bases
            .iter()
            .map(|b| (b.dimension.to_owned(), b.symbol.to_owned(), b.exponent))
            .collect();
        let sorted = |c: &Components| {
            MODEL_ORDER
                .iter()
                .filter_map(|name| c.iter().find(|x| x.0 == *name).cloned())
                .collect::<Components>()
        };
        if !dimensions
            .iter()
            .any(|(i, c)| i == vector_id && sorted(c) == sorted(&components))
        {
            dimensions.push((vector_id.clone(), components));
        }
    }
    out.extend(dimensions.iter().map(|(i, c)| dimension_node(i, c)));
    if let Some(unit) = named {
        if let Some([reference]) = unit["quantity_references"].as_array().map(Vec::as_slice) {
            let quantity = str_of(reference, "id").ok_or_else(|| unported("quantity reference"))?;
            let url = norm_dimension.map(|d| format!("#{d}"));
            let mut attributes = vec![("xmlns", NS), ("id", quantity), ("quantityType", "base")];
            if let Some(url) = &url {
                attributes.push(("dimensionURL", url.as_str()));
            }
            let names = first_with_id(Collection::Quantities, quantity)
                .and_then(|q| q["names"].as_array())
                .into_iter()
                .flatten()
                .filter(|n| n["lang"] == "en")
                .filter_map(|n| str_of(n, "value"))
                .map(|n| leaf("QuantityName", &[("lang", "en")], vec![text_node(n)]))
                .collect();
            out.push(leaf("Quantity", &attributes, names));
        }
    }
    Ok((id, out))
}
