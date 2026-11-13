require "json"
require "digest"
require ENV.fetch("PLURIMATH_CORPUS_GENERATOR")
CorpusGenerator.require_ox_engine!
M = Plurimath::Math
P = M::Symbols::Paren
source_root = ENV.fetch("PLURIMATH_SOURCE_ROOT")
source_head = IO.popen(["git","-C",source_root,"rev-parse","HEAD"], &:read).strip
raise "Expected clean Ruby source 0a1df68a" unless source_head == "0a1df68a5903d43da13b1e1ef85b6c0523feb1b4" && IO.popen(["git","-C",source_root,"status","--porcelain"], &:read).empty?
owner_file = M::Function::Fenced.instance_method(:line_breaking).source_location.first
raise "Fenced loaded outside the pinned source" unless File.realpath(owner_file).start_with?(File.realpath(source_root) + "/")
METHODS = {"asciimath"=>:to_asciimath,"latex"=>:to_latex,"unicodemath"=>:to_unicodemath,"html"=>:to_html,"mathml"=>:to_mathml,"omml"=>:to_omml}.freeze
rows = []
construct = []
def capture(id, node, options = {}, root_nodes = nil)
  root = M::Formula.new(root_nodes || [node])
  model = CorpusGenerator.serialize_node(root, "$")
  writers = METHODS.transform_values do |method|
    object = Marshal.load(Marshal.dump(root))
    begin
      {"output"=>object.public_send(method, **options)}
    rescue StandardError => e
      {"error"=>{"class"=>e.class.name,"message"=>e.message}}
    end
  end
  {"id"=>id,"model"=>model,"options"=>options,"writers"=>writers,"mutated"=>model != CorpusGenerator.serialize_node(root,"$")}
end
def make(id, left, body, right, node_options, rows, construct)
  begin
    node = M::Function::Fenced.new(left, body, right, node_options)
    rows << capture(id, node)
  rescue StandardError => e
    construct << {"id"=>id,"error"=>{"class"=>e.class.name,"message"=>e.message}}
  end
end
pairs = {
  "round"=>[P::Lround.new,P::Rround.new], "square"=>[P::Lsquare.new,P::Rsquare.new],
  "vert"=>[P::Vert.new,P::Vert.new], "norm"=>[P::Norm.new,P::Norm.new],
  "nil"=>[nil,nil], "left-nil"=>[nil,P::Rround.new], "right-nil"=>[P::Lround.new,nil],
  "raw-vert-symbol"=>[M::Symbols::Symbol.new("|"),M::Symbols::Symbol.new("|")],
  "invisible"=>[M::Symbols::Symbol.new("{:"),M::Symbols::Symbol.new(":}")],
  "entity"=>[M::Symbols::Symbol.new("&#x3016;"),M::Symbols::Symbol.new("&#x3017;")],
  "number"=>[M::Number.new("1"),M::Number.new("2")]
}
bodies = {
  "nil"=>nil, "empty"=>[], "one"=>[M::Symbols::Symbol.new("x")],
  "two"=>[M::Symbols::Symbol.new("x"),M::Number.new("2")], "with-nil"=>[nil],
  "mini"=>[M::Number.new("2",mini_sub_sized:true),M::Number.new("3",mini_sub_sized:true)],
  "fraction"=>[M::Function::Frac.new(M::Number.new("1"),M::Number.new("2"))],
  "choose"=>[M::Function::Frac.new(M::Number.new("3"),M::Number.new("4"),{choose:true})],
  "choose-false"=>[M::Function::Frac.new(M::Number.new("5"),M::Number.new("6"),{choose:false})],
  "nested"=>[M::Function::Fenced.new(P::Lsquare.new,[M::Symbols::Symbol.new("z")],P::Rsquare.new)]
}
profiles = {"default"=>{}, "nil-options"=>nil, "prefix"=>{open_prefixed:true,close_prefixed:true}, "attrs"=>{open_paren:{minsize:"1.25em",stretchy:false},close_paren:{minsize:"1.5625em",stretchy:true}}, "separator-node"=>{separators:";"}}
pairs.each { |pn,(l,rr)| bodies.each { |bn,body| profiles.each { |on,opt| make("#{pn}/#{bn}/#{on}",l,body,rr,opt,rows,construct) } } }
["(","|","{:"].product([")","|",":}"]).each do |l,rr|
  node = M::Function::Fenced.new(nil,[M::Number.new("7")],nil,{})
  begin
    node.open = l; node.close = rr; node.separators = ","
    rows << capture("setter/#{l}/#{rr}",node)
  rescue StandardError => e
    construct << {"id"=>"setter/#{l}/#{rr}","error"=>{"class"=>e.class.name,"message"=>e.message}}
  end
end
["(",false,1].each do |raw|
  node = M::Function::Fenced.new(nil,[M::Symbols::Symbol.new("x")],nil,{})
  node.parameter_one = raw; node.parameter_three = raw
  rows << capture("raw/#{raw.inspect}",node)
