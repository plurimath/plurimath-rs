//! Explicit per-request adapter for source formatter objects. Never changes model values.
use super::number_formatter::{self as nf, FormatterError, FormatterProfile, HashArg, RubyValue, SymbolMap, TargetWriter, WriterOutput};
use super::{Model, Xml};
use serde_json::{json, Value};

pub(super) struct FormatterRequest {
    formatter: nf::NumberFormatter,
    format: HashArg,
}
fn wire_error(value: Value) -> String { format!("formatter_error_json:{value}") }
fn invalid(message: impl Into<String>) -> String { wire_error(json!({"code":"invalid_request","message":message.into()})) }
fn resource(message: impl Into<String>) -> String { wire_error(json!({"code":"resource_limit","message":message.into(),"resource":{"kind":"formatter_value_range"}})) }
fn scalar(value: &Value) -> Result<RubyValue, String> {
    Ok(match value {
        Value::Null => RubyValue::Nil,
        Value::Bool(v) => RubyValue::Bool(*v),
        Value::String(v) => RubyValue::Str(v.clone()),
        Value::Number(v) => {
            let text=v.to_string();
            if text.contains(['.','e','E']) { return Err("unported promised Ruby Float#to_s formatter option spelling".into()); }
            RubyValue::Int(text.parse().map_err(|_| resource("formatter integer option exceeds the native i64 range"))?)
        },
        Value::Array(_) | Value::Object(_) => return Err("unported promised Ruby Array/Hash formatter option inspection; canonical JSON does not retain Ruby key and element kinds".into()),
    })
}
fn symbol_key(key: &str) -> &str {
    match key {
        "basePrefix"=>"base_prefix", "basePostfix"=>"base_postfix", "hexCapital"=>"hex_capital",
        "groupDigits"=>"group_digits", "fractionGroup"=>"fraction_group", "fractionGroupDigits"=>"fraction_group_digits",
        "digitCount"=>"digit_count", "paddingDigits"=>"padding_digits", "paddingGroupDigits"=>"padding_group_digits",
        "numberSign"=>"number_sign", "exponentSign"=>"exponent_sign", _=>key,
    }
}
fn hash(value: Option<&Value>) -> Result<HashArg, String> {
    match value {
        None|Some(Value::Null)=>Ok(HashArg::Nil),
        Some(Value::Object(values))=>{
            let mut map=SymbolMap::new();
            for (key,value) in values {
                let key=symbol_key(key);
                if map.has(key) { return Err(invalid(format!("duplicate source formatter key {key}"))); }
                map.set(key,scalar(value)?);
            }
            Ok(HashArg::Hash(map))
        },
        Some(value)=>Ok(HashArg::NotHash(scalar(value)?)),
    }
}
pub(super) fn decode(value: Option<&Value>, format: Option<&Value>) -> Result<Option<FormatterRequest>,String> {
    let Some(value)=value else { return Ok(None) };
    if value.is_null() || value==false { return Ok(None) }
    let obj=value.as_object().ok_or_else(||invalid("formatterOptions must be an object, null, or false"))?;
    let profile=obj.get("profile").and_then(Value::as_str).unwrap_or("standard");
    let allowed: &[&str]=match profile {
        "standard"=>&["profile","locale","precision","stringFormat","options"],
        "base"=>&["profile","locale","precision","localizeNumber","localizerSymbols"],
        _=>return Err(invalid("formatterOptions.profile must be standard or base")),
    };
    if obj.get("profile").is_some_and(|v|!v.is_string()) { return Err(invalid("formatterOptions.profile must be a string")); }
    if let Some(key)=obj.keys().find(|k|!allowed.contains(&k.as_str())) { return Err(invalid(format!("unknown formatterOptions member {key}"))); }
    let locale=match obj.get("locale") {Some(v)=>scalar(v)?,None=>RubyValue::Str("en".into())};
    let precision=scalar(obj.get("precision").unwrap_or(&Value::Null))?;
    let profile=if profile=="standard" {
        FormatterProfile::Standard {locale,precision,string_format:scalar(obj.get("stringFormat").unwrap_or(&Value::Null))?,options:hash(obj.get("options"))?}
    } else {
        FormatterProfile::Base {locale,precision,localize_number:scalar(obj.get("localizeNumber").unwrap_or(&Value::Null))?,localizer_symbols:hash(obj.get("localizerSymbols"))?}
    };
    let formatter=nf::NumberFormatter::new(&profile).map_err(|error| failure(error).replacen("formatter_error_json:","formatter_constructor_error_json:",1))?;
    Ok(Some(FormatterRequest { formatter,format:hash(format)? }))
}
fn failure(error: FormatterError) -> String {
    let message=match &error {
        FormatterError::Configuration { kind:nf::ConfigErrorKind::InvalidFormatterOption, option, value, supported }=>format!("invalid value {} for formatter option :{} (expected {})",value.as_deref().unwrap_or("nil"),option.as_deref().unwrap_or(""),supported.as_deref().unwrap_or("")),
        FormatterError::Configuration { kind:nf::ConfigErrorKind::ConflictingFormatterOptions, supported,.. }=>format!("formatter options cannot be used together: {}",supported.as_deref().unwrap_or("")),
        FormatterError::InvalidNumber {value}=>format!("[plurimath] Invalid number {} for number formatting. Expected a numeric string such as \"1234\", \"-12.34\", or \"1.2e5\".",RubyValue::Str(value.clone()).inspect()),
        FormatterError::UnsupportedBase {base}=>format!("[plurimath] Unsupported base `{base}` for number formatting. [plurimath] The formatter `:base` option must be one of: 2, 8, 10, 16."),
        FormatterError::RubyArgumentError {message}|FormatterError::RubyTypeError {message}|FormatterError::RubyNoMethodError {message}=>message.clone(),
        FormatterError::ResourceLimit {what}=>return resource(format!("native formatter resource limit: {what}")),
    };
    wire_error(json!({"code":"render_error","message":message,"source_class":error.ruby_class()}))
}
fn xml(value:nf::FmtXml)->Xml {
    match value {nf::FmtXml::Text(v)=>Xml::Text(v),nf::FmtXml::El(name,attrs,children)=>Xml::El(name,attrs,children.into_iter().map(xml).collect())}
}
impl FormatterRequest {
    fn render(&self,m:&Model,target:TargetWriter)->Result<WriterOutput,String> {
        // Source Number#value.to_s. Float/Array/Hash spelling remains explicitly gated
        // by the same default Number conversion rather than guessed here.
        let source=super::numbers::ordinary_string(m.field("value"))?;
        let base=match m.field("base") {
            Value::Null|Value::Bool(false)=>None,
            Value::Number(v)=>Some(v.as_i64().ok_or_else(||resource("Number.base exceeds the native i64 range"))?),
            _=>return Err("unported promised non-integer source Number.base formatter value".into()),
        };
        let formatted=self.formatter.format_number(&source,base,&self.format).map_err(failure)?;
        Ok(nf::render_for_writer(&formatted,target))
    }
    pub(super) fn text(&self,m:&Model,format:&str)->Result<String,String> {
        let target=match format {"asciimath"=>TargetWriter::AsciiMath,"latex"=>TargetWriter::Latex,"html"=>TargetWriter::Html,"unicodemath"=>TargetWriter::UnicodeMath,"plain"=>TargetWriter::PlainText,_=>return Err(format!("unknown formatter text writer {format}"))};
        match self.render(m,target)? {WriterOutput::Text(v)=>Ok(v),WriterOutput::Xml(_)=>Err("formatter text writer unexpectedly returned XML".into())}
    }
    pub(super) fn mathml(&self,m:&Model)->Result<Vec<Xml>,String> {
        match self.render(m,TargetWriter::MathMl)? {WriterOutput::Xml(v)=>Ok(vec![xml(v)]),WriterOutput::Text(_)=>Err("formatter MathML writer unexpectedly returned text".into())}
    }
}

