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

helpers=[]
['′','″','‴','⁗',"'",'&#x27;','&#x2032;','&#x2033;','&#x2034;','&#x2057;','prefix&#x2032;suffix',nil,false,[],['&#x2032;']].each_with_index do|value,i|
 symbol=M::Symbols::Symbol.new(value)
 begin;prime=symbol.prime_unicode?(symbol);rescue StandardError=>error;prime={error:{class:error.class.name,message:error.message}};end
 helpers<<{id:"prime#{i}",model:CorpusGenerator.serialize_node(symbol,'$'),prime:prime}
 power=F::Power.new(x,symbol)
 capture(rows,"power-prime#{i}",power)
 capture(rows,"limits-prime#{i}",F::Limits.new(power,a,b))
end
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'source-lexical-prime-comparison-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,declarations:{primes:Plurimath::Utility.primes_constants},helpers:helpers,rows:rows})+"\n")
