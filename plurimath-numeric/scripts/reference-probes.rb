require "json"
require "digest"
require ARGV.fetch(0)
requests = JSON.parse(File.read(ARGV.fetch(1)))
requests.fetch("cases").each do |row|
  row["expected_wire"] = Plurimath::EvaluationRequest.call(JSON.generate(row.fetch("request")))
end
requests["oracle"] = {"adapter_sha256" => Digest::SHA256.file(ARGV.fetch(0)).hexdigest, "ruby" => RUBY_DESCRIPTION}
File.write(ARGV.fetch(2), JSON.pretty_generate(requests) + "\n")
puts JSON.generate({"executed" => requests.fetch("cases").length})
