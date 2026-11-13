require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M=Plurimath::Math
SOURCE=ENV.fetch("PLURIMATH_SOURCE_ROOT")
HEAD=IO.popen(["git","-C",SOURCE,"rev-parse","HEAD"], &:read).strip
raise "expected clean pinned source" unless HEAD=="0a1df68a5903d43da13b1e1ef85b6c0523feb1b4" && IO.popen(["git","-C",SOURCE,"status","--porcelain"], &:read).empty?
module SourceValues
 def self.encode(value, active={})
  tracked=value.is_a?(Array)||value.is_a?(Hash)||value.is_a?(Plurimath::Math::Core)
  raise "cyclic source value" if tracked && active[value.object_id]
  active[value.object_id]=true if tracked
  begin
   case value
   when NilClass then ["nil"]
   when TrueClass,FalseClass then ["bool",value]
   when Integer then ["integer",value.to_s]
   when Float then ["float",[value].pack("G").unpack1("H*")]
   when String
    raise "unsupported binary/invalid source String encoding" unless [Encoding::UTF_8,Encoding::US_ASCII].include?(value.encoding) && value.valid_encoding?
    ["string",value]
   when Symbol then ["symbol",value.to_s]
   when Array then ["array",value.map{|v|encode(v,active)}]
   when Hash then ["hash",value.map{|k,v|[encode(k,active),encode(v,active)]}]
   when Plurimath::Math::Core
    ["node",value.class.name.delete_prefix("Plurimath::"),value.variables.map{|name|[name.to_s.delete_prefix("@"),encode(value.get(name),active)]}]
   else raise "unsupported source class #{value.class}"
   end
  ensure
   active.delete(value.object_id) if tracked
  end
 end
 def self.decode(value)
  tag,*args=value
  case tag
  when "nil" then nil
  when "bool","string" then args[0]
  when "integer" then Integer(args[0],10)
  when "float" then [args[0]].pack("H*").unpack1("G")
  when "symbol" then args[0].to_sym
  when "array" then args[0].map{|v|decode(v)}
  when "hash" then args[0].to_h{|k,v|[decode(k),decode(v)]}
  when "node"
   klass=args[0].split("::").reduce(Plurimath){|scope,name|scope.const_get(name,false)}
   raise "outside Core hierarchy" unless klass<=Plurimath::Math::Core
   node=klass.allocate
   args[1].each{|name,v|node.instance_variable_set("@#{name}",decode(v))}
   node
  else raise "unknown source tag"
  end
 end
end
def symbol;M::Symbols::Symbol.new("x");end
def number;M::Number.new("1");end
def nary(options);M::Function::Nary.new(Plurimath::Utility.symbols_class("&#x2211;",lang: :mathml),number,number,symbol,options);end
pairs=[
 ["mpadded-width-key",M::Function::Mpadded.new(symbol,{width:"0"}),M::Function::Mpadded.new(symbol,{"width"=>"0"})],
 ["mpadded-mask-key",M::Function::Mpadded.new(symbol,{mask:3}),M::Function::Mpadded.new(symbol,{"mask"=>3})],
 ["mpadded-flag-key",M::Function::Mpadded.new(symbol,{mpadded:true}),M::Function::Mpadded.new(symbol,{"mpadded"=>true})],
 ["mpadded-nested-key",M::Function::Mpadded.new(symbol,{mpadded:{width:"0"},phantom:false}),M::Function::Mpadded.new(symbol,{mpadded:{"width"=>"0"},phantom:false})],
 ["mpadded-array-key",M::Function::Mpadded.new(symbol),M::Function::Mpadded.new(symbol)],
 ["nary-mask-key",nary({mask:17}),nary({"mask"=>17})],
 ["nary-mask-value",nary({mask:"0d17"}),nary({mask: :"0d17"})],
 ["number-array-kind",M::Number.new([:x]),M::Number.new(["x"])],
 ["number-hash-key-kind",M::Number.new({x:1}),M::Number.new({"x"=>1})]
]
pairs[4][1].options=[[:width,"0"]];pairs[4][2].options=[["width","0"]]
producer={implementation:"plurimath-ruby",source_commit:HEAD,runtime:RUBY_DESCRIPTION,codec_sha256:Digest::SHA256.file(__FILE__).hexdigest,identity:"typed-value-tree; object identity is not represented"}
rows=[]
pairs.each do |id,left,right|
 [left,right].each_with_index do |node,index|
  root=M::Formula.new([node]);wire=SourceValues.encode(root);restored=SourceValues.decode(wire)
  raise "typed source roundtrip differs" unless SourceValues.encode(restored)==wire
  formats=id.start_with?("number-") ? %w[asciimath latex html mathml omml unicodemath] : %w[mathml omml unicodemath]
  writers=formats.to_h do |format|
   call=lambda do |object|
    begin;{"output"=>Marshal.load(Marshal.dump(object)).public_send("to_#{format}")};rescue StandardError=>error;{"error"=>{"class"=>error.class.name,"message"=>error.message}};end
   end
   expected=call.call(root);actual=call.call(restored)
   raise "restored source writer differs" unless actual==expected || (actual["error"]&&expected["error"]&&actual["error"]["class"]==expected["error"]["class"])
   [format,expected]
  end
  rows<<{id:"#{id}/#{index}",collision:id,canonical:CorpusGenerator.serialize_node(root,"$"),document:{schema:"plurimath.ruby-values/1",producer:producer,root:wire},writers:writers}
 end
end
values=[nil,false,true,2**100,-0.0,1.0,Float::INFINITY,Float::NAN,:x,"x",[:x,"x"],{x:1,"x"=>2}]
scalar_rows=values.map{|value|wire=SourceValues.encode(value);raise "scalar type/bits roundtrip" unless SourceValues.encode(SourceValues.decode(wire))==wire;wire}
cycle=[];cycle<<cycle;cycle_refused=begin;SourceValues.encode(cycle);false;rescue StandardError;true;end
raise "cycle admitted" unless cycle_refused
binary_refused=begin;SourceValues.encode("x".b);false;rescue StandardError;true;end
raise "binary String admitted" unless binary_refused
File.binwrite(ARGV.fetch(0),JSON.generate({profile:"typed-source-collision-candidate/1",producer:producer,rows:rows,scalar_roundtrips:scalar_rows,cycle_refused:cycle_refused,binary_refused:binary_refused})+"\n")
puts JSON.generate({variants:rows.length,writer_observations:rows.sum{|r|r[:writers].length},scalar_roundtrips:scalar_rows.length,cycle_refused:cycle_refused})
