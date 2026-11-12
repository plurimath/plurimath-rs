use super::{attrs, el, text, value_text, Model, Xml};
pub(super) fn xml_writer(m: &Model, f: &str) -> Result<Vec<Xml>, String> {
    let v = m.field("value");
    let s = value_text(v)?;
    if f == "omml" {
        if s == "&#x2062;" {
            return Ok(vec![]);
        }
        return Ok(vec![el(
            "m:r",
            vec![el("m:t", if v.is_null() { vec![] } else { vec![text(&s)] })],
        )]);
    }
    let mut a = vec![];
    if s.contains("&#x2147;") {
        a.push(("intent".into(), super::text_content::decode(&s)?))
    }
    let options = m.field("options");
    if !options.is_null() && !options.is_object() {
        return Err("source_render_error: symbol options cannot receive rspace lookup".into());
    }
    let r = &options["rspace"];
    if !r.is_null() {
        a.push((
            "rspace".into(),
            r.as_str()
                .ok_or("unported non-string symbol rspace")?
                .to_owned(),
        ))
    }
    Ok(vec![attrs(
        if r.is_null() { "mi" } else { "mo" },
        a,
        if v.is_null() || ["{:", ":}"].contains(&s.as_str()) {
            vec![]
        } else {
            vec![text(&s)]
        },
    )])
}
