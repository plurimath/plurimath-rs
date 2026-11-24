//! Byte-frozen gum-ref oracle gate (issue #21, first slice).
//!
//! The archived gum-ref request vectors (including the Monte Carlo seeds and
//! generator parameters carried in their requests) are replayed through
//! `evaluate_json`, and the exact output string is compared with a recorded
//! length and 64-bit FNV-1a digest. `tests/gum.rs` compares parsed JSON with
//! the engine name and version removed, so key order, number spelling and
//! whitespace can drift unnoticed there; this gate fails on any byte change.
//!
//! The recorded bytes are this crate's own output, not the Ruby bytes: the
//! archived `expected` values differ in key order and fraction reduction. The
//! digests therefore freeze the current behavior as the single corpus oracle
//! (`fixtures/oracle-bytes.json`). Whole-output fixtures for a few cases live in
//! `tests/legacy_bytes.rs` (issue #16); this gate covers every case. It is
//! deliberately separate from any gum-par cross-device suite.
//!
//! Regenerate only on purpose, against code whose output is accepted:
//! `BLESS_ORACLE_BYTES=1 cargo test -p plurimath-numeric --test oracle_bytes bless`.
use plurimath_numeric::evaluate_json;
use serde_json::{json, Value};

const FILES: &[(&str, &str)] = &[
    ("gum-f40", include_str!("../fixtures/gum-f40.json")),
    (
        "gum-conformity",
        include_str!("../fixtures/gum-conformity.json"),
    ),
    (
        "gum-annex-methods",
        include_str!("../fixtures/gum-annex-methods.json"),
    ),
    ("gum-mcm", include_str!("../fixtures/gum-mcm.json")),
    (
        "gum-mcm-joint-rational",
        include_str!("../fixtures/gum-mcm-joint-rational.json"),
    ),
];

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn digest(output: &str) -> (usize, String) {
    (output.len(), format!("{:016x}", fnv1a64(output.as_bytes())))
}

fn replayed() -> Vec<(String, String, String)> {
    let mut all = Vec::new();
    for (file, source) in FILES {
        let fixture: Value = serde_json::from_str(source).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let output = evaluate_json(&case["request"].to_string());
            all.push((
                file.to_string(),
                case["id"].as_str().unwrap().to_string(),
                output,
            ));
        }
    }
    all
}

#[test]
fn archived_vectors_replay_to_frozen_bytes() {
    let recorded: Value =
        serde_json::from_str(include_str!("../fixtures/oracle-bytes.json")).unwrap();
    let recorded = recorded.as_array().unwrap();
    let replay = replayed();
    assert_eq!(recorded.len(), replay.len(), "archived case count changed");
    let mut drifted = Vec::new();
    for (entry, (file, id, output)) in recorded.iter().zip(&replay) {
        let (len, hash) = digest(output);
        let same = entry["file"] == *file
            && entry["id"] == *id
            && entry["len"] == len
            && entry["fnv1a64"] == hash;
        if !same {
            drifted.push(format!("{file}/{id}"));
        }
    }
    assert!(drifted.is_empty(), "output bytes drifted for {drifted:?}");
}

#[test]
fn bless() {
    if std::env::var_os("BLESS_ORACLE_BYTES").is_none() {
        return;
    }
    let entries: Vec<Value> = replayed()
        .iter()
        .map(|(file, id, output)| {
            let (len, hash) = digest(output);
            json!({"file": file, "id": id, "len": len, "fnv1a64": hash})
        })
        .collect();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/oracle-bytes.json");
    std::fs::write(path, serde_json::to_string_pretty(&entries).unwrap() + "\n").unwrap();
}
