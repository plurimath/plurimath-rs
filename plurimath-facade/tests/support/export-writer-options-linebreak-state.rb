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
builders={
 'Sin'=>->(x){M::Function::Sin.new(x)}, 'Phantom'=>->(x){M::Function::Phantom.new(x)},
 'Cancel'=>->(x){M::Function::Cancel.new(x)}, 'Normal'=>->(x){M::Function::FontStyle::Normal.new(x)},
 'Color'=>->(x){M::Function::Color.new('red',x)}, 'Frac'=>->(x){M::Function::Frac.new(x,M::Number.new('2'))},
 'Over'=>->(x){M::Function::Over.new(x,M::Number.new('2'))}, 'Power'=>->(x){M::Function::Power.new(x,M::Number.new('2'))},
 'Base'=>->(x){M::Function::Base.new(x,M::Number.new('2'))},
}
%w[Abs Ceil Floor Norm Sqrt Hat Vec Dot Ddot Tilde Ubrace Bar].each{|k|builders[k]=->(x){M::Function.const_get(k).new(x)}}
builders.each do |name,builder|
 %w[formula array nested-array nil].each do |shape|
  [false,true].each do |split|
   x=case shape
    when 'formula' then M::Formula.new([sym('a'),M::Function::Linebreak.new,sym('b')])
    when 'array' then [sym('a'),M::Function::Linebreak.new,sym('b')]
    when 'nested-array' then [M::Formula.new([sym('a'),M::Function::Linebreak.new,sym('b')]),M::Function::Sin.new(sym('c'))]
    end
   record(rows,"split-context/#{name}/#{shape}/#{split}",[builder.call(x)],{split_on_linebreak:split,unary_function_spacing:false,intent:true})
  end
 end
end
File.write(ARGV[0],JSON.pretty_generate({ruby:RUBY_DESCRIPTION,source:'0a1df68a5903d43da13b1e1ef85b6c0523feb1b4',rows:rows})+"\n", encoding: "UTF-8")
puts JSON.generate(rows:rows.size)
