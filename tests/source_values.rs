use plurimath_facade::source_values::{self as sv, Domain,Profile,RubyValue as R};
use serde_json::json;
#[test]
fn canonical_domains_never_cast_string_values_or_array_pairs(){
 let v=sv::hash(&json!({"mask":"0d17","mpadded":{"width":"0"}}),Domain::NamedOptions,Profile::Canonical).unwrap();
 assert_eq!(v.get_symbol("mask"),Some(&R::String("0d17".into())));
 assert!(v.get_symbol("mpadded").unwrap().get_string("width").is_some());
 let nested=sv::in_domain(v.get_symbol("mpadded").unwrap(),Domain::NamedOptions,Profile::Canonical).unwrap();
 assert!(nested.get_symbol("width").is_some());
 assert_eq!(sv::decode(&json!([["width","0"]]),Domain::NamedOptions,Profile::Canonical).unwrap(),R::Array(vec![R::Array(vec![R::String("width".into()),R::String("0".into())])]));
}
#[test]
fn typed_keys_are_not_rebound_and_float_bits_roundtrip(){
 let value=R::Hash(vec![(R::Symbol("width".into()),R::String("0".into())),(R::String("width".into()),R::String("1".into()))]);
 let wire=sv::encode(&value);let decoded=sv::decode(&wire,Domain::NamedOptions,Profile::RubyValuesV1).unwrap();
 assert_eq!(decoded,value);assert_eq!(sv::in_domain(&decoded,Domain::NamedOptions,Profile::RubyValuesV1).unwrap(),value);
 for bits in [0,1,0x8000000000000000,0x3ff0000000000000,0x7ff8000000000011] {let v=R::Float(bits);assert_eq!(sv::decode(&sv::encode(&v),Domain::Literal,Profile::RubyValuesV1).unwrap(),v);assert!(sv::to_s(&v).is_err());}
}
#[test]
fn source_hash_equality_ignores_order_and_distinguishes_key_kind(){
 let a=R::Hash(vec![(R::Symbol("width".into()),R::String("0".into())),(R::Symbol("height".into()),R::String("1".into()))]);
 let mut b=a.clone();if let R::Hash(v)=&mut b{v.reverse()};assert!(sv::eql(&a,&b));
 let c=R::Hash(vec![(R::String("width".into()),R::String("0".into())),(R::Symbol("height".into()),R::String("1".into()))]);assert!(!sv::equal(&a,&c));
 assert_eq!(sv::inspect(&a).unwrap(),"{width: \"0\", height: \"1\"}");
 assert!(sv::equal(&R::Integer("1".into()),&R::Float(1f64.to_bits())));
 assert!(!sv::eql(&R::Integer("1".into()),&R::Float(1f64.to_bits())));
 assert!(!sv::equal(&R::Integer("9007199254740993".into()),&R::Float(9007199254740992f64.to_bits())));
}
#[test]
fn malformed_typed_values_refuse(){
 for value in [json!(["unknown",1]),json!(["integer","01"]),json!(["float","nan"]),json!(["hash",[[["symbol","x"],["nil"]],[["symbol","x"],["bool",true]]]])] {assert!(sv::decode(&value,Domain::Literal,Profile::RubyValuesV1).is_err());}
}

#[test]
fn actual_ruby_four_inspection_values(){
 let data:serde_json::Value=serde_json::from_str(include_str!("../fixtures/source-values/ruby4-inspection.json")).unwrap();
 let mut exact=0;let mut gated=0;
 for row in data["rows"].as_array().unwrap(){let value=sv::decode(&row["wire"],Domain::Literal,Profile::RubyValuesV1).unwrap();assert_eq!(sv::encode(&value),row["wire"]);match sv::inspect(&value){Ok(actual)=>{assert_eq!(actual,row["inspect"].as_str().unwrap(),"{}",row["id"]);assert_eq!(sv::to_s(&value).unwrap(),row["to_s"].as_str().unwrap());exact+=1},Err(_)=>gated+=1}}
 assert_eq!((exact,gated),(51,2));
}
#[test]
fn unqualified_symbol_inspection_never_returns_a_wrong_success(){
 let symbol=R::Symbol("α".into());
 assert_eq!(sv::to_s(&symbol).unwrap(),"α");
 assert!(sv::inspect(&symbol).is_err());
 assert!(sv::inspect(&R::Hash(vec![(symbol,R::String("0".into()))])).is_err());
}
