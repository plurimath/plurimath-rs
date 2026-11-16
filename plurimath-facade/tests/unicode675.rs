//! The 675-input UnicodeMath corpus (plurimath-testsuite unicode-expectations)
//! against the model Ruby plurimath 0a1df68 + umath 0fca022 builds for each input.
//! A source parse error must stay a native refusal; every other input must give
//! the identical typed model. Set UNICODE675_SHARD=<index>/<count> to run a slice.
use serde_json::{json, Value};

fn shard() -> (usize, usize) {
    match std::env::var("UNICODE675_SHARD") {
        Ok(spec) => {
            let (index, count) = spec
                .split_once('/')
                .expect("UNICODE675_SHARD=<index>/<count>");
            (index.parse().unwrap(), count.parse().unwrap())
        }
        Err(_) => (0, 1),
    }
}

#[test]
fn unicode675_rows_match_ruby_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/text-unicode-captures/unicode675-ruby.json"
    ))
    .unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 675);
    let (index, count) = shard();
    let (mut equal, mut refused, mut failures) = (0, 0, Vec::new());
    for (position, row) in rows.iter().enumerate() {
        let id = row["id"].as_str().unwrap();
        if let Ok(only) = std::env::var("UNICODE675_IDS") {
            if !only.split(',').any(|n| id.ends_with(&format!("-{n}"))) {
                continue;
            }
        } else if position % count != index {
            continue;
        }
        let response = plurimath_facade::request(
            &json!({"op":"parse","format":"unicodemath","input":row["text"]}),
        );
        match row["expect"].as_str().unwrap() {
            "model" if response["ok"] == true && response["model"] == row["parsed_model"] => {
                equal += 1
            }
            "model" if response["ok"] == true => {
                if std::env::var("UNICODE675_SHOW").is_ok() {
                    println!(
                        "SHOW {id} {}\n got  {}\n want {}",
                        row["text"], response["model"], row["parsed_model"]
                    );
                }
                failures.push(format!("{id} model differs"))
            }
            "model" => failures.push(format!(
                "{id} refused: {}",
                response["error"]["message"].as_str().unwrap_or("?")
            )),
            "error" if response["ok"] == false => refused += 1,
            _ => failures.push(format!("{id} source error but native parsed")),
        }
    }
    println!(
        "shard {index}/{count}: {equal} equal, {refused} source-error refusals, {} failures",
        failures.len()
    );
    for failure in &failures {
        println!("FAIL {failure}");
    }
    assert!(
        failures.is_empty(),
        "{} rows differ from source",
        failures.len()
    );
}
