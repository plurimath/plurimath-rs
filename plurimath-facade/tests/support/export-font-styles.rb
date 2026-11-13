require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
METHODS={"asciimath"=>:to_asciimath,"latex"=>:to_latex,"unicodemath"=>:to_unicodemath,"html"=>:to_html,"mathml"=>:to_mathml,"omml"=>:to_omml}
def sym(s);M::Symbols::Symbol.new(s);end
def num(s);M::Number.new(s);end
def fm(a);M::Formula.new(a);end
def wire(v);CorpusGenerator.serialize_node(v,"$");end
def child(name)
 case name
 when "nil" then nil
 when "x" then sym("x")
 when "word" then sym("AB")
 when "greek" then sym("α")
 when "entity" then sym("a&amp;b")
 when "number" then num("1.25")
 when "row" then fm([sym("x"),M::Symbols::Plus.new,num("2")])
 when "fraction" then M::Function::Frac.new(fm([num("3")]),fm([sym("y")]))
 when "fenced" then M::Function::Fenced.new(sym("("),[sym("z")],sym(")"))
 when "text" then M::Function::Text.new("a <&> β")
 when "normal" then M::Function::FontStyle::Normal.new(sym("q"))
 when "bold" then M::Function::FontStyle::Bold.new(sym("q"))
 when "array" then [sym("x")]
 when "string" then "raw"
 when "false" then false
 end
end
classes=Plurimath::Utility::FONT_STYLES.values.uniq.sort_by(&:name)
metadata={};rows=[]
classes.each do |klass|
 name=klass.name.sub(/^Plurimath::/,"");arg=sym("prototype")
 %i[to_asciimath to_latex to_unicodemath to_html].each { |method|arg.define_singleton_method(method){|**_|"PMFONTCHILD"} }
 node=klass.new(arg);entry={"aliases"=>Plurimath::Utility::FONT_STYLES.select{|_,v|v==klass}.keys.map(&:to_s),"default_parameter_two"=>wire(node)["fields"]["parameter_two"],"text"=>{}}
 %w[asciimath latex unicodemath html].each do |f|
  out=node.public_send(METHODS.fetch(f),options:{})
  raise "marker multiplicity" unless out.scan("PMFONTCHILD").length==1
  first,last=out.split("PMFONTCHILD",-1);entry["text"][f]={"prefix"=>first,"suffix"=>last}
 end
 x=klass.new(nil).to_mathml_without_math_tag(false,options:{})
 entry["mathvariant"]=x.attributes[:mathvariant]||x.attributes["mathvariant"]
 entry["unicode_family"]=klass.new(nil).send(:font_family,unicode:true)
 omml=Array(klass.new(nil).to_omml_without_math_tag(true,options:{})).flatten.first
 entry["omml_properties"]=omml.nodes.first.nodes.map{|n|[n.name,n.attributes["m:val"]||n.attributes[:"m:val"]]}
 entry["method_sources"]=%i[initialize to_asciimath to_latex to_unicodemath to_html to_mathml_without_math_tag to_omml_without_math_tag].to_h{|m|method=klass.instance_method(m);[m,{"owner"=>method.owner.name,"location"=>method.source_location,"file_sha256"=>method.source_location&&Digest::SHA256.file(method.source_location.first).hexdigest}]}
 metadata[name]=entry
 %w[nil x word greek entity number row fraction fenced text normal bold array string false].each do |profile|
  node=klass.new(child(profile));formula=fm([node]);before=wire(formula)
  outputs=METHODS.transform_values do |method|
   begin
    {"output"=>formula.public_send(method)}
   rescue StandardError=>error
    {"error"=>{"class"=>error.class.name,"message"=>error.message}}
   end
  end
  rows<<{"id"=>"#{name}:#{profile}","model"=>before,"writers"=>outputs,"mutated"=>wire(formula)!=before}
 end
end
%w[bold mathit monospace unknown-font].each do |token|
 %w[nil x row fraction text].each do |profile|
  formula=fm([M::Function::FontStyle.new(child(profile),token)]);before=wire(formula)
  outputs=METHODS.transform_values do |method|
   begin;{"output"=>formula.public_send(method)};rescue StandardError=>error;{"error"=>{"class"=>error.class.name,"message"=>error.message}};end
  end
  rows<<{"id"=>"FontStyle:#{token}:#{profile}","model"=>before,"writers"=>outputs,"mutated"=>wire(formula)!=before}
 end
end
out=ARGV.fetch(0);File.write(File.join(out,"font-metadata.json"),JSON.pretty_generate(metadata)+"\n");File.write(File.join(out,"font-source-reference.json"),JSON.pretty_generate({"source"=>"0a1df68a5903d43da13b1e1ef85b6c0523feb1b4","ruby"=>RUBY_DESCRIPTION,"xml_engine"=>Plurimath.xml_engine.to_s,"rows"=>rows})+"\n");puts JSON.generate(classes:classes.size,cases:rows.size,outputs:rows.sum{|r|r["writers"].values.count{|v|v.key?("output")}},errors:rows.sum{|r|r["writers"].values.count{|v|v.key?("error")}},mutations:rows.count{|r|r["mutated"]})