/// Formula public text writers wrap StandardError; nested XML fragments do not.
pub(super) fn scope_error(ctx:&super::WriterContext,error:String)->String {
 let Some(input)=ctx.formula_input else{return error};
 let Some(raw)=error.strip_prefix("formatter_error_json:")else{return error};
 let Ok(mut value)=serde_json::from_str::<Value>(raw)else{return error};
 if value["code"]!="render_error"||value["source_class"]=="Plurimath::Math::ParseError"{return error}
 let input=match super::numbers::ordinary_string(input){Ok(v)=>v,Err(_)=>return "unported promised Formula input_string Ruby inspection in render error".into()};
 let cause=value.clone();
 value["source_class"]=json!("Plurimath::Math::ParseError");
 value["message"]=json!(format!("[plurimath] Error: Failed to parse the following formula with type `{}`.\n[plurimath] Please first manually validate the formula.\n[plurimath] If this is a bug, please report the formula at our issue tracker at:\n[plurimath] https://github.com/plurimath/plurimath/issues\n---- FORMULA BEGIN ----\n{}\n---- FORMULA END ----\n",ctx.source_format,input));
 value["cause"]=cause;wire_error(value)
}

pub(super) fn source_error(ctx:&super::WriterContext,class:&str,message:String)->String {scope_error(ctx,wire_error(json!({"code":"render_error","source_class":class,"message":message})))}
