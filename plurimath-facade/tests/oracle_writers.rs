//! Feeds Ruby-oracle Math models (converted by scripts/oracle-writers/convert.py)
//! to the native writer API and compares with the recorded Ruby writer outcomes.
//! ORACLE_MODELS=<converted jsonl> ORACLE_REPORT=<out json> cargo test --test oracle_writers
//! Without ORACLE_MODELS the test is skipped (the input is generated, not committed).
use plurimath_facade::writer::write;
use serde_json::{json, Value};

const FORMATS: [(&str, &str); 6] = [
    ("asciimath", "to_asciimath"),
    ("latex", "to_latex"),
    ("mathml", "to_mathml"),
    ("omml", "to_omml"),
    ("unicodemath", "to_unicodemath"),
    ("html", "to_html"),
];

#[test]
fn oracle_models_match_ruby_writers() {
    let Ok(path) = std::env::var("ORACLE_MODELS") else {
        eprintln!("ORACLE_MODELS unset; skipped");
        return;
    };
    let mut report = serde_json::Map::new();
    let mut refusals = vec![];
    let mut mismatches = 0usize;
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let row: Value = serde_json::from_str(line).unwrap();
        let id = row["id"].as_str().unwrap().to_string();
        if row["model"].is_null() {
            refusals.push(id);
            continue;
        }
        let display = match row["display_flag"].as_str().unwrap() {
            "0" => false,
            "1" => true,
            other => panic!("unknown display flag {other}"),
        };
        for (fmt, ruby_key) in FORMATS {
            let expected = &row["source_writers"][ruby_key];
            let options = if fmt == "mathml" || fmt == "omml" {
                json!({"displayStyle": display})
            } else {
                json!({})
            };
            if std::env::var("ORACLE_TRACE").is_ok() {
                eprintln!("{id} {fmt}");
            }
            // Native budget per write; a write that exceeds it is a resource-limit
            // outcome, comparable only with Ruby's own Timeout::Error.
            let (tx, rx) = std::sync::mpsc::channel();
            {
                let (m, o) = (row["model"].clone(), options.clone());
                std::thread::Builder::new()
                    .stack_size(256 << 20)
                    .spawn(move || {
                        let _ = tx.send(write(&m, fmt, &o));
                    })
                    .unwrap();
            }
            let got = rx
                .recv_timeout(std::time::Duration::from_secs(std::env::var("ORACLE_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(20)))
                .unwrap_or_else(|_| Err("native_timeout".to_string()));
            let entry = report
                .entry(fmt.to_string())
                .or_insert_with(|| json!({"ok":0,"mismatch":0,"first":[],"causes":{}}));
            let ok = match (&got, expected["ok"].as_bool().unwrap()) {
                (Ok(out), true) => Some(out.as_str()) == expected["output"].as_str(),
                (Err(e), false) => {
                    let class = expected["error"]["class"].as_str().unwrap_or("");
                    if class == "Timeout::Error" {
                        e == "native_timeout"
                    } else {
                        class == "Plurimath::Math::ParseError" && e != "native_timeout"
                    }
                }
                _ => false,
            };
            if ok {
                entry["ok"] = json!(entry["ok"].as_u64().unwrap() + 1);
            } else {
                mismatches += 1;
                let cause = match &got {
                    Err(e) => format!("native error: {e}"),
                    Ok(_) if expected["ok"] == true => "output differs".to_string(),
                    Ok(_) => "native succeeded where Ruby raised".to_string(),
                };
                let ids = entry["causes"][cause.as_str()].take();
                let mut ids = if ids.is_array() { ids } else { json!([]) };
                ids.as_array_mut().unwrap().push(json!(id));
                entry["causes"][cause.as_str()] = ids;
                entry["mismatch"] = json!(entry["mismatch"].as_u64().unwrap() + 1);
                if entry["first"].as_array().unwrap().len() < 5 {
                    entry["first"].as_array_mut().unwrap().push(json!({
                        "id": id,
                        "native": match &got { Ok(o) => json!({"ok":o}), Err(e) => json!({"err":e}) },
                        "ruby": expected,
                    }));
                }
            }
        }
    }
    let out = json!({"formats": report, "source_refusals": refusals, "mismatches": mismatches});
    if let Ok(p) = std::env::var("ORACLE_REPORT") {
        std::fs::write(p, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }
    assert_eq!(mismatches, 0, "writer parity mismatches; see ORACLE_REPORT");
}
