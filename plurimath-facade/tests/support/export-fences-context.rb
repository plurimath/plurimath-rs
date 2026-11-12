require_relative "export-fences"
rows = []
nodes = {
  "simple"=>[M::Function::Fenced.new(P::Lround.new,[M::Function::Sin.new(M::Number.new("17"))],P::Rround.new)],
  "empty"=>[M::Function::Fenced.new(nil,[],nil)],
  "null-child"=>[M::Function::Fenced.new(nil,[nil],nil)],
  "choose"=>[M::Function::Fenced.new(P::Lround.new,[M::Function::Frac.new(M::Symbols::Symbol.new("p"),M::Number.new("37"),{choose:true})],P::Rround.new)],
  "choose-false"=>[M::Function::Fenced.new(P::Lround.new,[M::Function::Frac.new(M::Symbols::Symbol.new("r"),M::Number.new("41"),{choose:false})],P::Rround.new)],
  "choose-nil"=>[M::Function::Fenced.new(P::Lround.new,[M::Function::Frac.new(nil,M::Number.new("43"),{choose:true})],P::Rround.new)],
  "linebreak"=>[M::Function::Fenced.new(P::Lround.new,[M::Symbols::Symbol.new("s"),M::Function::Linebreak.new,M::Symbols::Symbol.new("t")],P::Rround.new)]
}
[[P::Lround.new,P::Rsquare.new],[P::Lsquare.new,P::Rsquare.new],[P::Lsquare.new,P::Rround.new],[P::Rsquare.new,P::Lsquare.new]].each_with_index do |(left,right),index|
  nodes["interval/#{index}"] = [M::Function::Fenced.new(left,[M::Number.new("47"),M::Symbols::Symbol.new(","),M::Symbols::Symbol.new("q")],right)]
end
nodes.each do |name,children|
  [true,false].product([true,false],[true,false]).each do |intent,spacing,style|
    %w[mathml omml].each do |format|
      opts = format == "mathml" ? {intent:intent,unary_function_spacing:spacing,display_style:style} : {display_style:style}
      root=M::Formula.new(children)
      object=Marshal.load(Marshal.dump(root))
      result=begin;{"output"=>object.public_send("to_#{format}",**opts)};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
      native=opts.transform_keys{|k|{display_style:"displayStyle",unary_function_spacing:"unaryFunctionSpacing"}[k] || k.to_s}
      rows << {"id"=>"#{name}/#{intent}/#{spacing}/#{style}/#{format}","model"=>CorpusGenerator.serialize_node(root,"$"),"writers"=>{format=>result},"options"=>native}
    end
  end
end
source=ENV.fetch("PLURIMATH_SOURCE_ROOT")
source_head=IO.popen(["git","-C",source,"rev-parse","HEAD"], &:read).strip
File.binwrite(ARGV.fetch(0),JSON.generate({"profile"=>"source-fenced-context/1","source"=>source_head,"ruby"=>RUBY_DESCRIPTION,"engine"=>"Ox","rows"=>rows})+"\n")
puts JSON.generate({context_models:rows.length,observations:rows.length})
