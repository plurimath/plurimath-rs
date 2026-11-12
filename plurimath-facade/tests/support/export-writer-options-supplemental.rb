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
[nil,'('].each do |left|
 [nil,')'].each do |right|
  %w[nil empty simple break nested-break].each do |shape|
   [false,true].each do |split|
    body=case shape
     when 'empty' then []
     when 'simple' then [sym('a')]
     when 'break' then [sym('a'),M::Function::Linebreak.new,sym('b')]
     when 'nested-break' then [M::Formula.new([sym('a'),M::Function::Linebreak.new,sym('b')])]
    end
    record(rows,"fence-null/#{left.inspect}/#{right.inspect}/#{shape}/#{split}",[M::Function::Fenced.new(left,body,right)],{intent:true,split_on_linebreak:split})
   end
  end
 end
end
[true,false].each do |split|
 %w[Abs Ceil Floor Norm Sqrt Hat Vec Dot Ddot Tilde Ubrace Bar].each do |kind|
  child=M::Formula.new([sym('a'),M::Function::Linebreak.new,sym('b'),sym('c')])
  record(rows,"multi-tail/#{kind}/#{split}",[M::Function.const_get(kind).new(child)],{intent:true,split_on_linebreak:split,display_style:false})
 end
 [true,false].each do |spacing|
  color=M::Function::Color.new(M::Function::Text.new('red'),M::Formula.new([M::Function::Sin.new(M::Function::Hat.new(sym)),M::Function::Linebreak.new,sym('b')]))
  record(rows,"color-node/#{split}/#{spacing}",[color],{intent:true,split_on_linebreak:split,unary_function_spacing:spacing,display_style:false})
 end
end
File.write(ARGV[0],JSON.pretty_generate({ruby:RUBY_DESCRIPTION,source:'0a1df68a5903d43da13b1e1ef85b6c0523feb1b4',rows:rows})+"\n", encoding: "UTF-8")
puts JSON.generate(rows:rows.size)
