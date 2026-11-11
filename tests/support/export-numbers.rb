require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
METHODS={"asciimath"=>:to_asciimath,"latex"=>:to_latex,"unicodemath"=>:to_unicodemath,"html"=>:to_html,"mathml"=>:to_mathml,"omml"=>:to_omml}.freeze
values={"nil"=>nil,"false"=>false,"true"=>true,"empty"=>"","zero"=>"0","one"=>"1","nine"=>"9","two-digits"=>"12","negative"=>"-2","decimal"=>"1.25","scientific"=>"1e3","entity"=>"&#x31;","text"=>"a<&>β","whitespace"=>" \n2 ","integer"=>2,"big-integer"=>9007199254740993,"huge-integer"=>(2**100),"float"=>1.25,"float-whole"=>1.0,"negative-zero"=>-0.0,"float-small"=>0.00001,"float-large"=>1e20,"array"=>["1","2"],"nil-array"=>["1",nil,"2"],"nested-array"=>[["1"],["2"]],"hash"=>{"k"=>1},"symbol"=>:"2"}
rows=[]
def capture(id,node,root,options={})
 model=CorpusGenerator.serialize_node(root,"$");outputs=METHODS.transform_values do |method|
  object=Marshal.load(Marshal.dump(root))
  begin;{"output"=>object.public_send(method,**options)};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
 end
 {"id"=>id,"model"=>model,"writers"=>outputs,"getter_value_type"=>node.value.class.name,"getter_base_type"=>node.base.class.name,"mutated"=>model!=CorpusGenerator.serialize_node(root,"$")}
end
values.each do |name,value|
 [[false,false],[true,false],[false,true],[true,true]].each do |sub,sup|
  [nil,2,16].each do |base|
   ["initialize","setter"].each do |construction|
    node=M::Number.new(value,mini_sub_sized:sub,mini_sup_sized:sup,base:base)
    node.value=value if construction=="setter"
    root=M::Formula.new([node]);rows<<capture("#{name}/#{sub}-#{sup}/base#{base.inspect}/#{construction}",node,root)
   end
  end
 end
end
# Formula's mini-leading and negation-sensitive joining are source behavior.
%w[0 2 9 12].each do |value|
 node=M::Number.new(value,mini_sup_sized:true)
 root=M::Formula.new([node,M::Symbols::Symbol.new("q")]);rows<<capture("formula-leading-#{value}",node,root)
 root=M::Formula.new([M::Symbols::Symbol.new("q"),node]);rows<<capture("formula-trailing-#{value}",node,root)
 root=M::Formula.new([M::Formula.new([node]),M::Symbols::Symbol.new("q")]);rows<<capture("formula-nested-leading-#{value}",node,root)
end
source=ENV.fetch("PLURIMATH_SOURCE_ROOT");head=IO.popen(["git","-C",source,"rev-parse","HEAD"],&:read).strip;raise "dirty reference"unless IO.popen(["git","-C",source,"status","--porcelain"],&:read).empty?
data={"profile"=>"source-number-default/1","source"=>head,"ruby"=>RUBY_DESCRIPTION,"engine"=>"Ox","default_formatter"=>Plurimath.configuration.number_formatter.inspect,"state_construction"=>"Actual source initializer and value= setter; independent Marshal clone per source writer","rows"=>rows};File.binwrite(ARGV.fetch(0),JSON.generate(data));File.binwrite(ARGV.fetch(1),JSON.generate({"source"=>head,"producer_sha256"=>Digest::SHA256.file(__FILE__).hexdigest,"mini_sub"=>Plurimath::UnicodeMath::Constants::SUB_DIGITS,"mini_sup"=>Plurimath::UnicodeMath::Constants::SUP_DIGITS,"source_number_sha256"=>Digest::SHA256.file(M::Number.instance_method(:to_latex).source_location.first).hexdigest}));puts JSON.generate(cases:rows.size,outputs:rows.sum{|row|row["writers"].values.count{|v|v.key?("output")}},mutations:rows.count{|row|row["mutated"]})