end
if ENV["FENCED_INDEPENDENT"] == "1"
  rows.clear
  construct.clear
  children = {
    "compound"=>[M::Function::Sin.new(M::Formula.new([M::Symbols::Symbol.new("z"),M::Symbols::Plus.new,M::Number.new("17")]))],
    "mixed"=>[M::Number.new("19"),M::Symbols::Symbol.new("q"),M::Number.new("23")],
    "later-choose"=>[M::Symbols::Symbol.new("t"),M::Function::Frac.new(M::Number.new("29"),M::Number.new("31"),{choose:false})],
    "choose-nil"=>[M::Function::Frac.new(nil,M::Symbols::Symbol.new("q"),{choose:true})],
    "nested-mini"=>[M::Function::Fenced.new(P::Lround.new,[M::Number.new("7",mini_sup_sized:true)],P::Rround.new)],
    "mini-symbols"=>[M::Symbols::Symbol.new("x",mini_sub_sized:true),M::Symbols::Symbol.new("+",mini_sup_sized:true)]
  }
  ["nested-mini","mini-symbols","mixed"].each do |shape|
    fence=M::Function::Fenced.new(P::Lround.new,children[shape],P::Rround.new)
    rows << capture("outer-formula/#{shape}",fence,{},[fence,M::Symbols::Symbol.new("w")])
  end
  %w[Langle Rangle Lfloor Rfloor Lceil Rceil Lcurly Rcurly Lbrace Rbrace Lbbrack Rbbrack OpenParen CloseParen UpcaseLangle UpcaseRangle].each do |name|
    children.each do |id,body|
      left=P.const_get(name).new
      make("unseen/#{name}/#{id}",left,body,P::Rround.new,{},rows,construct)
    end
  end
  %w[1em 2em 0.8em 1.000em 2.44140625em 3.14159em 1.25foo bad 0em -1em 0.99em 2e 2e+ 2_5em .5em].each do |size|
    make("size/#{size}",P::Lround.new,children["mixed"],P::Rround.new,{open_paren:{minsize:size},close_paren:{minsize:size}},rows,construct)
  end
  [nil,false,{},"bad"].each do |size|
    make("size-type/#{size.inspect}",P::Lround.new,children["mixed"],P::Rround.new,{open_paren:{minsize:size}},rows,construct)
  end
  [M::Number.new("4",mini_sub_sized:true),M::Number.new("5",mini_sup_sized:true)].each_with_index do |mini,index|
    make("mini-delimiter/#{index}",mini,children["mixed"],mini,{},rows,construct)
  end
  [P::Lround.new,P::Vert.new].each_with_index do |paren,index|
    paren.mini_sup_sized=true
    make("mini-paren/#{index}",paren,children["mixed"],P::Rround.new,{},rows,construct)
  end
  ["(","+","x","unknown","&"].each do |value|
    [true,false].each do |sub|
      symbol=M::Symbols::Symbol.new(value,mini_sub_sized:sub,mini_sup_sized:!sub)
      make("mini-symbol-delimiter/#{value}/#{sub}",symbol,children["mixed"],symbol,{},rows,construct)
    end
  end
  [false,"bad",1,{}].each do |body|
    make("raw-body/#{body.inspect}",P::Lround.new,body,P::Rround.new,{},rows,construct)
  end
  [{open_prefixed:false,close_prefixed:nil},{open_paren:{stretchy:nil,minsize:"1.25em",fence:true}},{open_paren:{minsize:"1.25em",fence:"false",stretchy:"true"}}].each_with_index do |options,index|
    make("node-options/#{index}",M::Symbols::Symbol.new("{:"),children["mixed"],M::Symbols::Symbol.new(":}"),options,rows,construct)
  end
end
source = ENV.fetch("PLURIMATH_SOURCE_ROOT")
result = {"profile"=>"source-fenced-default/1","source"=>source_head,"ruby"=>RUBY_DESCRIPTION,"engine"=>"Ox","rows"=>rows,"constructor_refusals"=>construct}
File.binwrite(ARGV.fetch(0),JSON.generate(result)+"\n")
mini_constants=%i[SUB_ALPHABETS SUB_OPERATORS SUB_PARENTHESIS SUP_ALPHABETS SUP_OPERATORS SUP_PARENTHESIS].to_h{|name|[name.to_s,Plurimath::UnicodeMath::Constants.const_get(name)]}
File.binwrite(ARGV.fetch(1),JSON.generate({"producer_sha256"=>Digest::SHA256.file(__FILE__).hexdigest,"source_sha256"=>Digest::SHA256.file(File.join(source,"lib/plurimath/math/function/fenced.rb")).hexdigest,"mini_constants"=>mini_constants})+"\n")
puts JSON.generate({models:rows.length,observations:rows.length*6,constructors:construct.length,mutations:rows.count{|r|r["mutated"]}})
