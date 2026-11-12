fn main() {
    for s in std::env::args().skip(1) {
        let (format, input) = s.split_once(':').unwrap();
        let value = match format {
            "latex" => latexmath::Parser::new()
                .unwrap()
                .parse_legacy(input)
                .map(|p| p.canonical())
                .map_err(|e| e.to_string()),
            "unicode" => umath::Parser::new()
                .unwrap()
                .parse_legacy(input)
                .map(|p| p.canonical())
                .map_err(|e| e.to_string()),
            _ => unreachable!(),
        };
        println!(
            "{}",
            serde_json::json!({"format":format,"input":input,"stage":value})
        );
    }
}
