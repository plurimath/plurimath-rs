use plurimath_facade::request;
use serde_json::{json,Value};
#[test]
fn source_table_ul_548_profiles_preserve_every_writer_byte_and_refusal(){
 let data:Value=serde_json::from_str(include_str!("../fixtures/table-ul/ruby-independent548.json")).unwrap();let mut outputs=0;let mut errors=0;
 for row in data["cases"].as_array().unwrap(){for(format,want)in row["writers"].as_object().unwrap(){
  let mut options=json!({"formatterOptions":row["formatterOptions"]});if format=="mathml"||format=="omml"{options["displayStyle"]=row["displayStyle"].clone()}
  let actual=request(&json!({"op":"write","model":row["model"],"format":format,"options":options}));
  if let Some(output)=want.get("output"){assert_eq!(actual["ok"],true,"{} {format}: {actual}",row["id"]);assert_eq!(&actual["output"],output,"{} {format}",row["id"]);outputs+=1;}
  else{assert_eq!(actual["ok"],false,"{} {format}: {actual}",row["id"]);assert_eq!(actual["error"]["source_class"],want["error"]["class"],"{} {format}",row["id"]);assert_eq!(actual["error"]["message"],want["error"]["message"],"{} {format}",row["id"]);assert_eq!(actual["error"]["cause"]["source_class"],want["error"]["cause_class"],"{} {format}",row["id"]);errors+=1;}
 }}assert_eq!(outputs,2878);assert_eq!(errors,410);
}
