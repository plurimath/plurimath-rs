require 'json';require 'digest';require 'plurimath'
require ENV.fetch('PLURIMATH_REFERENCE_SERIALIZER')
M=Plurimath::Math;F=M::Function;x=M::Symbols::Symbol.new('x');a=M::Number.new('1');prime=M::Symbols::Symbol.new('&#x27;');rows=[]
def capture(rows,id,node,options:{},display:true)
 formula=Plurimath::Math::Formula.new([node],display_style:display);model=CorpusGenerator.serialize_node(formula,'$');snapshot=Marshal.dump(formula)
 source_options=options.to_h{|k,v|[{splitOnLinebreak: :split_on_linebreak, displayStyle: :display_style, unaryFunctionSpacing: :unary_function_spacing}.fetch(k,k),v]}
 outputs=%w[asciimath latex html unicodemath mathml omml].to_h do|format|
  begin;[format,{value:Marshal.load(snapshot).public_send("to_#{format}",**source_options)}];rescue StandardError=>error;[format,{error:{class:error.class.name,message:error.message}}];end
 end
 rows<<{id:id,model:model,options:options,writers:outputs}
end
[false,true].each do|display|
 [nil,x].each_with_index do|payload,index|
  nested=M::Formula.new([x,F::Linebreak.new(payload),a])
  [F::Intent.new(nested,F::Text.new('variable')),F::Intent.new(x,nested)].each_with_index do|node,i|
   capture(rows,"intent-split-#{i}-#{display}-#{index}",node,options:{splitOnLinebreak:true},display:display)
  end
 end
end
fenced=F::Fenced.new(M::Symbols::Paren::Lround.new,[x,a],M::Symbols::Paren::Rround.new)
[false,true].each do|intent|
 [':derivative','&#x3a;derivative','&colon;derivative'].each_with_index do|metadata,i|
  node=F::Intent.new(M::Formula.new([F::Power.new(M::Symbols::Symbol.new('f'),prime),fenced]),M::Symbols::Symbol.new(metadata))
  capture(rows,"intent-literal-metadata-#{intent}-#{i}",node,options:{intent:intent})
 end
end
source=F::Intent.instance_method(:to_mathml_without_math_tag).source_location[0]
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'actual-source-intent-split-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,rows:rows})+"\n")
