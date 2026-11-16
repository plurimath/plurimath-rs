//! Source Math::Function::Menclose (enclosure notation) writers.
use super::{attrs, child, el, strict_slot_xml, Model, Value, WriterContext, Xml};

const SIDES: [(&str, &str); 4] = [
    ("top", "hideTop"),
    ("bottom", "hideBot"),
    ("left", "hideLeft"),
    ("right", "hideRight"),
];
const STRIKES: [(&str, &str); 4] = [
    ("horizontalstrike", "strikeH"),
    ("verticalstrike", "strikeV"),
    ("updiagonalstrike", "strikeBLTR"),
    ("downdiagonalstrike", "strikeTLBR"),
];
const MASKS: [(u32, &str); 8] = [
    (1, "top"),
    (2, "bottom"),
    (4, "left"),
    (8, "right"),
    (16, "horizontalstrike"),
    (32, "verticalstrike"),
    (64, "downdiagonalstrike"),
    (128, "updiagonalstrike"),
];
// UNICODEMATH_MENCLOSE_FUNCTIONS first key per value, mapped through
// UNARY_ARG_FUNCTIONS || UNARY_SYMBOLS.
const SYMBOLS: [(&str, &str); 8] = [
    ("bottom", "\u{2581}"),
    ("longdiv", "\u{27cc}"),
    ("updiagonalstrike downdiagonalstrike", "\u{2573}"),
    ("updiagonalstrike", "\u{2572}"),
    ("circle", "\u{2b2d}"),
    ("downdiagonalstrike", "\u{2571}"),
    ("roundedbox", "\u{25a2}"),
    ("box", "\u{25ad}"),
];

pub(super) fn handles(m: &Model) -> bool {
    m.class == "Math::Function::Menclose"
}
fn notation(m: &Model) -> Result<String, String> {
    match m.field("parameter_one") {
        Value::String(s) => Ok(s.clone()),
        other => Err(format!(
            "unported Menclose parameter_one shape (not a string notation): {other}"
        )),
    }
}
fn two(m: &Model) -> &Value {
    m.field("parameter_two")
}
fn present(v: &Value) -> bool {
    !v.is_null() && v != false
}
pub(super) fn text_writer(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let body = |ctx: &WriterContext| -> Result<String, String> {
        let v = two(m);
        if present(v) {
            child(v, f, ctx)
        } else {
            Ok(String::new())
        }
    };
    match f {
        "asciimath" | "latex" => body(ctx),
        "html" => {
            let n = notation(m)?;
            Ok(format!(
                "<menclose notation=\"{n}\">{}</menclose>",
                body(ctx)?
            ))
        }
        "unicodemath" => {
            let n = notation(m)?;
            if let Some((_, sym)) = SYMBOLS.iter().find(|(k, _)| *k == n) {
                let v = two(m);
                if !v.is_object() {
                    return Err(
                        "source_render_error: Menclose unicodemath argument has no writer".into(),
                    );
                }
                let inner = child(v, f, ctx)?;
                let wrapped = if v["class"] == "Math::Function::Fenced" {
                    inner
                } else {
                    format!("({inner})")
                };
                Ok(format!("{sym}{wrapped}"))
            } else if MASKS.iter().any(|(_, k)| n.contains(k)) {
                let mut mask = 0u32;
                for w in n.split_whitespace() {
                    if let Some((bit, _)) = MASKS.iter().find(|(_, k)| *k == w) {
                        mask += bit;
                    }
                }
                Ok(format!("▭({}&{})", mask ^ 15, body(ctx)?))
            } else {
                Ok(String::new())
            }
        }
        _ => Err(format!("unported Menclose text writer {f}")),
    }
}
fn flag(tag: &str) -> Xml {
    attrs(tag, vec![("m:val".into(), "on".into())], vec![])
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let n = notation(m)?;
    if f == "mathml" {
        let v = two(m);
        let body = if present(v) {
            strict_slot_xml(v, f, ctx, false)?
        } else {
            vec![]
        };
        return Ok(vec![attrs("menclose", vec![("notation".into(), n)], body)]);
    }
    let mut parts = vec![];
    if !["box", "circle", "roundedbox"].contains(&n.as_str()) {
        let mut pr = vec![];
        if !["box", "circle", "roundedbox"]
            .iter()
            .any(|k| n.contains(k))
        {
            for (side, tag) in SIDES {
                if !n.contains(side) {
                    pr.push(flag(&format!("m:{tag}")));
                }
            }
        }
        for (strike, tag) in STRIKES {
            if n.contains(strike) {
                pr.push(flag(&format!("m:{tag}")));
            }
        }
        parts.push(el("m:borderBoxPr", pr));
    }
    parts.push(super::portable_functions::omml_parameter(
        two(m),
        "m:e",
        ctx,
    )?);
    Ok(vec![el("m:borderBox", parts)])
}
