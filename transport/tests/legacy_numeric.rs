use serde_json::{json, Value};
fn report(mut v: Value) -> Value {
    v.sort_all_objects();
    let mut nums = vec![];
    let mut pending = vec![(&v, String::new())];
    while let Some((n, path)) = pending.pop() {
        match n{Value::Number(x)=>nums.push(json!({"path":path,"i64":x.is_i64(),"u64":x.is_u64(),"f64":x.is_f64(),"bits":format!("{:016x}",x.as_f64().unwrap().to_bits())})),Value::Array(a)=>{for(i,x)in a.iter().enumerate(){pending.push((x,format!("{}/{}",path,i)));}},Value::Object(m)=>{for(k,x)in m{pending.push((x,format!("{}/{}",path,k)));}},_=>{}}
    }
    nums.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    json!({"accepted":true,"canonical":v.to_string(),"numbers":nums})
}
#[test]
fn legacy151_number_kinds_bits_syntax_and_opaque_keys() {
    let mut wrong = vec![];
    let mut cases = 0;
    for line in include_str!("fixtures/legacy-json151.jsonl").lines() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = match plurimath_json::decode_with_limits(
            row["input"].as_str().unwrap().as_bytes(),
            127,
            usize::MAX,
            plurimath_json::NumberMode::Finite,
        ) {
            Ok(v) => report(v),
            Err(_) => json!({"accepted":false}),
        };
        if actual != row["expected"] {
            wrong.push(json!({"index":row["index"],"input":row["input"],"actual":actual,"expected":row["expected"]}));
        }
        cases += 1;
    }
    assert_eq!(cases, 252);
    assert!(
        wrong.is_empty(),
        "{}",
        serde_json::to_string(&wrong).unwrap()
    );
}
