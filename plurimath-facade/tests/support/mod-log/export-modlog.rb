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

fenced=F::Fenced.new(M::Symbols::Paren::Lround.new,[x,a],M::Symbols::Paren::Rround.new)
sub=M::Number.new('12');sub.mini_sub_sized=true
sup=M::Number.new('12');sup.mini_sup_sized=true
subsym=M::Symbols::Symbol.new('x');subsym.mini_sub_sized=true
supsym=M::Symbols::Symbol.new('x');supsym.mini_sup_sized=true
states={nil:nil,false:false,true:true,symbol:x,number:a,text:F::Text.new('t'),none:F::None.new,formula:M::Formula.new([x,a]),empty_formula:M::Formula.new([]),fenced:fenced,array:[x,a],nil_array:[nil],false_array:[false],raw:'raw',base:F::Base.new(x,a),power:F::Power.new(x,a),sub:sub,sup:sup,subsym:subsym,supsym:supsym,prime:M::Symbols::Symbol.new('&#x2032;'),literal_prime:M::Symbols::Symbol.new('′')}
begin;states[:typed_prime]=M::Symbols.const_get('Prime').new;rescue StandardError=>error;constructors<<{id:'typed-prime',error:{class:error.class.name,message:error.message}};end
[F::Mod,F::Log].each do|klass|
 states.each do|name,first|
  states.each do|label,second|
   capture(rows,"#{klass.name.split('::').last}-#{name}-#{label}",klass.new(first,second))
  end
 end
 [false,true,nil,'false'].each_with_index do|hide,i|
  [false,true].each do|display|
   [[nil,nil],[x,a],[false,false],[F::None.new,F::None.new],[fenced,sup]].each_with_index do|fields,j|
    node=klass.new(*fields);node.hide_function_name=hide
    capture(rows,"#{klass.name.split('::').last}-hide#{i}-fields#{j}-display#{display}",node,display:display)
   end
  end
 end
 [nil,{},false,{unknown:'v'},{mask:3}].each_with_index do|options,i|
  begin;node=klass.new(x,a);node.options=options;capture(rows,"#{klass.name.split('::').last}-options#{i}",node);rescue StandardError=>error;constructors<<{id:"#{klass.name.split('::').last}-options#{i}",error:{class:error.class.name,message:error.message}};end
 end
 [false,true].each do|display|
  [nil,x].each_with_index do|payload,i|
   2.times do|slot|
    fields=[x,a];fields[slot]=M::Formula.new([x,F::Linebreak.new(payload),a]);node=klass.new(*fields)
    capture(rows,"#{klass.name.split('::').last}-split-slot#{slot}-payload#{i}-display#{display}",node,options:{splitOnLinebreak:true},display:display)
   end
  end
 end
end
[['7 mod 3',:asciimath],['log_2^3 x',:asciimath],['\\log_2^3 x',:latex],['7\\mod 3',:latex],['log',:asciimath]].each_with_index do|(input,format),i|
 begin
  parsed=M.parse(input,format);parsed.value.each_with_index{|node,j|capture(rows,"parsed-#{i}-#{j}",node)}
 rescue StandardError=>error;constructors<<{id:"parsed-#{i}",error:{class:error.class.name,message:error.message}};end
end
files=[F::Mod,F::Log].map{|klass|klass.instance_method(:to_mathml_without_math_tag).source_location[0]}
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'canonical-source-mod-log-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,source_sha256:files.to_h{|p|[p,Digest::SHA256.file(p).hexdigest]},constructor_refusals:constructors,rows:rows})+"\n")
