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
names=JSON.parse(File.read(ENV.fetch("SYMBOL_CLASS_NAMES")))
receiver=M::Formula.new([]);entries={};errors=[];prime_checks=[]
names.each do |name|
 begin
  klass=name.split("::").reduce(M::Symbols){|scope,part|scope.const_get(part,false)}
  node=klass.new
  value=receiver.send(:unicodemath_field_value,node)
  raise "unexpected source value kind" unless value.nil? || value.is_a?(String)
  entries["Math::Symbols::#{name}"]=value
  model=CorpusGenerator.serialize_node(node,"$")
  begin
   prime_checks<<{model:model,value:receiver.send(:prime_unicode?,node)}
  rescue StandardError=>error
   prime_checks<<{model:model,error:{class:error.class.name,message:error.message}}
  end
 rescue StandardError=>error
  errors<<{name:name,class:error.class.name,message:error.message}
 end
end
File.write(ARGV.fetch(0),JSON.pretty_generate({source_head:HEAD,runtime:RUBY_DESCRIPTION,entries:entries,errors:errors,prime_checks:prime_checks}))
puts JSON.generate({entries:entries.size,errors:errors.size,prime:entries["Math::Symbols::Prime"]})
