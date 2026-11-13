require 'json'
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR", "/home/apple/projects/plurimath-org/workspace/core/ts-ci-fix/scripts/generate-corpus.rb")
CorpusGenerator.require_ox_engine!
metadata=JSON.parse(File.read(ENV.fetch("PLURIMATH_WRITER_METADATA"), encoding: "UTF-8"))
rows=metadata.fetch('rows').map do |r|
 k=Object.const_get('Plurimath::'+r.fetch('class'))
 info={class:r['class'],hide:k.instance_methods.include?(:hide_function_name),unary:!!(k < Plurimath::Math::Function::UnaryFunction)}
 begin;o=k.new(nil);info[:variables]=o.variables.map{|v|v.to_s.delete_prefix('@')};rescue StandardError=>e;info[:constructor_error]={class:e.class.name,message:e.message};end
 info
end
File.write(ARGV[0],JSON.pretty_generate({source:'0a1df68a5903d43da13b1e1ef85b6c0523feb1b4',ruby:RUBY_DESCRIPTION,rows:rows})+"\n", encoding: "UTF-8");puts JSON.generate(rows:rows.size)
