require 'json'
require 'digest'
require 'plurimath'
# Run inside the source Ruby bundle with its established corpus serializer.
require File.expand_path(ENV.fetch('PLURIMATH_CORPUS_GENERATOR'))
M=Plurimath::Math
F=M::Function
x=M::Symbols::Symbol.new('x')
y=M::Symbols::Symbol.new('y')
a=M::Number.new('1')
b=M::Number.new('2')
mini_sub=M::Number.new('1');mini_sub.mini_sub_sized=true
mini_sup=M::Number.new('2');mini_sup.mini_sup_sized=true
atoms={nil:nil,false:false,true:true,symbol:x,number:b,empty_formula:M::Formula.new([]),one_formula:M::Formula.new([x]),many_formula:M::Formula.new([x,b]),empty_array:[],one_array:[x],many_array:[x,b],nil_array:[nil],raw_string:'raw',power:F::Power.new(x,b),base:F::Base.new(x,a),prime:M::Symbols::Symbol.new('&#x27;'),mini_sub:mini_sub,mini_sup:mini_sup,sum:Plurimath::Utility.symbols_class('&#x2211;',lang: :mathml),prod:Plurimath::Utility.symbols_class('&#x220f;',lang: :mathml),integral:Plurimath::Utility.symbols_class('&#x222b;',lang: :mathml),sum_function:F::Sum.new,prod_function:F::Prod.new,int_function:F::Int.new,obrace:F::Obrace.new(x),ubrace:F::Ubrace.new(x),normal:F::FontStyle::Normal.new(x)}
formats=%w[asciimath latex html unicodemath mathml omml]
rows=[]
def capture(id,operator,formats,rows,displaystyle:true)
 formula=Plurimath::Math::Formula.new([operator],display_style:displaystyle)
 initial_model=CorpusGenerator.serialize_node(formula,'$')
 marshalled=Marshal.dump(formula)
 outputs=formats.to_h do|format|
  begin
   isolated=Marshal.load(marshalled)
   [format,{value:isolated.public_send("to_#{format}")}]
  rescue StandardError=>error
   [format,{error:{class:error.class.name,message:error.message}}]
  end
 end
 rows << {id:id,model:initial_model,options:{},writers:outputs}
end
# Constructor-created nodes, followed by their public parameter setters. These
# are actual source object states, including false and Array parameters.
3.times do|slot|
 atoms.each do|name,value|
  values=[x,a,b];values[slot]=value;operator=F::PowerBase.new(x,a,b)
  %i[parameter_one parameter_two parameter_three].zip(values){|field,v|operator.public_send("#{field}=",v)}
  capture("powerbase-slot#{slot+1}-#{name}",operator,formats,rows)
 end
end
[[x,mini_sub,mini_sup],[x,a,atoms[:prime]],[atoms[:power],a,b],[atoms[:sum],nil,b],[atoms[:sum],a,nil],[atoms[:sum],nil,nil]].each_with_index do|values,i|
 capture("powerbase-pair-#{i+1}",F::PowerBase.new(*values),formats,rows)
end
atoms.each do|name,value|
 operator=F::Sqrt.new(x);operator.parameter_one=value;capture("sqrt-#{name}",operator,['omml'],rows)
end
[false,true].each do|hide|
 2.times do|slot|
  atoms.each do|name,value|
   operator=F::Mod.new(x,y);operator.public_send("parameter_#{slot==0 ? 'one' : 'two'}=",value);operator.hide_function_name=hide
   capture("mod-#{hide ? 'hidden' : 'named'}-slot#{slot+1}-#{name}",operator,['omml'],rows)
  end
 end
end
capture('powerbase-displaystyle-false',F::PowerBase.new(x,a,b),['mathml','omml'],rows,displaystyle:false)
[atoms[:sum],atoms[:prod],atoms[:ubrace],atoms[:sum_function]].each_with_index do|base,index|
 capture("powerbase-underover-inline-#{index+1}",F::PowerBase.new(base,a,b),['omml'],rows,displaystyle:false)
end
[false,true].each do|hidden|
 operator=F::PowerBase.new(x,a,b);operator.hide_function_name=hidden
 capture("powerbase-hide-#{hidden}",operator,formats,rows)
 operator=F::Sqrt.new(x);operator.hide_function_name=hidden;operator.options={source_unused:true}
 capture("sqrt-hide-options-#{hidden}",operator,['omml'],rows)
end
metadata=atoms.filter_map do|name,value|
 next unless value.is_a?(M::Core)
 [name,{class:value.class.name.delete_prefix('Plurimath::'),tag_name:value.tag_name,omml_tag_name:value.omml_tag_name,mini_sized:value.mini_sized?}]
end.to_h
files=%w[power_base mod sqrt ternary_function].map{|name|File.join(File.dirname(F::PowerBase.instance_method(:to_latex).source_location[0]),name+'.rb')}
class_tags={}
(Plurimath::Utility.all_symbols_classes(:mathml).values.uniq+[F::Ubrace,F::Sum,F::Prod,F::Int]).each do|klass|
 object=klass.new
 tag=object.tag_name;omml=object.omml_tag_name
 next if tag=='subsup'&&omml=='subSup'
 class_tags[klass.name.delete_prefix('Plurimath::')]={tag_name:tag,omml_tag_name:omml}
end
result={profile:'actual-source-operator-writer-states-v1',ruby:RUBY_DESCRIPTION,source_heads:{plurimath:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip},source_sha256:files.to_h{|path|[path,Digest::SHA256.file(path).hexdigest]},state_construction:'Actual source constructors and public setters; model captured before writing by established CorpusGenerator.serialize_node; each format uses a Marshal deep clone of that source object to isolate writer mutation',atom_metadata:metadata,class_tags:class_tags,rows:rows}
File.write(ARGV[0],JSON.pretty_generate(result)+"\n")
