require "json"
require "digest"
require "prism"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M = Plurimath::Math
METHODS = {"asciimath"=>:to_asciimath,"latex"=>:to_latex,"unicodemath"=>:to_unicodemath,"html"=>:to_html,"mathml"=>:to_mathml,"omml"=>:to_omml}.freeze
SOURCE = File.expand_path("../..", File.dirname(M::Function::Sum.instance_method(:to_latex).source_location.first))
def sym(value) = M::Symbols::Symbol.new(value)
def num(value, **options) = M::Number.new(value, **options)
def fence = M::Function::Fenced.new(M::Symbols::Lparen.new, [sym("y"),M::Symbols::Minus.new,num("7")], M::Symbols::Rparen.new)
def compound = M::Formula.new([sym("a"),M::Symbols::Plus.new,M::Function::Frac.new(num("3"),sym("z"))])
def record(klass, id, args, hide: false, node_options: {}, writer_options: {})
 node=M::Function.const_get(klass).new(*args)
 node.hide_function_name=hide
 node.options=node_options
 root=M::Formula.new([node]);model=CorpusGenerator.serialize_node(root,"$")
 source_option_mutations=[]
 writers=METHODS.transform_values do |method|
  call_options=Marshal.load(Marshal.dump(writer_options))
  begin;{"output"=>root.public_send(method, **call_options)};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};ensure;source_option_mutations<<method.to_s unless call_options==writer_options;end
 end
 {"id"=>"#{klass}/#{id}","model"=>model,"writer_options"=>writer_options,"source_writer_options_mutated"=>source_option_mutations,"writers"=>writers,"mutated"=>model!=CorpusGenerator.serialize_node(root,"$")}
end
rows=[];data={}
%w[Sum Prod Int Oint].each do |klass|
 node=M::Function.const_get(klass).new
 bare=M::Formula.new([node]);body=M::Formula.new([M::Function.const_get(klass).new(nil,nil,sym("__marker__"))])
 data[klass]={"ascii"=>node.to_asciimath(options:{}),"latex"=>node.to_latex(options:{}),"unicode"=>node.to_unicodemath(options:{}),"mathml_operator"=>node.invert_unicode_symbols.to_s,"omml_operator_text"=>bare.to_omml[/<m:t>(.*?)<\/m:t>/m,1],"html_operator"=>node.to_html(options:{}),"lim_location"=>body.to_omml[/<m:limLoc m:val="([^"]*)"/,1]}
 profiles={
  "unseen-compounds"=>[compound,fence,M::Function::Frac.new(num("4"),sym("q"))],
  "unseen-recursive-body"=>[sym("i"),num("9"),M::Function::Sum.new(sym("j"),num("4"),sym("z"))],
  "empty-array-lower"=>[[],num("3"),sym("r")],
  "empty-array-body"=>[num("3"),sym("u"),[]],
  "nil-array-bound"=>[[sym("j"),nil],num("3"),sym("r")],
  "nil-array-body"=>[num("3"),sym("u"),[sym("j"),nil]],
  "false-only"=>[false,nil,nil],
  "true-bound"=>[true,num("3"),sym("r")],
  "false-array-body"=>[num("3"),sym("u"),[sym("j"),false]],
  "power-base-bounds"=>[M::Function::Base.new(sym("a"),num("3")),M::Function::Power.new(sym("b"),num("4")),sym("q")],
  "prime-upper"=>[sym("j"),sym("&#x27;"),sym("q")],
  "unicode-prime-upper"=>[sym("j"),sym("′"),sym("q")],
  "mini-number-bounds"=>[num("3",mini_sub_sized:true),num("2",mini_sup_sized:true),sym("q")],
  "mini-formula-bound"=>[sym("j"),M::Formula.new([num("2",mini_sup_sized:true)]),sym("q")],
  "mini-formula-body-mixed"=>[sym("j"),num("3"),M::Formula.new([num("2",mini_sup_sized:true),sym("z")])],
  "mini-formula-bound-suffix"=>[M::Formula.new([sym("z"),num("2",mini_sup_sized:true)]),num("3"),sym("q")],
  "mini-formula-bound-negated"=>[M::Formula.new([sym("z"),num("2",mini_sup_sized:true),sym("&#x338;")]),num("3"),sym("q")],
  "mini-number-nil-glyph"=>[num("12",mini_sub_sized:true),num("12",mini_sup_sized:true),sym("q")],
  "entity-text-body"=>[num("3"),sym("u"),M::Function::Text.new("a<&>β")]
 }
 profiles.each{|id,args|[false,true].each{|hide|rows<<record(klass,"#{id}/hide#{hide}",args,hide:hide)}}
 [nil,false,0,-5,"-17tail","bad",1.5,[],{"key"=>"value"}].each_with_index{|mask,i|rows<<record(klass,"mask-type-#{i}",[sym("a"),num("3"),sym("r")],node_options:{mask:mask})}
 rows<<record(klass,"caller-mask",[sym("a"),num("3"),sym("r")],writer_options:{options:{mask:"7"}})
