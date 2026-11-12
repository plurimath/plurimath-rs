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
[["zero","0.000","0.000"],["leading","00012.400","00.007"],["negative","-0.100","0.010"],["huge","900719925474099312345.000","0.123"]].each do|name,v,m|
 [:absolute,:concise].each{|notation|cases<<["standard/#{name}/#{notation}",->{standard(v,m,notation:notation)}]}
end
[:direct,:multiletter,:formula,:power,:fallback].each{|kind|cases<<["standard/unit/#{kind}",->{standard(unit:unit(kind))}]}
[false,true].each do|prob|
 cases<<["expanded/prob#{prob}",->{U.new(num("-10.00"),category: :expanded,magnitude:num("0.300"),coverage_factor:num("2.000"),coverage_probability:(num("0.950") if prob),unit:unit(:multiletter))}]
end
[nil,:shortest,:probabilistically_symmetric].each{|kind|cases<<["interval/#{kind}",->{U.new(num("-1.000"),category: :coverage_interval,lower:num("0.005"),upper:num("10.000"),coverage_probability:num("0.9750"),interval_kind:kind,unit:unit(:multiletter))}]}
# Setters bypass constructor checks in source. Preserve observable blank/malformed states separately.
[[:value,nil],[:value,false],[:magnitude,nil],[:magnitude,false],[:magnitude,num("000.000")],[:unit,false],[:category,nil],[:notation,:concise]].each do|field,value|
 cases<<["setter/#{field}/#{value.inspect.gsub(/0x[0-9a-f]+/,"ADDRESS")}",->{node=standard;node.public_send("#{field}=",value);node}]
end
rows=[]
cases.each_with_index do|(name,make),i|
 [{},{display_style:false},{intent:true}].each_with_index do|options,oi|
  node=make.call;root=M::Formula.new([node]);model=CorpusGenerator.serialize_node(root,"$")
  writers=METHODS.transform_values do|method|
   begin;{"output"=>((method==:to_mathml || method==:to_omml) ? root.public_send(method,**options) : root.public_send(method,options:options.dup))};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
  end
  rows<<{"id"=>"independent/#{i}/#{name}/options#{oi}","model"=>model,"options"=>options,"writers"=>writers}
 end
end
File.write(ARGV.fetch(0),JSON.generate({"ruby"=>RUBY_DESCRIPTION,"source"=>"0a1df68a5903d43da13b1e1ef85b6c0523feb1b4","profile"=>"uncertainty-writers-source-v1","rows"=>rows}))
puts JSON.generate(rows:rows.size,outputs:rows.sum{|r|r["writers"].values.count{|w|w.key?("output")}},errors:rows.sum{|r|r["writers"].values.count{|w|w.key?("error")}})
