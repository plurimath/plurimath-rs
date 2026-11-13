require 'json';require 'digest';require 'plurimath'
require File.expand_path(ENV.fetch('PLURIMATH_CORPUS_GENERATOR'))
M=Plurimath::Math;F=M::Function;x=M::Symbols::Symbol.new('x');a=M::Number.new('2');rows=[]
fields={nil:nil,false:false,true:true,symbol:x,number:a,formula:M::Formula.new([x,a]),empty_formula:M::Formula.new([]),sqrt:F::Sqrt.new(x),empty_array:[],one_array:[x],nil_array:[nil],many_array:[x,nil,a],none:F::None.new,raw:'raw'}
attributes={empty:{},before:{linebreakstyle:'before'},after:{linebreakstyle:'after'},with_attrs:{linebreakstyle:'after',linebreak:'newline',id:'line-1'},false_style:{linebreakstyle:false},nil_value:{id:nil},unicode:{title:'a&b'}}
fields.each do|name,field|
 attributes.each do|style,attrs|
  node=F::Linebreak.new(field,attrs);formula=M::Formula.new([node]);model=CorpusGenerator.serialize_node(formula,'$');snapshot=Marshal.dump(formula)
  outputs=%w[asciimath latex html unicodemath mathml omml].to_h do|format|
   begin;[format,{value:Marshal.load(snapshot).public_send("to_#{format}")}];rescue StandardError=>error;[format,{error:{class:error.class.name,message:error.message}}];end
  end
  rows << {id:"linebreak-#{name}-#{style}",model:model,options:{},writers:outputs}
 end
end
files=[F::Linebreak.instance_method(:to_asciimath).source_location[0],F::UnaryFunction.instance_method(:asciimath_value).source_location[0]]
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'actual-source-linebreak-leaf-states-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,source_sha256:files.to_h{|path|[path,Digest::SHA256.file(path).hexdigest]},rows:rows})+"\n")
