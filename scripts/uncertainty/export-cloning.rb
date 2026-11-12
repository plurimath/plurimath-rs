require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
U=M::Function::Uncertainty
expected="c558519097f56a6e67cb7873cd9e33b12317b628db4fb20bd2cb2fed42416199"
raise "uncertainty source differs from pinned oracle" unless Digest::SHA256.file(U.instance_method(:to_asciimath).source_location.first).hexdigest==expected
METHODS={"asciimath"=>:to_asciimath,"latex"=>:to_latex,"unicodemath"=>:to_unicodemath,"html"=>:to_html,"mathml"=>:to_mathml,"omml"=>:to_omml}.freeze
def num(s);M::Number.new(s);end
def standard(v="-001.230",m="0.004",**kw);U.new(num(v),category: :standard,magnitude:num(m),**kw);end
def unit(kind)
 case kind
 when :direct then M::Function::Unitsml.new("kg*m*s^-2")
 when :multiletter then M::Function::Unitsml.new("cm")
 when :formula then M::Formula.new([M::Function::FontStyle::Normal.new(M::Symbols::Symbol.new("kg"))])
 when :power then M::Formula.new([M::Function::Power.new(M::Function::FontStyle::Normal.new(M::Symbols::Symbol.new("s")),M::Formula.new([M::Symbols::Minus.new,num("2")]))])
 when :fallback then M::Formula.new([M::Symbols::Symbol.new("x"),M::Symbols::Plus.new,num("2")])
 end
end
cases=[]
def expanded;U.new(num("10.00"),category: :expanded,magnitude:num("0.30"),coverage_factor:num("2.00"));end
def interval;U.new(num("10.00"),category: :coverage_interval,lower:num("1.00"),upper:num("12.00"),coverage_probability:num("0.950"));end
[[:distribution,:unknown],[:notation,:unknown],[:notation,:concise],[:magnitude,nil],[:coverage_factor,num("2")],[:unit,M::Formula.new([])]].each do |key,value|
 cases << ["standard/#{key}/#{value.inspect}",->{node=standard;node.public_send("#{key}=",value);node}]
end
[[:coverage_factor,num("0.000")],[:coverage_factor,nil],[:coverage_probability,num("1.00")],[:coverage_probability,num("0.000")],[:lower,num("1")],[:magnitude,num("-0.1")],[:magnitude,num("bad")]].each do |key,value|
 cases << ["expanded/#{key}/#{value.inspect}",->{node=expanded;node.public_send("#{key}=",value);node}]
end
[[:lower,num("12.00")],[:lower,num("20")],[:coverage_probability,num("0")],[:interval_kind,:unknown],[:magnitude,num("0.3")]].each do|key,value|
 cases << ["interval/#{key}/#{value.inspect}",->{node=interval;node.public_send("#{key}=",value);node}]
end
cases << ["concise/places",->{node=standard;node.notation=:concise;node.magnitude=num("0.0040");node}]
rows=[]
cases.each_with_index do|(name,make),i|
 [{split_on_linebreak:true}].each_with_index do|options,oi|
  node=make.call;root=M::Formula.new([node]);model=CorpusGenerator.serialize_node(root,"$")
  writers=METHODS.select{|format,_|["mathml","omml"].include?(format)}.transform_values do|method|
   begin;{"output"=>((method==:to_mathml || method==:to_omml) ? root.public_send(method,**options) : root.public_send(method,options:options.dup))};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
  end
  rows<<{"id"=>"independent/#{i}/#{name}/options#{oi}","model"=>model,"options"=>options,"writers"=>writers}
 end
end
File.write(ARGV.fetch(0),JSON.generate({"ruby"=>RUBY_DESCRIPTION,"source"=>"0a1df68a5903d43da13b1e1ef85b6c0523feb1b4","profile"=>"uncertainty-writers-source-v1","rows"=>rows}))
puts JSON.generate(rows:rows.size,outputs:rows.sum{|r|r["writers"].values.count{|w|w.key?("output")}},errors:rows.sum{|r|r["writers"].values.count{|w|w.key?("error")}})
