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
fenced=F::Fenced.new(M::Symbols::Paren::Lround.new,[x,a],M::Symbols::Paren::Rround.new)
fields={nil:nil,false:false,true:true,symbol:x,number:a,text:F::Text.new('hi'),none:F::None.new,empty_formula:M::Formula.new([]),formula:M::Formula.new([x,a]),fenced:fenced,array:[x,a],empty_array:[],raw:'raw',integer:3,nil_array:[nil],false_array:[false],nested_array:[[x],[a]]}
fields.each do |name,field|
 capture(rows,"sup-default-#{name}",F::Sup.new(field))
 [false,true,nil].each do |hidden|
  [false,true].each do |intent|
   node=F::Sup.new(field);node.hide_function_name=hidden
   capture(rows,"sup-#{name}-hide#{hidden.inspect}-intent#{intent}",node,options:{intent:intent})
  end
 end
 fields.each do |label,other|
  capture(rows,"root-#{name}-#{label}",F::Root.new(field,other))
 end
 node=F::Ms.new(field);capture(rows,"ms-constructor-#{name}",node)
 node=F::Ms.new;node.value=field;capture(rows,"ms-value-setter-#{name}",node)
end
[false,true].each do|display|
 [false,true].each do|intent|
  capture(rows,"root-context-#{display}-#{intent}",F::Root.new(a,M::Formula.new([x,a])),options:{intent:intent},display:display)
 end
end
[false,true].each do |display|
 [nil,x].each_with_index do|payload,index|
  nested=M::Formula.new([x,F::Linebreak.new(payload),a])
  [F::Sup.new(nested),F::Root.new(nested,x),F::Root.new(a,nested),F::Ms.new(nested)].each_with_index do|node,i|
   capture(rows,"leaf-split-#{i}-#{display}-#{index}",node,options:{splitOnLinebreak:true},display:display)
  end
 end
end
source_files=[F::Sup,F::Root,F::Ms].to_h{|klass|file=klass.instance_methods(false).map{|m|klass.instance_method(m).source_location&.first}.compact.first;[file,Digest::SHA256.file(file).hexdigest]}
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'actual-source-leaf-writers-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,source_sha256:source_files,rows:rows})+"\n")
