fn main() {
    for s in std::env::args().skip(1) {
        println!(
            "{}",
            serde_json::json!({"input":s,"result":plurimath_facade::stage_tree(&s)})
        );
    }
}
