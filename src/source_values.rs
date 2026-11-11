//! Explicit source value kinds. Canonical domains affect Hash keys, never values.
use serde_json::Value;

pub const SCHEMA: &str = "plurimath.ruby-values/1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile { Canonical, RubyValuesV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain { Literal, NamedOptions, Attributes }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RubyValue {
    Nil, Bool(bool), Integer(String), Float(u64), String(String), Symbol(String),
    Array(Vec<RubyValue>), Hash(Vec<(RubyValue,RubyValue)>),
    Node {class:String,fields:Vec<(String,RubyValue)>},
}
impl RubyValue {
    pub fn get_symbol(&self,key:&str)->Option<&Self>{
        if let Self::Hash(entries)=self {entries.iter().find_map(|(k,v)|matches!(k,Self::Symbol(s)if s==key).then_some(v))}else{None}
    }
    pub fn get_string(&self,key:&str)->Option<&Self>{
        if let Self::Hash(entries)=self {entries.iter().find_map(|(k,v)|matches!(k,Self::String(s)if s==key).then_some(v))}else{None}
    }
}
fn integer(s:&str)->Result<String,String>{
    let digits=s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b|b.is_ascii_digit()) || (digits.len()>1 && digits.starts_with('0')) || s=="-0" {return Err("invalid canonical source Integer".into())}
    Ok(s.into())
}
pub fn decode(value:&Value,domain:Domain,profile:Profile)->Result<RubyValue,String>{
    if profile==Profile::RubyValuesV1{return decode_tag(value,0)}
    Ok(match value {
        Value::Null=>RubyValue::Nil,Value::Bool(v)=>RubyValue::Bool(*v),Value::String(v)=>RubyValue::String(v.clone()),
        Value::Number(n)=>{let s=n.to_string();if s.contains(['.','e','E']){RubyValue::Float(n.as_f64().ok_or("source Float out of range")?.to_bits())}else{RubyValue::Integer(integer(&s)?)}},
        Value::Array(v)=>RubyValue::Array(v.iter().map(|v|decode(v,Domain::Literal,profile)).collect::<Result<_,_>>()?),
        Value::Object(v)=>RubyValue::Hash(v.iter().map(|(k,v)|Ok((if domain==Domain::Literal{RubyValue::String(k.clone())}else{RubyValue::Symbol(k.clone())},decode(v,Domain::Literal,profile)?))).collect::<Result<_,String>>()?),
    })
}
pub fn hash(value:&Value,domain:Domain,profile:Profile)->Result<RubyValue,String>{
    let v=decode(value,domain,profile)?;
    if matches!(v,RubyValue::Hash(_)){Ok(v)}else{Err("source_render_error: source value is not a Hash".into())}
}
pub fn in_domain(value:&RubyValue,domain:Domain,profile:Profile)->Result<RubyValue,String>{
    if profile==Profile::RubyValuesV1 || domain==Domain::Literal{return Ok(value.clone())}
    match value {
        RubyValue::Hash(entries)=>Ok(RubyValue::Hash(entries.iter().map(|(k,v)|Ok((match k{RubyValue::String(s)=>RubyValue::Symbol(s.clone()),RubyValue::Symbol(_)=>k.clone(),_=>return Err("canonical domain Hash key must be a name".into())},v.clone()))).collect::<Result<_,String>>()?)),
        _=>Err("source_render_error: domain value is not a Hash".into()),
    }
}
fn quote(s:&str)->String{
    let chars=s.chars().collect::<Vec<_>>();let mut out=String::from("\"");
    for (i,c) in chars.iter().enumerate(){match *c {
        '"'=>out.push_str("\\\""),'\\'=>out.push_str("\\\\"),'\n'=>out.push_str("\\n"),'\r'=>out.push_str("\\r"),'\t'=>out.push_str("\\t"),'\u{0008}'=>out.push_str("\\b"),'\u{000c}'=>out.push_str("\\f"),'\u{000b}'=>out.push_str("\\v"),'\u{0007}'=>out.push_str("\\a"),'\u{001b}'=>out.push_str("\\e"),
        '#' if chars.get(i+1).is_some_and(|v|['{','$','@'].contains(v))=>out.push_str("\\#"),
        c if c<' ' || c=='\u{007f}'=>out.push_str(&format!("\\u{:04X}",c as u32)),c=>out.push(c),
    }}out.push('"');out
}
fn label(s:&str)->bool{
    let mut chars=s.chars();let Some(first)=chars.next()else{return false};
    (first.is_ascii_alphabetic()||first=='_') && chars.enumerate().all(|(i,c)|c.is_ascii_alphanumeric()||c=='_'||(['!','?'].contains(&c)&&i+2==s.len()))
}
fn symbol_inspect(s:&str)->Result<String,String>{
    if !s.is_ascii() || s.starts_with(['@','$']) {return Err("unported source Symbol identifier inspection".into())}
    if label(s)||s.strip_suffix('=').is_some_and(label)||["+","-","*","/","%","**","<",">","<=",">=","==","===","!=","=~","!~","<=>","<<",">>","&","|","^","~","!","[]","[]=","+@","-@"].contains(&s){Ok(format!(":{s}"))}else{Ok(format!(":{}",quote(s)))}
}
pub fn to_s(v:&RubyValue)->Result<String,String>{
    match v {
        RubyValue::Nil=>Ok(String::new()),RubyValue::Bool(v)=>Ok(v.to_string()),RubyValue::Integer(v)=>Ok(v.clone()),RubyValue::String(v)|RubyValue::Symbol(v)=>Ok(v.clone()),
        RubyValue::Array(_)|RubyValue::Hash(_)=>inspect(v),
        RubyValue::Float(_)=>Err("unported source-qualified Ruby Float#to_s".into()),
        RubyValue::Node{..}=>Err("unported identity-bearing Ruby object inspection".into()),
    }
}
pub fn inspect(v:&RubyValue)->Result<String,String>{
    Ok(match v {
        RubyValue::Nil=>"nil".into(),RubyValue::Bool(v)=>v.to_string(),RubyValue::Integer(v)=>v.clone(),RubyValue::String(v)=>quote(v),RubyValue::Symbol(v)=>symbol_inspect(v)?,
        RubyValue::Array(values)=>format!("[{}]",values.iter().map(inspect).collect::<Result<Vec<_>,_>>()?.join(", ")),
        RubyValue::Hash(entries)=>format!("{{{}}}",entries.iter().map(|(k,v)|Ok(if let RubyValue::Symbol(s)=k {if !s.is_ascii(){return Err("unported source Symbol identifier inspection".into())} if label(s){format!("{s}: {}",inspect(v)?)}else{format!("{}: {}",quote(s),inspect(v)?)}}else{format!("{} => {}",inspect(k)?,inspect(v)?)})).collect::<Result<Vec<_>,String>>()?.join(", ")),
        RubyValue::Float(_)=>return Err("unported source-qualified Ruby Float#inspect".into()),
        RubyValue::Node{..}=>return Err("unported identity-bearing Ruby object inspection".into()),
    })
}
pub fn eql(a:&RubyValue,b:&RubyValue)->bool{ equal_impl(a,b,false) }
pub fn equal(a:&RubyValue,b:&RubyValue)->bool{ equal_impl(a,b,true) }
fn equal_impl(a:&RubyValue,b:&RubyValue,loose:bool)->bool{
    use RubyValue::*;
    match(a,b){
        (Float(a),Float(b))=>f64::from_bits(*a)==f64::from_bits(*b),
        (Integer(i),Float(f))|(Float(f),Integer(i)) if loose=>{let f=f64::from_bits(*f);f.is_finite()&&f.fract()==0.0&&if f==0.0{i=="0"}else{&format!("{f:.0}")==i}},
        (Array(a),Array(b))=>a.len()==b.len()&&a.iter().zip(b).all(|(a,b)|equal_impl(a,b,loose)),
        (Hash(a),Hash(b))=>a.len()==b.len()&&a.iter().all(|(k,v)|b.iter().any(|(kb,vb)|eql(k,kb)&&equal_impl(v,vb,loose))),
        _=>a==b,
    }
}
pub fn encode(v:&RubyValue)->Value{
    use serde_json::json;
    match v{
        RubyValue::Nil=>json!(["nil"]),RubyValue::Bool(v)=>json!(["bool",v]),RubyValue::Integer(v)=>json!(["integer",v]),RubyValue::Float(v)=>json!(["float",format!("{v:016x}")]),RubyValue::String(v)=>json!(["string",v]),RubyValue::Symbol(v)=>json!(["symbol",v]),
        RubyValue::Array(v)=>json!(["array",v.iter().map(encode).collect::<Vec<_>>()]),RubyValue::Hash(v)=>json!(["hash",v.iter().map(|(k,v)|json!([encode(k),encode(v)])).collect::<Vec<_>>()]),
        RubyValue::Node{class,fields}=>json!(["node",class,fields.iter().map(|(k,v)|json!([k,encode(v)])).collect::<Vec<_>>()]),
    }
}
fn decode_tag(v:&Value,depth:usize)->Result<RubyValue,String>{
    if depth>128{return Err("source value depth limit exceeded".into())}
    let a=v.as_array().ok_or("source value must be a tagged array")?;
    let tag=a.first().and_then(Value::as_str).ok_or("missing source value tag")?;
    let expected=if tag=="nil"{1}else if tag=="node"{3}else{2};if a.len()!=expected{return Err("invalid source tag arity".into())}
    let string=||a[1].as_str().ok_or_else(||"source tag payload must be a string".to_owned());
    Ok(match tag{
        "nil"=>RubyValue::Nil,"bool"=>RubyValue::Bool(a[1].as_bool().ok_or("source Bool payload must be boolean")?),"integer"=>RubyValue::Integer(integer(string()?)?),"string"=>RubyValue::String(string()?.into()),"symbol"=>RubyValue::Symbol(string()?.into()),
        "float"=>{let s=string()?;if s.len()!=16||!s.bytes().all(|c|c.is_ascii_hexdigit()){return Err("source Float requires 16 IEEE hex digits".into())}RubyValue::Float(u64::from_str_radix(s,16).map_err(|_|"invalid source Float bits")?)},
        "array"=>RubyValue::Array(a[1].as_array().ok_or("source Array payload must be an array")?.iter().map(|v|decode_tag(v,depth+1)).collect::<Result<_,_>>()?),
        "hash"=>{let mut entries=vec![];for pair in a[1].as_array().ok_or("source Hash payload must be entries")?{let pair=pair.as_array().filter(|p|p.len()==2).ok_or("invalid source Hash entry")?;let key=decode_tag(&pair[0],depth+1)?;if matches!(key,RubyValue::Hash(_)|RubyValue::Array(_)|RubyValue::Node{..}){return Err("mutable/identity source Hash keys are not supported".into())}if entries.iter().any(|(k,_)|eql(k,&key)){return Err("duplicate source Hash key".into())}entries.push((key,decode_tag(&pair[1],depth+1)?));}RubyValue::Hash(entries)},
        "node"=>{let class=string()?.to_owned();if !class.starts_with("Math::"){return Err("source node class must use Math:: namespace".into())}let mut fields=vec![];for pair in a[2].as_array().ok_or("source node fields must be entries")?{let pair=pair.as_array().filter(|p|p.len()==2).ok_or("invalid source node field")?;let key=pair[0].as_str().ok_or("source field name must be a string")?.to_owned();if fields.iter().any(|(k,_)|k==&key){return Err("duplicate source node field".into())}fields.push((key,decode_tag(&pair[1],depth+1)?));}RubyValue::Node{class,fields}},
        _=>return Err(format!("unknown source value tag {tag}")),
    })
}
