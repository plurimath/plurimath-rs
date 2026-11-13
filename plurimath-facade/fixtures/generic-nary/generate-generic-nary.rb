require 'json';require 'digest';require 'plurimath'
require File.expand_path(ENV.fetch('PLURIMATH_CORPUS_GENERATOR'))
M=Plurimath::Math;F=M::Function;x=M::Symbols::Symbol.new('x');a=M::Number.new('1');b=M::Number.new('2');int=Plurimath::Utility.symbols_class('&#x222b;',lang: :mathml);sum=Plurimath::Utility.symbols_class('&#x2211;',lang: :mathml)
mini=M::Number.new('1');mini.mini_sub_sized=true;mini_sup=M::Number.new('2');mini_sup.mini_sup_sized=true
rows=[]
def capture(rows,id,node,display:true,options:{})
 object=Plurimath::Math::Formula.new([node],display_style:display);model=CorpusGenerator.serialize_node(object,'$');snapshot=Marshal.dump(object)
 outputs=%w[asciimath latex html unicodemath mathml omml].to_h do|fmt|
  source_options=options.to_h{|k,v|[{splitOnLinebreak: :split_on_linebreak, displayStyle: :display_style, unaryFunctionSpacing: :unary_function_spacing}.fetch(k,k),v]}
  begin;[fmt,{value:Marshal.load(snapshot).public_send("to_#{fmt}",**source_options)}];rescue StandardError=>error;[fmt,{error:{class:error.class.name,message:error.message}}];end
 end
 rows<<{id:id,model:model,options:options,writers:outputs}
end
states={nil:nil,false:false,true:true,symbol:x,number:a,int:int,sum:sum,none:F::None.new,empty_formula:M::Formula.new([]),formula:M::Formula.new([x,b]),fenced:F::Fenced.new(M::Formula.new([x,b])),power:F::Power.new(x,b),base:F::Base.new(x,a),prime:M::Symbols::Symbol.new('&#x27;'),mini:mini,mini_sup:mini_sup,empty_array:[],array:[x,a],nil_array:[nil],raw:'raw'}
4.times do|slot|
 states.each do|name,value|
  values=[int,a,b,x];values[slot]=value;node=F::Nary.new; %i[parameter_one parameter_two parameter_three parameter_four].zip(values){|field,v|node.public_send("#{field}=",v)}
  capture(rows,"slot#{slot+1}-#{name}",node)
 end
end
[{}, {type:'undOvr'}, {type:'subSup'}, {type:'bogus'}, {type:false}, {type:nil}, {type:0}, {type:[]}, {type:{}},nil,false,'raw'].each_with_index do|options,i|
 node=F::Nary.new(sum,a,b,x);node.options=options;capture(rows,"options-#{i}",node)
end
[nil,false,true,*(0..32),-1,1.9,-3.9,'','  +31_0x','nonsense',[],{}].each_with_index do|mask,i|
 [[nil,nil],[a,nil],[nil,b],[a,b],[M::Formula.new([x]),b],[M::Formula.new([x,b]),b],[M::Formula.new([x]),nil],[M::Formula.new([]),nil]].each_with_index do|(sub,sup),j|
  capture(rows,"mask-#{i}-limits-#{j}",F::Nary.new(sum,sub,sup,x,{mask:mask}))
 end
end
[[nil,nil,nil,nil],[sum,nil,nil,nil],[sum,false,false,false],[sum,a,b,nil],[int,a,b,F::Fenced.new(x)],[sum,a,M::Symbols::Symbol.new('&#x27;'),x]].each_with_index do|args,i|capture(rows,"combination-#{i}",F::Nary.new(*args));end
[false,true].each do|display|
 [false,true].each do|intent|capture(rows,"render-context-#{display}-#{intent}",F::Nary.new(sum,a,b,x),display:display,options:{intent:intent});end
end
4.times do|slot|
 [[],[a],[a,b]].each_with_index do|prefix,j|
  [nil,x].each_with_index do|payload,k|
   field=M::Formula.new([*prefix,F::Linebreak.new(payload),b]);values=[sum,a,b,x];values[slot]=field
   [false,true].each do|display|
    capture(rows,"split-slot#{slot}-prefix#{j}-payload#{k}-#{display}",F::Nary.new(*values,{type:'undOvr',mask:1}),display:display,options:{splitOnLinebreak:true})
   end
  end
 end
end
symbol_metadata=M::Symbols.nary_classes.to_h do|klass|
 object=klass.new
 [klass.name.delete_prefix('Plurimath::'),{intent_name:object.nary_intent_name,nary_attribute:object.nary_attr_value(options:{})}]
end
M::Symbols.nary_classes.each_with_index do|klass,i|
 [nil,a,M::Formula.new([a,b]),M::Formula.new([x,b])].each_with_index do|lower,j|
  capture(rows,"intent-symbol-#{i}-#{j}",F::Nary.new(klass.new,lower,b,x),options:{intent:true})
 end
end
[[nil,nil,nil,nil],[sum,nil,nil,nil],[sum,nil,nil,x],[nil,a,b,x],[sum,a,b,nil],[sum,M::Formula.new([a,b]),M::Formula.new([x,b]),M::Formula.new([x,b])]].each_with_index do|args,i|
 capture(rows,"intent-shape-#{i}",F::Nary.new(*args),options:{intent:true})
end
files=[F::Nary.instance_method(:to_asciimath).source_location[0],M::Core.instance_method(:masked_tag).source_location[0],M::Symbols::Symbol.instance_method(:nary_attr_value).source_location[0]]
result={profile:'actual-source-generic-nary-states-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,symbol_declarations:symbol_metadata,source_sha256:files.to_h{|path|[path,Digest::SHA256.file(path).hexdigest]},rows:rows};File.write(ARGV.fetch(0),JSON.pretty_generate(result)+"\n")
