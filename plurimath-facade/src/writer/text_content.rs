//! Source Text/Mbox and Left/Right handlers. Literal-sensitive branches are not templates.
use super::{attrs, el, present, text, Model, Xml};
use serde_json::Value;
fn error(s: &str) -> String {
    format!("source_render_error: {s}")
}
fn raw(v: &Value) -> Result<String, String> {
    match v {
        Value::Null => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        Value::Bool(v) => Ok(v.to_string()),
        Value::Number(n) => {
            let s = n.to_string();
            if s.chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_digit() || (i == 0 && c == '-'))
            {
                Ok(s)
            } else {
                Err("unported source floating-point interpolation".into())
            }
        }
        _ => Err("unported source nondeterministic object interpolation".into()),
    }
}
fn string(v: &Value) -> Result<String, String> {
    if v.is_null() {
        return Ok(String::new());
    }
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| error("text parameter must be a string"))
}
pub(super) fn decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut out = String::new();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos] != b'&' {
            let c = value[pos..].chars().next().unwrap();
            out.push(c);
            pos += c.len_utf8();
            continue;
        }
        let start = pos;
        let mut at = pos + 1;
        let mut base = 0;
        if at < bytes.len() && bytes[at] == b'#' {
            at += 1;
            base = 10;
            if at < bytes.len() && (bytes[at] == b'x' || bytes[at] == b'X') {
                base = 16;
                at += 1
            }
        }
        let first = at;
        if base == 0 {
            if at < bytes.len() && bytes[at].is_ascii_alphabetic() {
                at += 1;
                while at < bytes.len() && bytes[at].is_ascii_alphanumeric() {
                    at += 1
                }
            }
        } else {
            let max = if base == 16 { 6 } else { 7 };
            while at < bytes.len()
                && at - first < max
                && if base == 16 {
                    bytes[at].is_ascii_hexdigit()
                } else {
                    bytes[at].is_ascii_digit()
                }
            {
                at += 1
            }
        }
        if at == first
            || at == bytes.len()
            || !(bytes[at] == b';' || bytes[at] == b'\n' || bytes[at] == b'<')
        {
            out.push('&');
            pos = start + 1;
            continue;
        }
        let digits = &value[first..at];
        let decoded = if base == 0 {
            crate::symbols::tables()["html_entities"][digits]
                .as_u64()
                .map(|v| v as u32)
        } else {
            u32::from_str_radix(digits, base).ok()
        };
        if let Some(code) = decoded {
            let c = char::from_u32(code)
                .ok_or_else(|| error("entity codepoint is outside source UTF-8 range"))?;
            out.push(c);
            pos = at + usize::from(bytes[at] == b';')
        } else {
            out.push('&');
            pos = start + 1
        }
    }
    Ok(out)
}
fn tokens(value: &str, f: &str) -> String {
    let mut out = String::new();
    let mut remaining = value;
    while let Some(at) = remaining.find("unicode[:") {
        out.push_str(&remaining[..at]);
        let rest = &remaining[at + 9..];
        let Some(end) = rest.find(']') else {
            out.push_str(&remaining[at..]);
            return out;
        };
        let name = &rest[..end];
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            out.push_str("unicode[:");
            remaining = rest;
            continue;
        }
        if ["asciimath", "latex"].contains(&f) {
            out.push_str(name)
        } else {
            let a = crate::symbols::tables()["text_unicode_invert"][name].as_str();
            let b = crate::symbols::tables()["text_symbols_invert"][name].as_str();
            out.push_str(a.or(b).unwrap_or(""))
        }
        remaining = &rest[end + 1..]
    }
    out.push_str(remaining);
    out
}
fn omml_value(v: &Value) -> Result<String, String> {
    let value = string(v)?.replace(' ', "&#xa0;");
    let decoded = decode(&value)?;
    let mut out = String::new();
    for c in decoded.chars() {
        let n = c as u32;
        if !(0x20..=0x7e).contains(&n) || ['"', '&', '\'', '<', '>'].contains(&c) {
            out.push_str(&format!("&#x{n:x};"))
        } else {
            out.push(c)
        }
    }
    Ok(tokens(&out, "omml"))
}
pub(super) fn text_writer(m: &Model, f: &str) -> Result<String, String> {
    let v = m.field("parameter_one");
    let mbox = m.kind() == "Mbox";
    if mbox && f == "latex" {
        return Ok(format!("\\mbox{{{}}}", raw(v)?));
    }
    if mbox && f == "html" {
        return raw(v);
    }
    if ["Left", "Right"].contains(&m.kind()) {
        let name = m.kind().to_lowercase();
        return Ok(match f {
            "asciimath" => format!("{name}{}", raw(v)?),
            "html" => format!("<i>{}</i>", raw(v)?),
            "latex" => {
                let token = v
                    .as_str()
                    .and_then(|x| crate::symbols::tables()["latex_left_right"][x].as_str())
                    .unwrap_or(".");
                format!("\\{name} {token}")
            }
            "unicodemath" => raw(v)?,
            _ => unreachable!(),
        });
    }
    match f {
        "asciimath" => Ok(format!("\"{}\"", tokens(&string(v)?, f))),
        "latex" => Ok(format!("\\text{{{}}}", tokens(&string(v)?, f))),
        "html" => Ok(tokens(&string(v)?, f)),
        "unicodemath" => {
            if !present(v) {
                return Ok(String::new());
            }
            let s = string(v)?;
            if s.starts_with('\\') {
                Ok(s)
            } else {
                Ok(format!("\"{}\"", decode(&s)?))
            }
        }
        _ => unreachable!(),
    }
}
pub(super) fn xml_writer(m: &Model, f: &str) -> Result<Vec<Xml>, String> {
    let v = m.field("parameter_one");
    if ["Left", "Right"].contains(&m.kind()) {
        if f == "mathml" {
            if !present(v) {
                return Ok(vec![el("mo", vec![])]);
            }
            let mut s = string(v)?;
            if (m.kind() == "Left" && s == "\\{") || (m.kind() == "Right" && s == "\\}") {
                s.remove(0);
            }
            return Ok(vec![el("mo", vec![text(&s)])]);
        }
        return Ok(vec![if present(v) {
            el("m:r", vec![el("m:t", vec![text(&string(v)?)])])
        } else {
            el("m:r", vec![])
        }]);
    }
    if f == "mathml" {
        return Ok(vec![if present(v) {
            el("mtext", vec![text(&tokens(&string(v)?, f))])
        } else {
            el("mtext", vec![])
        }]);
    }
    let t = el("m:t", vec![text(&omml_value(v)?)]);
    if m.kind() == "Mbox" {
        return Ok(vec![t]);
    }
    let lang = m.field("lang");
    let omml = lang == "omml" || lang["string"] == "omml";
    let mut c = vec![];
    if !omml {
        c.push(el(
            "m:rPr",
            vec![attrs("m:sty", vec![("m:val".into(), "p".into())], vec![])],
        ))
    }
    c.push(t);
    Ok(vec![el("m:r", c)])
}

pub(super) fn direct_omml(m: &Model) -> Result<Vec<Xml>, String> {
    Ok(vec![el(
        "m:t",
        vec![text(&omml_value(m.field("parameter_one"))?)],
    )])
}
