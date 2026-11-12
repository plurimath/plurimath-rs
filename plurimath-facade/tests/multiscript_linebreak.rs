use plurimath_facade::request;
use serde_json::{json,Value};
#[test]
fn independent_multiscript_continuations_match_source() {
 let data:Value=serde_json::from_str(include_str!("../fixtures/multiscript-writers/linebreak-transform-source.json")).unwrap();
 let mut failures=Vec::new();let(mut outputs,mut errors)=(0,0);
 for row in data["rows"].as_array().unwrap(){for(format,expected)in row["writers"].as_object().unwrap(){
 let actual=request(&json!({"op":"write","format":format,"model":row["model"],"options":{}}));
 let ok=if expected.get("output").is_some(){let matched=actual["ok"]==true&&actual["output"]==expected["output"];if matched{outputs+=1;}matched}else{let matched=actual["ok"]==false&&actual["error"]["source_class"]==expected["error"]["class"];if matched{errors+=1;}matched};
 if !ok{failures.push(json!({"id":row["id"],"format":format,"expected":expected,"actual":actual}));}
 }}
 if let Some(out)=std::env::var_os("MULTISCRIPT_LINEBREAK_OUTPUT"){std::fs::write(out,serde_json::to_vec(&failures).unwrap()).unwrap();}
 println!("{}",json!({"outputs":outputs,"source_errors":errors,"failures":failures.len()}));
 assert!(failures.is_empty(),"{} mismatches; bounded IDs {:?}",failures.len(),failures.iter().take(5).map(|v|(&v["id"],&v["format"])).collect::<Vec<_>>());
 assert_eq!(outputs+errors,378);
}
