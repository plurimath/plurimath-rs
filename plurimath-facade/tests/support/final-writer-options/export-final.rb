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
states={nil:nil,false:false,true:true,symbol:x,number:a,none:F::None.new,formula:M::Formula.new([x,a]),empty_formula:M::Formula.new([]),fenced:fenced,array:[x,a],nil_array:[nil],false_array:[false],raw:'raw',power:F::Power.new(x,a),base:F::Base.new(x,a)}
[F::Limits,F::Underover].each do|klass|
 states.each do|name,value|
  3.times do|slot|
   fields=[x,a,b];fields[slot]=value
   capture(rows,"#{klass.name.split('::').last}-slot#{slot}-#{name}",klass.new(*fields))
  end
 end
 values=[nil,false,x,a,F::None.new,M::Formula.new([x,a])]
 values.product(values,values).each_with_index do|fields,i|
  capture(rows,"#{klass.name.split('::').last}-matrix#{i}",klass.new(*fields))
 end
end
states.each do|name,under|
 states.each do|other,base|
  capture(rows,"underset-#{name}-#{other}",F::Underset.new(under,base))
 end
end
under_options=[nil,false,true,{},[],{accentunder:true},{accentunder:false},{accentunder:nil},{accentunder:'false'},{accentunder:'true'},{accent:true},{id:'u',accentunder:true},{accentunder:0},[['accentunder','true']],{width:'0'}]
under_options.each_with_index do|opt,i|
 [false,true].each do|display|
  [nil,x,F::None.new,fenced,[x,a]].each_with_index do|base,j|
   node=F::Underset.new(x,base);node.options=opt
   capture(rows,"underset-options#{i}-field#{j}-display#{display}",node,display:display)
  end
 end
end
frac_options=[nil,false,true,{},[],0,'raw',{choose:true},{choose:false},{choose:nil},{linethickness:'0'},{linethickness:'0.0'},{linethickness:'1'},{linethickness:0},{linethickness:nil},{linethickness:false},{bevelled:'true'},{bevelled:true},{bevelled:'false'},{displaystyle:true},{displaystyle:false},{displaystyle:nil},{ldiv:true},{ldiv:false},{ldiv:nil},{unicodemath_fraction:true},{unicodemath_fraction:false},{unicodemath_fraction:nil},{choose:true,bevelled:'true'},{choose:true,linethickness:'0'},{bevelled:'true',linethickness:'0'},{ldiv:true,displaystyle:false},{id:'n',title:'a&b'},{unknown:'x'},[['linethickness','0']],[['choose',true]]]
frac_options.each_with_index do|opt,i|
 [[x,b],[nil,nil],[M::Formula.new([x,a]),fenced]].each_with_index do|fields,j|
  begin
   capture(rows,"frac-constructor#{i}-fields#{j}",F::Frac.new(*fields,opt))
  rescue StandardError=>error
   constructors<<{id:"frac-constructor#{i}-fields#{j}",error:{class:error.class.name,message:error.message}}
  end
  node=F::Frac.new(*fields);node.options=opt
  capture(rows,"frac-setter#{i}-fields#{j}",node)
 end
 [false,true].each do|hide|
  node=F::Frac.new(x,b);node.options=opt;node.hide_function_name=hide
  capture(rows,"frac-hide#{hide}-options#{i}",node)
 end
 [false,true].each do|display|
  node=F::Frac.new(x,b);node.options=opt
  capture(rows,"frac-fenced-options#{i}-display#{display}",F::Fenced.new(M::Symbols::Paren::Lround.new,[node],M::Symbols::Paren::Rround.new),display:display)
 end
end
[0,1,-1,2,3,4,5,6,7,8,9,10,'1','2','3','1x','  +1_0tail','0d1','0D3','0x3','0b11','1__2','1_2','+','-','',nil,false,1.8,-1.8,'1.8','999999999999999999999999999999999999999','00000000000000000000000000000001'].each_with_index do|n,i|
 [[n,'2'],['1',n]].each_with_index do|fields,j|
  node=F::Frac.new(*fields.map{|value|M::Number.new(value)},{unicodemath_fraction:true})
  capture(rows,"frac-unicode-cast#{i}-slot#{j}",node)
 end
end
accents=['&#x23df;','&#x23de;','&#x332;','&#x305;','&#x20d7;','x','_',"\u0332"]
accents.each_with_index do|value,i|
 [x,F::Obrace.new(x),F::Ubrace.new(x),nil].each_with_index do|base,j|
  capture(rows,"underset-accent#{i}-base#{j}",F::Underset.new(M::Symbols::Symbol.new(value),base))
 end
end
[F::Limits,F::Underover,F::Underset,F::Frac].each do|klass|
 [false,true].each do|display|
  [nil,x].each_with_index do|payload,i|
   nested=M::Formula.new([x,F::Linebreak.new(payload),a])
   count=klass<=F::TernaryFunction ? 3 : 2
   count.times do|slot|
    fields=[x,a,b].take(count);fields[slot]=nested
    capture(rows,"split-#{klass.name.split('::').last}-slot#{slot}-payload#{i}-display#{display}",klass.new(*fields),options:{splitOnLinebreak:true},display:display)
   end
  end
 end
end
[['\\underset{a}{x}',:latex],['\\overset{b}{\\underset{a}{x}}',:latex],['\\frac{1}{2}',:latex],['\\binom{3}{2}',:latex],['(1)/(2)',:asciimath],['⅜',:unicodemath]].each_with_index do|(input,format),i|
 begin
  parsed=M.parse(input,format)
  parsed.value.each_with_index{|node,j|capture(rows,"parsed-#{i}-#{j}",node)}
 rescue StandardError=>error
  constructors<<{id:"parsed-#{i}",input:input,format:format,error:{class:error.class.name,message:error.message}}
 end
end
symbol_hex=ObjectSpace.each_object(Class).select{|klass|klass<M::Symbols::Symbol&&klass.name}.sort_by(&:name).to_h do|klass|
 begin;[klass.name.sub('Plurimath::',''),klass.new.input(:unicodemath)&.flatten&.find{|input|input.match?(/&#x.+;/)}];rescue StandardError;[klass.name.sub('Plurimath::',''),nil];end
end
metadata={unicode_fractions:Plurimath::UnicodeMath::Constants::UNICODE_FRACTIONS,horizontal_brackets:Plurimath::UnicodeMath::Constants::HORIZONTAL_BRACKETS,diacritic_belows:Plurimath::UnicodeMath::Constants::DIACRITIC_BELOWS,accent_symbols:Plurimath::UnicodeMath::Constants::ACCENT_SYMBOLS,symbol_hex:symbol_hex}
files=[F::Limits,F::Underset,F::Underover,F::Frac,F::TernaryFunction,F::BinaryFunction].map{|klass|klass.instance_method(:to_mathml_without_math_tag).source_location[0]}
File.write(ARGV.fetch(0),JSON.pretty_generate({profile:'canonical-source-final-writer-options-v1',ruby:RUBY_DESCRIPTION,source_head:`git -C #{Gem.loaded_specs.fetch('plurimath').full_gem_path} rev-parse HEAD`.strip,source_sha256:files.uniq.to_h{|path|[path,Digest::SHA256.file(path).hexdigest]},declarations:metadata,constructor_refusals:constructors,rows:rows})+"\n")
