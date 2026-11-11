//! Source Math::Function::Ul procedures; Number children retain their source class.
use super::{Model,Value,WriterContext,Xml,el,attrs,text,child,strict_slot_xml,control_properties};
pub(super) fn handles(m:&Model)->bool{m.class=="Math::Function::Ul"}
fn truthy(v:&Value)->bool{!v.is_null()&&v!=false}
fn error(ctx:&WriterContext,class:&str,method:&str)->String{super::formatter_context::source_error(ctx,class,format!("undefined source underline argument operation {method}"))}
fn one(v:&Value,f:&str,ctx:&WriterContext)->Result<String,String>{if !v.is_object(){return Err(error(ctx,"NoMethodError",&format!("to_{f}")))}child(v,f,ctx)}
pub(super) fn text_writer(m:&Model,f:&str,ctx:&WriterContext)->Result<String,String>{
 let v=m.field("parameter_one");
 match f{
  "asciimath"=>Ok(if truthy(v){format!("underline({})",one(v,f,ctx)?)}else{"underline".into()}),
  "latex"=>Ok(if truthy(v){format!("\\underline{{{}}}",one(v,f,ctx)?)}else{"\\underline".into()}),
  "html"=>{
   let body=if let Some(values)=v.as_array(){let mut out=String::new();for item in values{out.push_str(&one(item,f,ctx)?)}Some(out)}else if truthy(v){Some(one(v,f,ctx)?)}else{None};
   Ok(format!("<i>underline</i>{}",body.map(|s|format!("<i>{s}</i>")).unwrap_or_default()))
  },
  "unicodemath"=>{if !v.is_object(){return Err(error(ctx,"NoMethodError","to_unicodemath"))}Ok(format!("▁{}",super::parens(v,f,ctx)?))},
  _=>Err(format!("unknown underline text format {f}")),
 }
}
fn argument(v:&Value,f:&str,ctx:&WriterContext)->Result<Vec<Xml>,String>{
 if let Some(values)=v.as_array(){
  if f!="omml"{return Err(error(ctx,"NoMethodError","to_mathml_without_math_tag"))}
  let mut out=vec![];for item in values{if !item.is_object(){return Err(error(ctx,"NoMethodError","insert_t_tag"))}out.extend(strict_slot_xml(item,f,ctx,false)?)}return Ok(out)
 }
 if !v.is_object(){return Err(error(ctx,"NoMethodError",if f=="omml"{"insert_t_tag"}else{"to_mathml_without_math_tag"}))}
 strict_slot_xml(v,f,ctx,false)
}
fn attribute_values(m:&Model,ctx:&WriterContext)->Result<Vec<(String,String)>,String>{
 let value=m.field("attributes");
 if !truthy(value)||value.as_array().is_some_and(Vec::is_empty)||value==""{return Ok(vec![])}
 let values=value.as_object().ok_or_else(||error(ctx,"NoMethodError","attributes.empty?/set_attr"))?;
 values.iter().map(|(key,value)|Ok((key.clone(),super::numbers::ordinary_string(value)?))).collect()
}
fn accentunder(m:&Model,ctx:&WriterContext)->Result<bool,String>{
 let value=m.field("attributes");if !truthy(value){return Ok(false)}
 if let Some(values)=value.as_object(){return Ok(values.get("accentunder").is_some_and(truthy))}
 let class=if value.is_array()||value.is_string()||value.as_i64().is_some(){"TypeError"}else{"NoMethodError"};Err(error(ctx,class,"attributes[:accentunder]"))
}
pub(super) fn xml(m:&Model,f:&str,ctx:&WriterContext)->Result<Vec<Xml>,String>{
 let value=m.field("parameter_one");
 if f=="mathml"{
  let mark=el("mo",vec![text("&#x332;")]);if !truthy(value){return Ok(vec![mark])}
  let mut children=argument(value,f,ctx)?;children.push(mark);return Ok(vec![attrs("munder",attribute_values(m,ctx)?,children)])
 }
 if f!="omml"{return Err(format!("unknown underline XML format {f}"))}
 if !truthy(value){return Ok(vec![el("m:r",vec![el("m:t",vec![text("&#x332;")])])])}
 if accentunder(m,ctx)?{
  return Ok(vec![el("m:groupChr",vec![el("m:groupChrPR",vec![attrs("m:chr",vec![("m:val".into(),"_".into())],vec![]),attrs("m:pos",vec![("m:val".into(),"bot".into())],vec![])]),el("m:e",argument(value,f,ctx)?)])])
 }
 let display=ctx.with_display_style(true);
 Ok(vec![el("m:limLow",vec![el("m:limLowPr",vec![control_properties()]),el("m:e",argument(value,f,&display)?),el("m:lim",vec![el("m:r",vec![el("m:t",vec![text("&#x332;")])])])])])
}
