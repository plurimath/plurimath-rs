require "json"
def encode(v)
 case v
 when NilClass then ["nil"]
 when TrueClass,FalseClass then ["bool",v]
 when Integer then ["integer",v.to_s]
 when Float then ["float",[v].pack("G").unpack1("H*")]
 when String then ["string",v]
 when Symbol then ["symbol",v.to_s]
 when Array then ["array",v.map{|x|encode(x)}]
 when Hash then ["hash",v.map{|k,x|[encode(k),encode(x)]}]
 else raise "unsupported source value"
 end
end
names=["width","foo bar","foo?","foo!","foo=","+","[]","with-dash","α","","a\nb"]
values=[nil,true,false,1,2**100,"0","a\nb","a\"b","\u0000","\u0001","\u007f",'#{x}','#@x',"α😀"]
names.each{|v|values << v.to_sym;values << {v.to_sym=>"0"};values << {v=>"0"}}
values += [[ :x,"x",nil,1], {width:"0",height:"1"},{"width"=>"0",:width=>"1"},{mpadded:{width:"0"}}, [["width","0"]], [{x:1},{"x"=>1}]]
File.binwrite(ARGV.fetch(0),JSON.generate({ruby:RUBY_DESCRIPTION,rows:values.each_with_index.map{|v,i|{id:i,wire:encode(v),to_s:v.to_s,inspect:v.inspect}}})+"\n")
puts JSON.generate({rows:values.length})
