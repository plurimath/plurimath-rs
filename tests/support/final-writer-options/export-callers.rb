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

[false,true].each do|display|
 [F::Limits,F::Underover].each do|klass|
  [[x,a,b],[F::None.new,a,b],[nil,nil,nil],[x,false,F::None.new],[M::Formula.new([x,a]),F::Base.new(x,a),F::Power.new(x,b)]].each_with_index do|fields,i|
   capture(rows,"caller-#{klass.name.split('::').last}-#{display}-#{i}",klass.new(*fields),display:display)
  end
 end
 parsed=M.parse('⅜',:unicode)
 parsed.value.each_with_index{|node,i|capture(rows,"parsed-unicode-fraction-#{display}-#{i}",node,display:display)}
end
[F::Text.new('1'),F::Ms.new('1'),F::None.new,M::Formula.new([]),M::Symbols::Symbol.new('1')].each_with_index do|field,i|
 [[field,M::Number.new('2')],[M::Number.new('1'),field]].each_with_index do|fields,j|
  capture(rows,"fraction-getter-#{i}-#{j}",F::Frac.new(*fields,{unicodemath_fraction:true}))
 end
end
[true,false].each do|choose|
 [0,1].each do|slot|
  fields=[a,b];fields[slot]=true
  inner=F::Frac.new(*fields,{choose:choose})
  outer=F::Frac.new(inner,b,{choose:true})
  capture(rows,"nested-choose-refusal-#{choose}-#{slot}",F::Fenced.new(M::Symbols::Paren::Lround.new,[outer],M::Symbols::Paren::Rround.new))
 end
end
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'source-display-caller-and-unicode-parser-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,rows:rows})+"\n")
