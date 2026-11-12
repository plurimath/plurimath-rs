require_relative "export-fences"
rows=[]
state_rows=[]
def symbol(value); M::Symbols::Symbol.new(value); end
breaks={"plain"=>M::Function::Linebreak.new,"after"=>M::Function::Linebreak.new(symbol("p"),{linebreakstyle:"after"}),"before"=>M::Function::Linebreak.new(symbol("q"),{linebreakstyle:"before"}),"array-before"=>M::Function::Linebreak.new([symbol("r"),symbol("s")]),"array-after"=>M::Function::Linebreak.new([symbol("r"),symbol("s")],{linebreakstyle:"after"}),"empty-array"=>M::Function::Linebreak.new([]),"false"=>M::Function::Linebreak.new(false)}
shapes={"empty"=>[],"nil"=>nil,"trailing"=>[symbol("a"),M::Function::Linebreak.new],"nested-formula"=>[M::Formula.new([symbol("a"),M::Function::Linebreak.new,symbol("b")])],"nested-fence"=>[M::Function::Fenced.new(P::Lsquare.new,[symbol("a"),M::Function::Linebreak.new,symbol("b")],P::Rsquare.new)]}
breaks.each do |name,marker|
  shapes["direct/#{name}"]=[symbol("a"),marker,symbol("b"),symbol("c")]
  shapes["multiple/#{name}"]=[symbol("d"),marker,symbol("e"),M::Function::Linebreak.new,symbol("f")]
  shapes["leading/#{name}"]=[marker,symbol("g")]
end
[[P::Lround.new,P::Rround.new],[P::Vert.new,P::Vert.new],[nil,nil]].each_with_index do |(left,right),index|
  shapes.each do |shape,body|
    node=M::Function::Fenced.new(left,body,right,{open_prefixed:true,separators:"ignored"})
    input=CorpusGenerator.serialize_node(node,"$")
    [[],[symbol("pending")]].each_with_index do |pending,pindex|
      object=Marshal.load(Marshal.dump(node));target=M::Formula.new(Marshal.load(Marshal.dump(pending)))
      expected=begin
        object.line_breaking(target)
        {"node"=>CorpusGenerator.serialize_node(object,"$"),"continuation"=>target.value.map{|v|CorpusGenerator.serialize_value(v,"$")}}
      rescue StandardError=>e
        {"error"=>{"class"=>e.class.name,"message"=>e.message}}
      end
      state_rows << {"id"=>"#{index}/#{shape}/#{pindex}","model"=>input,"pending"=>pending.map{|v|CorpusGenerator.serialize_node(v,"$")},"expected"=>expected}
    end
    %w[mathml omml].each do |format|
      root=M::Formula.new([Marshal.load(Marshal.dump(node))])
      model=CorpusGenerator.serialize_node(root,"$")
      result=begin;{"output"=>root.public_send("to_#{format}",split_on_linebreak:true)};rescue StandardError=>e;{"error"=>{"class"=>e.class.name,"message"=>e.message}};end
      rows<<{"id"=>"#{index}/#{shape}/#{format}","model"=>model,"options"=>{"splitOnLinebreak"=>true},"writers"=>{format=>result}}
    end
  end
end
File.binwrite(ARGV.fetch(0),JSON.generate({"profile"=>"source-fenced-linebreaking/1","source"=>IO.popen(["git","-C",ENV.fetch("PLURIMATH_SOURCE_ROOT"),"rev-parse","HEAD"], &:read).strip,"rows"=>rows,"states"=>state_rows})+"\n")
puts JSON.generate({split_writer_observations:rows.length,state_observations:state_rows.length})
