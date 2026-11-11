require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
METHODS={"asciimath"=>:to_asciimath,"latex"=>:to_latex,"unicodemath"=>:to_unicodemath,"html"=>:to_html,"mathml"=>:to_mathml,"omml"=>:to_omml}.freeze
def sym(x)=M::Symbols::Symbol.new(x)
def num(x)=M::Number.new(x)
def tr(*items)=M::Function::Tr.new(items.map{|x|M::Function::Td.new(Array(x))})
rows=[]
profiles={"nil"=>nil,"empty"=>[],"one-row"=>[tr(sym("x"))],"two-rows"=>[tr(sym("a")),tr(num("7"))],"two-columns"=>[tr(sym("x"),num("3")),tr(num("8"),sym("z"))],"nil-row"=>[nil,tr(sym("n"))],"direct-symbol"=>[sym("z")],"direct-formula"=>[M::Formula.new([sym("x"),M::Symbols::Plus.new,num("2")])],"empty-row"=>[M::Function::Tr.new([])],"nested-table"=>[tr(M::Function::Substack.new([tr(sym("q"))]))],"false-row"=>[false],"raw-string"=>"literal"}
profiles.each{|id,v|rows<<["Substack/#{id}",M::Function::Substack.new(v)]}
[nil,[],sym("x"),"raw"].each_with_index{|v,i|rows<<["Msline/#{i}",M::Function::Msline.new(v)]}
[{}, {alt:"x"}, {alt:"<&>\"β",src:"data:x&y",width:"2em"}, {alt:"fallback",index:"65"},{alt:"fallback",index:"0"},{alt:"fallback",index:"31"},{alt:"fallback",index:"9"},{alt:"fallback",index:"65535"},{alt:"fallback",index:"-1"},{alt:"fallback",index:"128512"},{alt:"fallback",index:"65tail"},{alt:"fallback",index:nil}].each_with_index{|v,i|rows<<["Mglyph/#{i}",M::Function::Mglyph.new(v)]}
[[],[nil],[sym("x"),num("3")],[sym("|"),sym("y")],[M::Formula.new([sym("a"),M::Symbols::Plus.new,num("2")])]].each_with_index{|v,i|rows<<["Td/direct#{i}",M::Function::Td.new(v)]}
[[],[M::Function::Td.new([sym("x")])],[M::Function::Td.new([sym("| ")]),M::Function::Td.new([num("7"),sym("q")])]].each_with_index{|v,i|rows<<["Tr/direct#{i}",M::Function::Tr.new(v)]}
rows=rows.map do |id,node|
 root=M::Formula.new([node]);model=CorpusGenerator.serialize_node(root,"$")
 writers=METHODS.to_h do |format,method|
  target=Marshal.load(Marshal.dump(root))
  result=begin;{"output"=>target.public_send(method)};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
  [format,result]
 end
 {"id"=>id,"model"=>model,"writers"=>writers}
end
head=IO.popen(["git","-C",ENV.fetch("PLURIMATH_SOURCE_ROOT"),"rev-parse","HEAD"],&:read).strip
raise "dirty reference" unless IO.popen(["git","-C",ENV.fetch("PLURIMATH_SOURCE_ROOT"),"status","--porcelain"],&:read).empty?
meta={"profile"=>"source-stacked-writers/1","source"=>head,"producer_sha256"=>Digest::SHA256.file(__FILE__).hexdigest,"rows"=>rows}
File.write(ARGV.fetch(1),JSON.generate(meta))
File.write(ARGV.fetch(0),JSON.generate({"source"=>head,"source_files"=>%w[Substack Msline Mglyph].to_h{|c|p=M::Function.const_get(c).instance_method(:to_mathml_without_math_tag).source_location.first;[p.delete_prefix(ENV.fetch("PLURIMATH_SOURCE_ROOT")+"/"),Digest::SHA256.file(p).hexdigest]}}))
puts JSON.generate(cases:rows.length,outputs:rows.sum{|r|r["writers"].values.count{|v|v.key?("output")}},refusals:rows.sum{|r|r["writers"].values.count{|v|v.key?("error")}})
