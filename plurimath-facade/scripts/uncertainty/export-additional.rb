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
cases << ["blank-constructor", ->{U.new}]
cases << ["unit-linebreak", ->{standard(unit:M::Formula.new([M::Symbols::Symbol.new("m"),M::Function::Linebreak.new,M::Symbols::Symbol.new("s")]))}]
cases << ["number-value-integer", ->{node=standard;node.value.value=2;node}]
cases << ["number-value-false", ->{node=standard;node.value.value=false;node}]
rows=[]
cases.each_with_index do|(name,make),i|
 [{},{split_on_linebreak:true}].each_with_index do|options,oi|
  node=make.call;root=M::Formula.new([node]);model=CorpusGenerator.serialize_node(root,"$")
  writers=METHODS.transform_values do|method|
   begin;{"output"=>((method==:to_mathml || method==:to_omml) ? root.public_send(method,**options) : root.public_send(method,options:options.dup))};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
  end
  rows<<{"id"=>"independent/#{i}/#{name}/options#{oi}","model"=>model,"options"=>options,"writers"=>writers}
 end
end
File.write(ARGV.fetch(0),JSON.generate({"ruby"=>RUBY_DESCRIPTION,"source"=>"0a1df68a5903d43da13b1e1ef85b6c0523feb1b4","profile"=>"uncertainty-writers-source-v1","rows"=>rows}))
puts JSON.generate(rows:rows.size,outputs:rows.sum{|r|r["writers"].values.count{|w|w.key?("output")}},errors:rows.sum{|r|r["writers"].values.count{|w|w.key?("error")}})
