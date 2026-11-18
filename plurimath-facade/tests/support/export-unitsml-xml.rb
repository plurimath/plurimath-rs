# Usage: ruby export-unitsml-xml.rb UNITSDB_JSON OUTPUT_JSON
# Exports Formula#to_mathml(unitsml: {xml: true}) for every UnitsDB unit
# symbol and for the k, u, m and M prefixed SI unit symbols.
require "plurimath"
require "json"
require "digest"

data = JSON.parse(File.read(ARGV.fetch(0)))
symbols = data["units"].flat_map { |unit| unit["symbols"].map { |s| s["id"] } }.uniq
si = data["units"].select do |unit|
  unit["unit_system_reference"].to_a.any? { |r| r["id"].to_s.downcase.start_with?("si") }
end.flat_map { |unit| unit["symbols"].map { |s| s["id"] } }.uniq
inputs = symbols + %w[k u m M].flat_map { |prefix| si.map { |symbol| prefix + symbol } }
rows = inputs.uniq.map do |input|
  formula = begin
    Plurimath::Math.parse(%("unitsml(#{input})"), :asciimath)
  rescue StandardError
    next { "input" => input, "parse_error" => true }
  end
  row = { "input" => input }
  begin
    row["xml"] = formula.to_mathml(unitsml: { xml: true })
  rescue StandardError => e
    row["xml_error"] = e.class.name
  end
  row
end
plurimath = Gem.loaded_specs.fetch("plurimath")
File.binwrite(ARGV.fetch(1), JSON.pretty_generate(
  "profile" => "source-unitsml-xml/1",
  "ruby" => RUBY_DESCRIPTION,
  "plurimath" => plurimath.version.to_s,
  "unitsml" => Gem.loaded_specs.fetch("unitsml").version.to_s,
  "unitsdb_sha256" => Digest::SHA256.file(ARGV.fetch(0)).hexdigest,
  "producer_sha256" => Digest::SHA256.file(__FILE__).hexdigest,
  "rows" => rows,
) + "\n")
puts JSON.generate(rows: rows.length, xml: rows.count { |r| r["xml"] })
