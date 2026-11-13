require "json";require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math;F=M::Function
root=Gem.loaded_specs.fetch("plurimath").full_gem_path
raise "wrong source"unless `git -C #{root} rev-parse HEAD`.strip=="0a1df68a5903d43da13b1e1ef85b6c0523feb1b4"
values={nil:nil,false:false,symbol:M::Symbols::Symbol.new("x"),number:M::Number.new("12"),formula:M::Formula.new([M::Symbols::Symbol.new("x"),M::Number.new("12")]),fenced:F::Fenced.new(M::Symbols::Paren::Lround.new,[M::Number.new("12")],M::Symbols::Paren::Rround.new),none:F::None.new,array:[M::Symbols::Symbol.new("x"),M::Number.new("12")],empty:[],nil_array:[nil],string:"raw",integer:1}
rows=[];constructors=[]
def capture(rows,id,node)
 formula=Plurimath::Math::Formula.new([node]);model=CorpusGenerator.serialize_node(formula,"$");snapshot=Marshal.dump(formula)
 writers=%w[asciimath latex html unicodemath mathml omml].to_h do |fmt|
  begin;[fmt,{output:Marshal.load(snapshot).public_send("to_#{fmt}")}]
  rescue StandardError=>error;[fmt,{error:{class:error.class.name,message:error.message,cause_class:error.cause&.class&.name}}];end
 end
 rows<<{id:id,model:model,writers:writers}
end
[F::Sqrt,F::Obrace].each do |klass|
 values.each do |name,value|
  begin;capture(rows,"#{klass.name}-#{name}",klass.new(Marshal.load(Marshal.dump(value))));rescue StandardError=>error;constructors<<{class:klass.name,id:name,error:error.message};end
 end
end
[F::Lim,F::Overset,F::Stackrel].each do |klass|
 values.each do |an,a|;values.each do |bn,b|
  begin;capture(rows,"#{klass.name}-#{an}-#{bn}",klass.new(Marshal.load(Marshal.dump(a)),Marshal.load(Marshal.dump(b))));rescue StandardError=>error;constructors<<{class:klass.name,id:[an,bn],error:error.message};end
 end;end
end
data={diacritic_overlays:Plurimath::UnicodeMath::Constants::DIACRITIC_OVERLAYS}
File.write(ARGV.fetch(0),JSON.pretty_generate({source_head:`git -C #{root} rev-parse HEAD`.strip,runtime:RUBY_DESCRIPTION,rows:rows,constructor_refusals:constructors,constants:data}))
File.write(ARGV.fetch(1),JSON.pretty_generate(data));puts JSON.generate({cases:rows.size,calls:rows.size*6,constructor_refusals:constructors.size})
