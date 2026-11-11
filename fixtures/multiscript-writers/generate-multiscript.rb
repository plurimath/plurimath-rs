require 'json'
require 'digest'
require 'plurimath'
require File.expand_path(ENV.fetch('PLURIMATH_CORPUS_GENERATOR'))
M=Plurimath::Math;F=M::Function
x=M::Symbols::Symbol.new('x');y=M::Symbols::Symbol.new('y');a=M::Number.new('1');b=M::Number.new('2')
mini=M::Number.new('1');mini.mini_sub_sized=true
prime=M::Symbols::Symbol.new('&#x27;')
base=F::PowerBase.new(x,a,b)
rows=[]
def capture(rows,id,node,displaystyle:true,options:{})
 formula=Plurimath::Math::Formula.new([node],display_style:displaystyle)
 initial=CorpusGenerator.serialize_node(formula,'$');state=Marshal.dump(formula)
 outputs=%w[asciimath latex html unicodemath mathml omml].to_h do|format|
  object=Marshal.load(state)
  begin
   value=object.public_send("to_#{format}",**options)
   [format,{value:value,after:CorpusGenerator.serialize_node(object,'$')}]
  rescue StandardError=>error
   [format,{error:{class:error.class.name,message:error.message}}]
  end
 end
 rows << {id:id,model:initial,options:options,writers:outputs}
end
# Constructor states and public setters are intentionally kept distinct.
[[nil,nil,nil],[base,[],[]],[base,[a],[b]],[x,[a],[b]]].each_with_index do|args,i|
 capture(rows,"constructor-#{i}",F::Multiscript.new(*args))
end
states={nil:nil,false:false,true:true,empty:[],one:[a],two:[a,b],none:[F::None.new],nil_item:[nil],false_item:[false],nested:[[a],[b]],mini:[mini],prime:[prime],power:[F::Power.new(x,b)],direct_node:a,raw:'raw',empty_string:'',formula:M::Formula.new([a,b]),empty_formula:M::Formula.new([])}
states.each do|subname,sub|
 states.each do|supname,sup|
  node=F::Multiscript.new;node.parameter_one=base;node.parameter_two=sub;node.parameter_three=sup
  capture(rows,"lists-#{subname}-#{supname}",node)
 end
end
bases={nil:nil,false:false,true:true,symbol:x,number:a,power: F::Power.new(x,b),powerbase:base,formula:M::Formula.new([x]),empty_formula:M::Formula.new([]),array:[x],empty_array:[],raw:'raw',none:F::None.new}
bases.each do|name,value|
 node=F::Multiscript.new;node.parameter_one=value;node.parameter_two=[a];node.parameter_three=[b]
 capture(rows,"base-#{name}",node)
end
[false,true].each do|display|
 node=F::Multiscript.new(base,[a],[b]);capture(rows,"displaystyle-#{display}",node,displaystyle:display)
end
[false,true].each do|hide|
 node=F::Multiscript.new(base,[a],[b]);node.hide_function_name=hide;capture(rows,"hide-#{hide}",node)
end
[[[],[a,b]],[[a],[a,b]],[[a,b],[a]],[[nil,a],[b]],[[a],[nil,b]],[[nil],[b]]].each_with_index do|args,i|
 capture(rows,"uneven-#{i}",F::Multiscript.new(base,*args))
end
none_with_field=F::None.new(a)
[none_with_field,F::None.new].each_with_index do|placeholder,i|
 capture(rows,"placeholder-state-#{i}",F::Multiscript.new(base,[placeholder],[placeholder]))
end
sources=[F::None.instance_method(:to_asciimath).source_location[0],F::Multiscript.instance_method(:to_asciimath).source_location[0],F::TernaryFunction.instance_method(:to_html).source_location[0],M::Core.instance_method(:omml_parameter).source_location[0],Plurimath::Utility.method(:filter_values).source_location[0],F::PowerBase.instance_method(:mmultiscript).source_location[0]].uniq
result={profile:'actual-source-multiscript-writer-states-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,source_sha256:sources.to_h{|path|[path,Digest::SHA256.file(path).hexdigest]},construction:'Source constructors and public setters, public CorpusGenerator model serializer, isolated Marshal clone per format; after-state recorded separately',rows:rows}
File.write(ARGV.fetch(0),JSON.pretty_generate(result)+"\n")
