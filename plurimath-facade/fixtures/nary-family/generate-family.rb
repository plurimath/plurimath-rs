require 'json';require 'digest';require 'plurimath'
require File.expand_path(ENV.fetch('PLURIMATH_CORPUS_GENERATOR'))
M=Plurimath::Math;F=M::Function;x=M::Symbols::Symbol.new('x');y=M::Symbols::Symbol.new('y');a=M::Number.new('1');b=M::Number.new('2');rows=[]
def capture(rows,id,node,options:{},display:true)
 object=Plurimath::Math::Formula.new([node],display_style:display);model=CorpusGenerator.serialize_node(object,'$');snapshot=Marshal.dump(object)
 source_options=options.to_h{|k,v|[{splitOnLinebreak: :split_on_linebreak, displayStyle: :display_style, unaryFunctionSpacing: :unary_function_spacing}.fetch(k,k),v]}
 outputs=%w[asciimath latex html unicodemath mathml omml].to_h do|format|
  begin;[format,{value:Marshal.load(snapshot).public_send("to_#{format}",**source_options)}];rescue StandardError=>error;[format,{error:{class:error.class.name,message:error.message}}];end
 end
 rows<<{id:id,model:model,options:options,writers:outputs}
end
fenced=F::Fenced.new(M::Symbols::Paren::Lround.new,[x,b],M::Symbols::Paren::Rround.new)
states={nil:nil,false:false,true:true,number:a,symbol:x,empty_formula:M::Formula.new([]),number_formula:M::Formula.new([a,b]),mixed_formula:M::Formula.new([x,b]),fenced:fenced,none:F::None.new,empty_array:[],array:[a,b],raw:'raw'}
families=[F::Sum,F::Prod,F::Int,F::Oint]
families.each do|klass|
 3.times do|slot|
  states.each do|name,value|
   values=[a,b,x];values[slot]=value;node=klass.new(*values)
   capture(rows,"intent-#{klass.name.split('::').last}-slot#{slot}-#{name}",node,options:{intent:true})
  end
 end
 [[nil,nil,nil],[a,nil,nil],[nil,b,nil],[a,b,nil],[nil,nil,x],[false,false,false]].each_with_index do|values,i|
  capture(rows,"intent-#{klass.name.split('::').last}-early-#{i}",klass.new(*values),options:{intent:true})
 end
 [false,true].each do|hide|
  node=klass.new(a,b,x);node.hide_function_name=hide;capture(rows,"intent-#{klass.name.split('::').last}-hide-#{hide}",node,options:{intent:true})
 end
 3.times do|slot|
  [[],[a],[a,b]].each_with_index do|prefix,j|
   [nil,x].each_with_index do|payload,k|
    [false,true].each do|display|
     field=M::Formula.new([*prefix,F::Linebreak.new(payload),b]);values=[a,b,x];values[slot]=field;node=klass.new(*values)
     capture(rows,"break-#{klass.name.split('::').last}-slot#{slot}-prefix#{j}-payload#{k}-#{display}",node,options:{splitOnLinebreak:true},display:display)
    end
   end
  end
 end
 [0,1,3,12,31,'0d17'].each do|mask|
  node=klass.new(M::Formula.new([a,F::Linebreak.new,b]),b,x);node.options={mask:mask}
  capture(rows,"break-#{klass.name.split('::').last}-mask-#{mask}",node,options:{splitOnLinebreak:true,intent:true})
 end
end
sources=families.map{|klass|klass.instance_method(:to_mathml_without_math_tag).source_location[0]}+[M::Core.instance_method(:intentify).source_location[0],Plurimath::Utility::IntentEncoding.method(:naryand_intent).source_location[0]]
result={profile:'actual-source-nary-family-intent-break-states-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,source_sha256:sources.uniq.to_h{|path|[path,Digest::SHA256.file(path).hexdigest]},intent_declarations:families.to_h{|klass|[klass.name.delete_prefix('Plurimath::'),klass.new.intent_names[:name]]},rows:rows};File.write(ARGV.fetch(0),JSON.pretty_generate(result)+"\n")
