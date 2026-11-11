//! Moxml's source entity wrapper, applied around the accepted document parser.
//! The marker and name grammar match Moxml::Entity and the source XML reader.
use moxml::document::{ContentRow, ElementRow};

const MARKER: &str = "\u{fffc}\u{feff}";

fn first_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}
fn name_byte(byte: u8) -> bool {
    first_name_byte(byte) || byte.is_ascii_digit() || matches!(byte, b'.' | b':' | b'-')
}
fn rewrite(input: &str, prefix: &str, replacement: impl Fn(&str) -> Option<String>) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = input[cursor..].find(prefix) {
        let start = cursor + relative;
        let name_start = start + prefix.len();
        let mut end = name_start;
        if input
            .as_bytes()
            .get(end)
            .is_some_and(|b| first_name_byte(*b))
        {
            end += 1;
            while input.as_bytes().get(end).is_some_and(|b| name_byte(*b)) {
                end += 1;
            }
            if input.as_bytes().get(end) == Some(&b';') {
                output.push_str(&input[cursor..start]);
                if let Some(value) = replacement(&input[name_start..end]) {
                    output.push_str(&value);
                } else {
                    output.push_str(&input[start..=end]);
                }
                cursor = end + 1;
                continue;
            }
        }
        output.push_str(&input[cursor..name_start]);
        cursor = name_start;
    }
    output.push_str(&input[cursor..]);
    output
}

pub(super) fn prepare(input: &str) -> String {
    let rewritten = rewrite_declared_encoding(input);
    rewrite(&rewritten, "&", |name| {
        if matches!(name, "amp" | "lt" | "gt" | "quot" | "apos") {
            None
        } else {
            Some(format!("{MARKER}{name};"))
        }
    })
}
fn rewrite_declared_encoding(input: &str) -> String {
    if input.starts_with("<?xml") {
        let declaration_end = input.find('>').unwrap_or(input.len());
        if let Some(position) = input[..declaration_end].find("encoding=") {
            let value_start = position + "encoding=".len();
            if let Some(quote @ (b'\'' | b'"')) = input.as_bytes().get(value_start) {
                if let Some(relative) = input[value_start + 1..declaration_end].find(*quote as char)
                {
                    let value_end = value_start + relative + 2;
                    return format!("{}\"UTF-8\"{}", &input[..value_start], &input[value_end..]);
                }
            }
        }
    }
    input.to_owned()
}
fn restore(input: &str) -> String {
    let first = rewrite(input, MARKER, |name| Some(format!("&{name};")));
    rewrite(&first, "&#xFFFC;&#xFEFF;", |name| Some(format!("&{name};")))
}

pub(super) fn restore_rows(rows: &mut [ElementRow]) {
    for row in rows {
        for attribute in &mut row.attributes {
            attribute.value = restore(&attribute.value);
        }
        for content in &mut row.content {
            match content {
                ContentRow::Text(value) | ContentRow::CData(value) => *value = restore(value),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entity_marking_preserves_the_source_standard_and_unknown_reference_policy() {
        assert_eq!(
            prepare("<r>&nbsp; &amp; &#160; &raw</r>"),
            format!("<r>{MARKER}nbsp; &amp; &#160; &raw</r>")
        );
        assert_eq!(
            restore(&format!("{MARKER}nbsp; &#xFFFC;&#xFEFF;alpha;")),
            "&nbsp; &alpha;"
        );
        assert_eq!(
            prepare("<?xml encoding='ISO-8859-1'?><r/>"),
            "<?xml encoding=\"UTF-8\"?><r/>"
        );
    }
}

/// The source XHTML1 decoder used by Text#value= and accent-token lookup.
pub(super) fn decode_html(input: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = input[cursor..].find('&') {
        let start = cursor + relative;
        let token_start = start + 1;
        let mut end = token_start;
        let mut radix = None;
        let mut digits_start = token_start;
        if input.as_bytes().get(end) == Some(&b'#') {
            end += 1;
            let hexadecimal = matches!(input.as_bytes().get(end), Some(b'x' | b'X'));
            if hexadecimal {
                end += 1;
            }
            digits_start = end;
            let limit = if hexadecimal { 6 } else { 7 };
            while end - digits_start < limit
                && input.as_bytes().get(end).is_some_and(|byte| {
                    if hexadecimal {
                        byte.is_ascii_hexdigit()
                    } else {
                        byte.is_ascii_digit()
                    }
                })
            {
                end += 1;
            }
            if end > digits_start {
                radix = Some(if hexadecimal { 16 } else { 10 });
            }
        } else if input
            .as_bytes()
            .get(end)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            end += 1;
            while input
                .as_bytes()
                .get(end)
                .is_some_and(u8::is_ascii_alphanumeric)
            {
                end += 1;
            }
        }
        let terminator = input.as_bytes().get(end);
        let complete = end > token_start && matches!(terminator, Some(b';' | b'\n' | b'<'));
        if complete {
            let codepoint = if let Some(radix) = radix {
                Some(
                    u32::from_str_radix(&input[digits_start..end], radix)
                        .map_err(|error| error.to_string())?,
                )
            } else {
                super::tables()["XHTML1_ENTITY_CODEPOINTS"][&input[token_start..end]]
                    .as_u64()
                    .map(|code| code as u32)
            };
            if let Some(codepoint) = codepoint {
                let character = char::from_u32(codepoint)
                    .ok_or_else(|| format!("invalid codepoint 0x{codepoint:X} in UTF-8"))?;
                output.push_str(&input[cursor..start]);
                output.push(character);
                cursor = end + usize::from(terminator == Some(&b';'));
                continue;
            }
        }
        output.push_str(&input[cursor..token_start]);
        cursor = token_start;
    }
    output.push_str(&input[cursor..]);
    Ok(output)
}
