require 'json'
values=['0d17','0D17','-0d17','+0D31','  0d17x','0d','0D','0d_17','0d__17','0_d17','00d17','0d1_7','0d1__7','0d+17','0d-17','0d 17','0x17','0b17','0o17','0d017','0d0_17','0d_','\t0D_1_7',' -0d_31','0d999999999999999999999999999999999999999999999999999999999999999999']
File.write(ARGV.fetch(0),JSON.pretty_generate({ruby:RUBY_DESCRIPTION,rows:values.map{|v|{input:v,to_i:v.to_i,mod32:v.to_i%32}}})+"\n")
