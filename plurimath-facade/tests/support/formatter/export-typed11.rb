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
methods={"asciimath"=>:to_asciimath,"latex"=>:to_latex,"html"=>:to_html,"unicodemath"=>:to_unicodemath,"mathml"=>:to_mathml,"omml"=>:to_omml}
values=["1234.56",1234,:number,:'1234',nil,false,["1234"],{x:1234},{"x"=>1234},-1234,9007199254740993]
profile={"locale"=>"en","options"=>{},"precision"=>nil,"stringFormat"=>nil}
rows=values.each_with_index.map do |value,index|
 formula=M::Formula.new([M::Number.new(value)])
 encoded=SourceValues.encode(formula)
 document={schema:"plurimath.ruby-values/1",producer:{implementation:"plurimath-ruby",source_commit:HEAD,runtime:RUBY_DESCRIPTION,codec_sha256:Digest::SHA256.hexdigest(File.read(__FILE__)),identity:"typed-value-tree; object identity is not represented"},root:encoded}
 formatter=Plurimath::Formatter::Standard.new(locale:"en",options:{},precision:nil,string_format:nil)
 writers=methods.transform_values do |method|
  begin
   {output:formula.public_send(method,formatter:formatter)}
  rescue StandardError=>error
   {error:{class:error.class.name,message:error.message,cause_class:error.cause&.class&.name}}
  end
 end
 raise "source mutated" unless SourceValues.encode(formula)==encoded
 {id:"typed-formatter-#{index}",document:document,formatter_options:profile,writers:writers}
end
File.write(ARGV.fetch(0),JSON.pretty_generate({rows:rows,source_head:HEAD,runtime:RUBY_DESCRIPTION,mutations:0}))
puts JSON.generate({cases:rows.size,writer_outcomes:rows.sum{|r|r[:writers].size}})
