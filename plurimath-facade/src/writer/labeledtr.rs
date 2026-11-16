//! Source Math::Function::Mlabeledtr: BinaryFunction defaults plus the labeled-row overrides.
use super::{child, el, portable_functions as pf, Model, Value, WriterContext, Xml};

pub(super) fn handles(m: &Model) -> bool {
    m.class == "Math::Function::Mlabeledtr"
}
fn present(v: &Value) -> bool {
    !v.is_null() && v != false
}
fn slot(v: &Value, f: &str, ctx: &WriterContext) -> Result<String, String> {
    if !v.is_object() {
        return Err("source_render_error: Mlabeledtr argument has no writer".into());
    }
    child(v, f, ctx)
}
pub(super) fn text_writer(m: &Model, f: &str, ctx: &WriterContext) -> Result<String, String> {
    let (a, b) = (m.field("parameter_one"), m.field("parameter_two"));
    match f {
        "asciimath" => Ok(format!(
            "mlabeledtr{}{}",
            pf::wrapped(a, ctx)?,
            pf::wrapped(b, ctx)?
        )),
        "latex" => {
            let part = |v: &Value| -> Result<String, String> {
                if present(v) {
                    pf::latex_wrapped(v, ctx)
                } else {
                    Ok(String::new())
                }
            };
            Ok(format!("\\mlabeledtr{}{}", part(a)?, part(b)?))
        }
        "html" => {
            let part = |v: &Value| -> Result<String, String> {
                if present(v) {
                    Ok(format!("<i>{}</i>", slot(v, f, ctx)?))
                } else {
                    Ok(String::new())
                }
            };
            Ok(format!("{}{}", part(a)?, part(b)?))
        }
        "unicodemath" => {
            let first = if present(a) {
                slot(a, f, ctx)?
            } else {
                String::new()
            };
            let label = if !present(b) {
                String::new()
            } else if b["class"] == "Math::Function::Text" {
                match &b["fields"]["parameter_one"] {
                    Value::String(s) => s.clone(),
                    other => return Err(format!("unported Mlabeledtr label value shape: {other}")),
                }
            } else {
                return Err("unported Mlabeledtr label: parameter_two is not Text".into());
            };
            Ok(format!("{first}#{label}"))
        }
        _ => Err(format!("unported Mlabeledtr text writer {f}")),
    }
}
pub(super) fn xml(m: &Model, f: &str, ctx: &WriterContext) -> Result<Vec<Xml>, String> {
    let (a, b) = (m.field("parameter_one"), m.field("parameter_two"));
    if f == "mathml" {
        let td = |v: &Value| -> Result<Xml, String> {
            if !v.is_object() {
                return Err("source_render_error: Mlabeledtr argument has no writer".into());
            }
            Ok(el("mtd", super::strict_slot_xml(v, f, ctx, false)?))
        };
        return Ok(vec![el(
            "mtable",
            vec![el("mlabeledtr", vec![td(b)?, td(a)?])],
        )]);
    }
    let mut body = vec![];
    for v in [a, b] {
        if present(v) {
            body.extend(super::multiscripts::xml_child(v, "omml", ctx)?);
        }
    }
    Ok(vec![el("m:r", body)])
}
