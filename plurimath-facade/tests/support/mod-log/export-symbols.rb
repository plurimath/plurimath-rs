require 'json';require 'digest';require 'plurimath'
require ENV.fetch('PLURIMATH_REFERENCE_SERIALIZER')
M=Plurimath::Math;F=M::Function;x=M::Symbols::Symbol.new('x');a=M::Number.new('1');b=M::Number.new('2');rows=[];constructors=[]
def capture(rows,id,node,options:{},display:true)
 formula=Plurimath::Math::Formula.new([node],display_style:display);model=CorpusGenerator.serialize_node(formula,'$');snapshot=Marshal.dump(formula)
 source_options=options.to_h{|k,v|[{splitOnLinebreak: :split_on_linebreak, displayStyle: :display_style, unaryFunctionSpacing: :unary_function_spacing}.fetch(k,k),v]}
 outputs=%w[asciimath latex html unicodemath mathml omml].to_h do|format|
  begin;[format,{value:Marshal.load(snapshot).public_send("to_#{format}",**source_options)}];rescue StandardError=>error;[format,{error:{class:error.class.name,message:error.message}}];end
 end
 rows<<{id:id,model:model,options:options,writers:outputs}
end

['x','a','ab','0','&','@','^','(',')','α',nil].each_with_index do|value,i|
 [[:mini_sub_sized,true],[:mini_sup_sized,true],[:both,true],[:slashed,true],[:mini_sub_sized,'false']].each_with_index do|(field,state),j|
  symbol=M::Symbols::Symbol.new(value)
  if field==:both;symbol.mini_sub_sized=true;symbol.mini_sup_sized=true;else;symbol.public_send("#{field}=",state);end
  capture(rows,"symbol-shared-#{i}-#{j}",symbol)
 end
end
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'actual-source-shared-symbol-unicode-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,rows:rows})+"\n")
