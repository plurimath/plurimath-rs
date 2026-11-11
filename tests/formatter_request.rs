use plurimath_facade::request;
use serde_json::{json,Value};
fn fixtures()->Vec<Value>{serde_json::from_str::<Value>(include_str!("../fixtures/formatter/ruby-standard66.json")).unwrap()["cases"].as_array().unwrap().clone()}
#[test]
fn source_standard66_all_six_writers_are_byte_exact(){
 let rows=fixtures();let mut count=0;
 for row in rows {for (format,expected)in row["expected"].as_object().unwrap(){
  let result=request(&json!({"op":"write","model":row["model"],"format":format,"options":{"formatterOptions":row["formatterOptions"]}}));
  assert_eq!(result["ok"],true,"{} {format}: {result}",row["id"]);
  assert_eq!(&result["output"],expected,"{} {format}",row["id"]);count+=1;
 }}assert_eq!(count,396);
}
#[test]
fn formatter_context_is_per_request_and_number_wire_is_unchanged(){
 let rows=fixtures();
 for row in rows.iter().step_by(7){
  let plain=json!({"op":"write","model":row["model"],"format":"asciimath"});let before=request(&plain);
  let configured=request(&json!({"op":"write","model":row["model"],"format":"asciimath","options":{"formatterOptions":row["formatterOptions"]}}));
  assert_eq!(configured["output"],row["expected"]["asciimath"]);
  assert_eq!(request(&plain),before);
  let model=request(&json!({"op":"model","model":row["model"]}));assert_eq!(model["model"],row["model"]);
  for disabled in [Value::Null,json!(false)]{
   assert_eq!(request(&json!({"op":"write","model":row["model"],"format":"asciimath","options":{"formatterOptions":disabled}})),before);
  }
 }
}
#[test]
fn formatter_contexts_are_independent_across_threads(){
 let rows=fixtures();std::thread::scope(|scope|{for row in rows.iter().step_by(9){scope.spawn(move||{for _ in 0..8{
  let result=request(&json!({"op":"write","model":row["model"],"format":"asciimath","options":{"formatterOptions":row["formatterOptions"]}}));assert_eq!(result["output"],row["expected"]["asciimath"]);
 }});}});
}
#[test]
fn scalar_type_and_resource_boundaries_are_explicit(){
 let model=json!({"class":"Math::Number","fields":{"value":"1234"}});
 let render=|profile:Value|request(&json!({"op":"write","model":model,"format":"asciimath","options":{"formatterOptions":profile}}));
 assert_eq!(render(json!({"precision":1.5}))["error"]["code"],"unported_capability");
 assert_eq!(render(json!({"options":{"groupDigits":[]}}))["error"]["code"],"unported_capability");
 assert_eq!(render(json!({"profile":"imagined"}))["error"]["code"],"invalid_request");
 let huge:Value=serde_json::from_str("{\"options\":{\"groupDigits\":9223372036854775808}}").unwrap();assert_eq!(render(huge)["error"]["code"],"resource_limit");
}

#[test]
fn source_composites_and_errors_preserve_caller_scope(){
 let data:Value=serde_json::from_str(include_str!("../fixtures/formatter/ruby-contexts51.json")).unwrap();let mut observations=0;
 for row in data["cases"].as_array().unwrap(){for(format,want)in row["writers"].as_object().unwrap(){
  let actual=request(&json!({"op":"write","format":format,"model":row["model"],"options":{"formatterOptions":row["formatterOptions"]}}));
  if let Some(output)=want.get("output"){assert_eq!(actual["ok"],true,"{} {format}: {actual}",row["id"]);assert_eq!(&actual["output"],output,"{} {format}",row["id"])}
  else{assert_eq!(actual["ok"],false);assert_eq!(actual["error"]["source_class"],want["error"]["class"]);assert_eq!(actual["error"]["message"],want["error"]["message"]);assert_eq!(actual["error"]["cause"]["source_class"],want["error"]["cause_class"])}
  observations+=1;
 }}assert_eq!(observations,306);
}
#[test]
fn construction_errors_stay_outside_formula_render_scope(){
 let row=fixtures().remove(0);let result=request(&json!({"op":"write","model":row["model"],"format":"asciimath","options":{"formatterOptions":{"options":"not a Hash"}}}));
 assert_eq!(result["error"]["source_class"],"Plurimath::ConfigurationError");assert!(result["error"].get("cause").is_none());
}
