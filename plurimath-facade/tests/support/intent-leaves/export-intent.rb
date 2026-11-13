require 'json';require 'digest';require 'plurimath'
require ENV.fetch('PLURIMATH_REFERENCE_SERIALIZER')
M=Plurimath::Math;F=M::Function;x=M::Symbols::Symbol.new('x');a=M::Number.new('1');prime=M::Symbols::Symbol.new('&#x27;');rows=[]
def capture(rows,id,node,options:{},display:true)
 formula=Plurimath::Math::Formula.new([node],display_style:display);model=CorpusGenerator.serialize_node(formula,'$');snapshot=Marshal.dump(formula)
 outputs=%w[asciimath latex html unicodemath mathml omml].to_h do|format|
  begin;[format,{value:Marshal.load(snapshot).public_send("to_#{format}",**options)}];rescue StandardError=>error;[format,{error:{class:error.class.name,message:error.message}}];end
 end
 rows<<{id:id,model:model,options:options,writers:outputs}
end
fenced=F::Fenced.new(M::Symbols::Paren::Lround.new,[x,a],M::Symbols::Paren::Rround.new)
fields={nil:nil,false:false,true:true,symbol:x,number:a,text:F::Text.new('hi'),none:F::None.new,empty_formula:M::Formula.new([]),formula:M::Formula.new([x,a]),fenced:fenced,array:[x,a],empty_array:[],raw:'raw'}
metadata={nil:nil,false:false,true:true,symbol:M::Symbols::Symbol.new('variable'),entity:M::Symbols::Symbol.new('&#x3b1;'),derivative:M::Symbols::Symbol.new(':derivative'),number:a,text:F::Text.new('variable'),none:F::None.new,formula:M::Formula.new([x]),raw:'literal'}
fields.each do|name,field|
 metadata.each do|label,meta|
  capture(rows,"intent-field-#{name}-meta-#{label}",F::Intent.new(field,meta))
 end
end
[false,true].each do|global|
 [false,true].each do|display|
  [M::Formula.new([F::Power.new(M::Symbols::Symbol.new('f'),prime),fenced]),M::Formula.new([F::Power.new(M::Symbols::Symbol.new('f'),prime)]),M::Formula.new([F::Power.new(M::Symbols::Symbol.new('f'),prime),fenced,x]),x].each_with_index do|field,i|
   capture(rows,"intent-derivative-shape#{i}-global#{global}-display#{display}",F::Intent.new(field,M::Symbols::Symbol.new(':derivative')),options:{intent:global},display:display)
  end
 end
end
source=F::Intent.instance_method(:to_mathml_without_math_tag).source_location[0]
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'actual-source-explicit-intent-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,source_sha256:{source=>Digest::SHA256.file(source).hexdigest},rows:rows})+"\n")
