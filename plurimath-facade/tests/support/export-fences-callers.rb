require_relative "export-fences"
rows=[]
[nil,"",";","|",false,2].each do |separator|
  node=M::Function::Fenced.new(P::Lround.new,[M::Number.new("53")],P::Rround.new,{separators:"ignored node value"})
  caller={separators:separator}
  fragment=node.to_omml_without_math_tag(true,options:caller).first
  rows << {"model"=>CorpusGenerator.serialize_node(node,"$"),"caller"=>caller,"output"=>node.dump_nodes(fragment,indent:2)}
end
File.binwrite(ARGV.fetch(0),JSON.generate({"profile"=>"source-fenced-internal-callers/1","rows"=>rows})+"\n")
puts JSON.generate({caller_observations:rows.length})
