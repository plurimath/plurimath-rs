require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
def sym(x)=M::Symbols::Symbol.new(x)
def line = M::Function::Linebreak.new(sym("="))
methods={"asciimath"=>:to_asciimath,"latex"=>:to_latex,"html"=>:to_html,"unicodemath"=>:to_unicodemath,"mathml"=>:to_mathml,"omml"=>:to_omml}
bases=[sym("a"),M::Formula.new([sym("a"),line,sym("b")]),nil]
subs=[[],[sym("i")],[line,sym("j")],[sym("i"),line,sym("j")],[line,sym("i"),line,sym("j")],[M::Formula.new([sym("i"),line,sym("j")])],nil]
sups=[[],[sym("n")],[line,sym("n")]]
rows=[]
bases.each_with_index{|base,bi|subs.each_with_index{|sub,si|sups.each_with_index do |sup,ui|
 node=M::Function::Multiscript.new(base,sub,sup);root=M::Formula.new([node]);model=CorpusGenerator.serialize_node(root,"$")
 writers=methods.to_h do |fmt,method|
  target=Marshal.load(Marshal.dump(root));value=begin;{"output"=>target.public_send(method)};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end;[fmt,value]
 end
 rows<<{"id"=>"multi-break/#{bi}/#{si}/#{ui}","model"=>model,"writers"=>writers}
end}}
root=ENV.fetch("PLURIMATH_SOURCE_ROOT");head=IO.popen(["git","-C",root,"rev-parse","HEAD"],&:read).strip;raise "dirty reference" unless IO.popen(["git","-C",root,"status","--porcelain"],&:read).empty?
probe=M::Function::Multiscript.new(sym("x"),[sym("i")],[sym("n")]);meta={"source"=>head,"variables"=>probe.variables.map(&:to_s),"has_hide"=>probe.respond_to?(:hide_function_name),"producer_sha256"=>Digest::SHA256.file(__FILE__).hexdigest}
File.write(ARGV.fetch(0),JSON.generate(meta));File.write(ARGV.fetch(1),JSON.generate({"source"=>head,"profile"=>"multiscript-line-breaking/1","rows"=>rows}));puts JSON.generate(cases:rows.length)
