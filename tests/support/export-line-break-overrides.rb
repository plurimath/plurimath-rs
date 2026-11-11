require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
source=ENV.fetch("PLURIMATH_SOURCE_ROOT")
head=IO.popen(["git","-C",source,"rev-parse","HEAD"], &:read).strip
raise "Expected clean pinned Ruby source" unless head=="0a1df68a5903d43da13b1e1ef85b6c0523feb1b4" && IO.popen(["git","-C",source,"status","--porcelain"], &:read).empty?
def symbol(v);M::Symbols::Symbol.new(v);end
def clone(v);Marshal.load(Marshal.dump(v));end
def split_formula(value)
 M::Formula.new([symbol(value),M::Function::Linebreak.new,symbol(value.upcase)])
end
shapes={"nil"=>nil,"leaf"=>M::Number.new("17"),"empty"=>M::Formula.new([]),"break"=>split_formula("a"),"multiple"=>M::Formula.new([symbol("b"),M::Function::Linebreak.new,symbol("c"),M::Function::Linebreak.new,symbol("d")]),"unary-break"=>M::Function::Sin.new(split_formula("e")),"fenced-break"=>M::Function::Fenced.new(M::Symbols::Paren::Lround.new,[symbol("f"),M::Function::Linebreak.new,symbol("g")],M::Symbols::Paren::Rround.new),"array"=>[symbol("h"),M::Function::Linebreak.new,symbol("i")],"false"=>false}
rows=[]
states=[]
%w[Frac Overset Mod].each do |name|
 shapes.each do |first_name,first|
  shapes.each do |second_name,second|
   node=M::Function.const_get(name).new(clone(first),clone(second))
   root=M::Formula.new([node])
   model=CorpusGenerator.serialize_node(root,"$")
   [[],[symbol("pending")]].each_with_index do |pending,index|
    object=clone(node);target=M::Formula.new(clone(pending))
    result=begin
     object.line_breaking(target)
     {"node"=>CorpusGenerator.serialize_node(object,"$"),"continuation"=>target.value.map{|v|CorpusGenerator.serialize_value(v,"$")}}
    rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
    states<<{"id"=>"#{name}/#{first_name}/#{second_name}/pending#{index}","model"=>CorpusGenerator.serialize_node(node,"$"),"pending"=>pending.map{|v|CorpusGenerator.serialize_node(v,"$")},"expected"=>result}
   end
   %w[mathml omml].each do |format|
    [false,true].each do |split|
     opts={split_on_linebreak:split}
     result=begin;{"output"=>clone(root).public_send("to_#{format}",**opts)};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
     rows<<{"id"=>"#{name}/#{first_name}/#{second_name}/#{format}/#{split}","model"=>model,"format"=>format,"options"=>{"splitOnLinebreak"=>split},"expected"=>result}
    end
   end
  end
 end
end
files=%w[frac overset mod].to_h{|name|[name,Digest::SHA256.file(File.join(source,"lib/plurimath/math/function/#{name}.rb")).hexdigest]}
File.binwrite(ARGV.fetch(0),JSON.generate({"profile"=>"source-binary-line-break-overrides/1","source"=>head,"ruby"=>RUBY_DESCRIPTION,"engine"=>"Ox","source_sha256"=>files,"rows"=>rows,"states"=>states})+"\n")
File.binwrite(ARGV.fetch(1),JSON.generate({"producer_sha256"=>Digest::SHA256.file(__FILE__).hexdigest})+"\n")
puts JSON.generate({writer_observations:rows.length,state_observations:states.length})