end
32.times do |mask|
 [[nil,num("2"),nil],[num("3"),nil,nil],[nil,nil,sym("t")]].each_with_index do |args,i|
  rows<<record("Int","mask-bits-#{mask}-shape#{i}",args,node_options:{mask:mask})
 end
end
source_files=%w[sum prod int oint ternary_function binary_function nary].map{|name|M::Function.const_get({"ternary_function"=>"TernaryFunction","binary_function"=>"BinaryFunction","nary"=>"Nary"}.fetch(name,name.capitalize)).instance_method(name=="ternary_function" ? :narypr : :to_asciimath).source_location.first}.uniq
source_files << M::Core.instance_method(:masked_tag).source_location.first
source_files << M::Number.instance_method(:to_unicodemath).source_location.first
# Use only proven literal predicate methods as static metadata; value-sensitive
# predicates are left explicit, rather than sampled and treated as constants.
wrap_static={};wrap_dynamic={};asts={}
classes=JSON.parse(File.read(ENV.fetch("PLURIMATH_CLASS_INVENTORY"),encoding:"UTF-8")).fetch("rows").map do |row|
 row.fetch("class").split("::").inject(Plurimath){|scope,name|scope.const_get(name,false)}
end
classes.each do |klass|
 method=klass.instance_method(:validate_function_formula);file,line=method.source_location
 source_files<<file
 stack=[asts[file]||=Prism.parse_file(file).value];found=nil
 until stack.empty?
  node=stack.pop
  if node.is_a?(Prism::DefNode)&&node.name==method.name&&node.location.start_line==line;found=node;break;end
  stack.concat(node.compact_child_nodes)
 end
 body=found&.body&.body
 name=klass.name.delete_prefix("Plurimath::")
 if body&.length==1&&[Prism::TrueNode,Prism::FalseNode].any?{|type|body.first.is_a?(type)}
  wrap_static[name]=klass.allocate.public_send(:validate_function_formula)
 else
  wrap_dynamic[name]={"owner"=>method.owner.name,"line"=>line}
 end
end

source_root=ENV.fetch("PLURIMATH_SOURCE_ROOT")
head=IO.popen(["git","-C",source_root,"rev-parse","HEAD"], &:read).strip
status=IO.popen(["git","-C",source_root,"status","--porcelain"], &:read)
raise "reference checkout dirty" unless status.empty?
meta={"profile"=>"source-nary-constants/1","source"=>head,"producer_sha256"=>Digest::SHA256.file(__FILE__).hexdigest,"classes"=>data,"mini_sub"=>Plurimath::UnicodeMath::Constants::SUB_DIGITS,"mini_sup"=>Plurimath::UnicodeMath::Constants::SUP_DIGITS,"primes"=>Plurimath::Utility.primes_constants.values.uniq,"latex_wrap_static"=>wrap_static,"latex_wrap_dynamic"=>wrap_dynamic,"left_classes"=>classes.select{|k|k<=M::Function::Left}.map{|k|k.name.delete_prefix("Plurimath::")},"right_classes"=>classes.select{|k|k<=M::Function::Right}.map{|k|k.name.delete_prefix("Plurimath::")},"prime_inputs"=>M::Symbols::Symbol.descendants.sort_by(&:name).to_h{|k|[k.name.delete_prefix("Plurimath::"),k.new.input(:unicodemath)&.flatten&.find{|v|v.match?(/&#x.+;/)}]},"prime_symbol_classes"=>[M::Symbols::Symbol,*M::Symbols::Symbol.descendants].map{|c|c.name.delete_prefix("Plurimath::")}.sort,"source_files"=>source_files.uniq.sort.to_h{|path|[path.delete_prefix(source_root+"/"),Digest::SHA256.file(path).hexdigest]}}
File.binwrite(ARGV.fetch(0),JSON.generate(meta))
raise "source model mutated" if rows.any?{|row|row["mutated"]}
File.binwrite(ARGV.fetch(1),JSON.generate({"profile"=>"source-nary-independent/2","source"=>head,"ruby"=>RUBY_DESCRIPTION,"rows"=>rows}))
puts JSON.generate(cases:rows.size,outputs:rows.sum{|row|row["writers"].values.count{|v|v.key?("output")}},exceptions:rows.sum{|row|row["writers"].values.count{|v|v.key?("error")}},mutations:0)
