//! The six Number writers' formatter branches: `TextRenderer` (asciimath,
//! latex, html, unicodemath), `MathmlRenderer`, `OmmlRenderer`, plus the
//! `t_tag`/`nary_attr_value` plain `to_s` path.

use super::render::{Formatted, FormattedNotation, FormattedNumber, NotationStyle};

/// Structural XML mirroring the parent writer's private
/// `Xml::{El(name, attrs, children), Text(text)}` one-to-one. Names carry
/// their namespace prefix (`m:t`); text is unescaped (the parent's dump
/// escapes).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FmtXml {
    El(String, Vec<(String, String)>, Vec<FmtXml>),
    Text(String),
}

fn el(name: &str, children: Vec<FmtXml>) -> FmtXml {
    FmtXml::El(name.to_string(), Vec::new(), children)
}
fn leaf(name: &str, text: &str) -> FmtXml {
    el(name, vec![FmtXml::Text(text.to_string())])
}

impl FmtXml {
    /// Compact serialization (escapes `&`, `<`, `>`), for tests/diagnostics.
    pub(crate) fn to_compact(&self) -> String {
        match self {
            FmtXml::Text(t) => t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"),
            FmtXml::El(n, a, c) => {
                let attrs: String = a.iter().map(|(k, v)| format!(" {k}=\"{v}\"")).collect();
                if c.is_empty() {
                    format!("<{n}{attrs}/>")
                } else {
                    format!("<{n}{attrs}>{}</{n}>", c.iter().map(FmtXml::to_compact).collect::<String>())
                }
            }
        }
    }
}

/// Which Number writer method is being served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TargetWriter {
    /// `Number#to_asciimath` → `TextRenderer.render(result, :asciimath)`
    AsciiMath,
    /// `Number#to_latex`
    Latex,
    /// `Number#to_html`
    Html,
    /// `Number#to_unicodemath` (mini_sub/mini_sup handled by the parent first)
    UnicodeMath,
    /// `Number#to_mathml_without_math_tag` → `MathmlRenderer.render`
    MathMl,
    /// `Number#to_omml_without_math_tag` → `[OmmlRenderer.render]`
    Omml,
    /// `Number#t_tag` / `insert_t_tag` / `font_style_t_tag` /
    /// `nary_attr_value`: `format_value_with_options(options).to_s`
    PlainText,
}

/// Writer output: text for text writers / `PlainText`, one XML node for
/// MathML (`mn`/`msub`/`mrow`) and OMML (`m:t`/`m:sSub`/`m:sSup`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum WriterOutput {
    Text(String),
    Xml(FmtXml),
}

pub(crate) fn render_for_writer(result: &Formatted, writer: TargetWriter) -> WriterOutput {
    match writer {
        TargetWriter::AsciiMath => WriterOutput::Text(text(result, "asciimath")),
        TargetWriter::Latex => WriterOutput::Text(text(result, "latex")),
        TargetWriter::Html => WriterOutput::Text(text(result, "html")),
        TargetWriter::UnicodeMath => WriterOutput::Text(text(result, "unicodemath")),
        TargetWriter::MathMl => WriterOutput::Xml(mathml(result)),
        TargetWriter::Omml => WriterOutput::Xml(omml(result)),
        TargetWriter::PlainText => WriterOutput::Text(result.to_string()),
    }
}

/// `TextRenderer.render`.
fn text(result: &Formatted, format: &str) -> String {
    let n = match result {
        Formatted::Number(n) if n.has_base_notation() => n,
        other => return other.to_string(),
    };
    if n.base_notation.is_literal() || !n.base_notation.is_semantic() {
        return n.to_string();
    }
    let sign = n.sign_text().unwrap_or("");
    let digits = n.digits_string();
    let base = n.base_notation.base;
    match format {
        "latex" => format!("{sign}\\mathrm{{{digits}}}_{{{base}}}"),
        "html" => format!("{sign}{digits}<sub>{base}</sub>"),
        _ => format!("{sign}{digits}_({base})"),
    }
}

/// `MathmlRenderer.render`.
fn mathml(result: &Formatted) -> FmtXml {
    match result {
        Formatted::Notation(n) => mathml_notation(n),
        Formatted::Number(n) if n.base_notation.is_semantic() => mathml_semantic_base(n),
        other => leaf("mn", &other.to_string()),
    }
}

fn mathml_notation(n: &FormattedNotation) -> FmtXml {
    if n.notation_style == NotationStyle::E {
        return leaf("mn", &n.to_string());
    }
    el(
        "mrow",
        vec![
            leaf("mn", &n.coefficient.to_string()),
            leaf("mo", &n.times_symbol),
            el("msup", vec![leaf("mn", "10"), leaf("mn", &n.formatted_exponent())]),
        ],
    )
}

fn mathml_semantic_base(n: &FormattedNumber) -> FmtXml {
    let sub = el("msub", vec![leaf("mn", &n.digits_string()), leaf("mn", &n.base_notation.base.to_string())]);
    match n.sign_text() {
        None => sub,
        Some(sign) => el("mrow", vec![leaf("mo", sign), sub]),
    }
}

/// `XmlHelper.pr_element("ctrl", true, namespace: "m")` — identical to the
/// parent writer's `control_properties()`.
fn ctrl_pr() -> FmtXml {
    el(
        "m:ctrlPr",
        vec![el(
            "w:rPr",
            vec![
                FmtXml::El(
                    "w:rFonts".into(),
                    vec![("w:ascii".into(), "Cambria Math".into()), ("w:hAnsi".into(), "Cambria Math".into())],
                    vec![],
                ),
                el("w:i", vec![]),
            ],
        )],
    )
}

fn run(text: &str) -> FmtXml {
    el("m:r", vec![leaf("m:t", text)])
}

/// `OmmlRenderer.render`.
fn omml(result: &Formatted) -> FmtXml {
    match result {
        Formatted::Notation(n) if n.notation_style != NotationStyle::E => el(
            "m:sSup",
            vec![
                el("m:sSupPr", vec![ctrl_pr()]),
                el(
                    "m:e",
                    vec![run(&n.coefficient.to_string()), run(&format!(" {} ", n.times_symbol)), run("10")],
                ),
                el("m:sup", vec![run(&n.formatted_exponent())]),
            ],
        ),
        Formatted::Number(n) if n.base_notation.is_semantic() => el(
            "m:sSub",
            vec![
                el("m:sSubPr", vec![ctrl_pr()]),
                el("m:e", vec![run(&format!("{}{}", n.sign_text().unwrap_or(""), n.digits_string()))]),
                el("m:sub", vec![run(&n.base_notation.base.to_string())]),
            ],
        ),
        other => leaf("m:t", &other.to_string()),
    }
}
