require 'json'
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR", "/home/apple/projects/plurimath-org/workspace/core/ts-ci-fix/scripts/generate-corpus.rb")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
rows=[]
def sym(s='x'); M::Symbols::Symbol.new(s); end
def record(rows,id,nodes,options={},ds:true)
 root=M::Formula.new(nodes,display_style:ds)
 model=CorpusGenerator.serialize_node(root,'$')
 %w[mathml omml].each do |format|
  opts=options.select{|k,v| format=='mathml' || [:display_style,:split_on_linebreak].include?(k)}
  out=begin;{'output'=>root.public_send("to_#{format}",**opts)};rescue StandardError=>e;{'error'=>{'class'=>e.class.name,'message'=>e.message}};end
  rows << {'id'=>id+'/'+format,'model'=>model,'format'=>format,'options'=>opts.transform_keys{|k| {display_style:'displayStyle',unary_function_spacing:'unaryFunctionSpacing',split_on_linebreak:'splitOnLinebreak'}[k] || k.to_s},'expected'=>out}
 end
end
[true,false].each do |ds|
 %w[Hat Vec Dot Ddot Tilde Ubrace Bar].each do |outer|
  [nil,{}, {accent:true}].each_with_index do |a,ai|
   [false,true].each do |inner_accent|
    inner=M::Function::Hat.new(sym);inner.attributes={accent:true} if inner_accent
    node=M::Function.const_get(outer).new(inner);node.attributes=a unless a.nil? || !node.respond_to?(:attributes=)
    record(rows,"nested/#{outer}/#{ai}/#{inner_accent}/#{ds}",[node],{display_style:ds})
   end
  end
 end
 %w[Sin Cos Ln Exp].each do |klass|
  [true,false,nil].each do |spacing|
   record(rows,"unary/#{klass}/#{spacing}/#{ds}",[M::Function.const_get(klass).new(M::Function::Hat.new(sym))],{display_style:ds,unary_function_spacing:spacing,intent:true})
  end
 end
end
%w[Sin Hat Vec Bar Abs Sqrt].each do |klass|
 record(rows,"intent/#{klass}",[M::Function.const_get(klass).new(M::Formula.new([sym('f'),M::Function::Fenced.new('(',[sym('x')],')')]))],{intent:true})
end
%w[Ii Jj Dd UpcaseDd Intercal].each do |klass|
 record(rows,"intent-symbol/#{klass}",[M::Symbols.const_get(klass).new],{intent:true})
end
[false,true].each do |split|
 record(rows,"linebreak/basic/#{split}",[sym('a'),M::Function::Linebreak.new,sym('b')],{split_on_linebreak:split,intent:true})
 %w[after before duplicate].each do |style|
  record(rows,"linebreak/#{style}/#{split}",[sym('a'),M::Function::Linebreak.new(M::Symbols::Plus.new,{linebreakstyle:style}),sym('b')],{split_on_linebreak:split})
 end
 %w[Hat Vec Sin Abs Sqrt].each do |klass|
  record(rows,"linebreak/nested/#{klass}/#{split}",[M::Function.const_get(klass).new(M::Formula.new([sym('a'),M::Function::Linebreak.new,sym('b')]))],{split_on_linebreak:split,intent:true})
 end
end
File.write(ARGV[0],JSON.pretty_generate({ruby:RUBY_DESCRIPTION,source:'0a1df68a5903d43da13b1e1ef85b6c0523feb1b4',rows:rows})+"\n", encoding: "UTF-8")
puts JSON.generate(rows:rows.size)
