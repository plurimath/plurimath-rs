# frozen_string_literal: true
# Build-time source graph export; the native runtime never loads Ruby.
require "json"
require "digest"
require "open3"
require "plurimath"
require "parsanol/native/serializer"
require "parsanol/parg/compiler"

module PortableHtmlSource
  # The source HTML grammar names are ASCII; this is an explicit portable
  # captured-literal operation, never a process-local dynamic callback.
  class StateMatchAscii < Parsanol::Atoms::StateMatch
    def try(source, context, _consume_all)
      mark_unsafe(context)
      expected = lookup(context, slot)
      return [false, nil] if expected.nil?
      expected = expected.to_s
      actual = source.peek(expected.bytesize)
      return [false, nil] unless actual.downcase(:ascii) == expected.downcase(:ascii)
      source.bytepos += expected.bytesize
      [true, actual]
    end
  end

  class Serializer < Parsanol::GrammarSerializer
    private
    def serialize_state_match(atom)
      return super unless atom.is_a?(StateMatchAscii)
      { "StateMatchAscii" => { "slot" => atom.slot.to_s } }
    end
  end

  # The one source dynamic closure has this exact structural lowering:
  # parse_tag(:close, captured_name) => '</' + captured ASCII comparison
  # + the original tag-name boundary + '>'. Scope/capture stay source-owned.
  class Parse < Plurimath::Html::Parse
    def matching_close_tag
      str("</") >> StateMatchAscii.new(:html_tag_name) >> tag_name_boundary >> str(">")
    end
  end
end

output = File.expand_path(ARGV.fetch(0))
source_file, source_line = Plurimath::Html::Parse.instance_method(:matching_close_tag).source_location
source_sha = Digest::SHA256.file(source_file).hexdigest
expected_source = "b6262929322518d8d14381a4d67b86460c7e019b3baef3c8b5bc2db7b6398d9e"
raise "HTML grammar source changed; requalify closure lowering" unless source_sha == expected_source
raise "this graph declares decimal period" unless Plurimath.configuration.decimal == "."
parser = PortableHtmlSource::Parse.new
graph = JSON.parse(PortableHtmlSource::Serializer.serialize(parser.root, preserve_identity: true))
raise "process-local callback remained" if graph.fetch("atoms").any? { |atom| atom.key?("Dynamic") || atom.key?("Custom") }
ascii_count = graph.fetch("atoms").count { |atom| atom.key?("StateMatchAscii") }
raise "source close-tag operation absent" if ascii_count.zero?
serializer_file = Parsanol::GrammarSerializer.instance_method(:serialize).source_location.first
core_root = File.expand_path("../../..", source_file)
head, status = Open3.capture2("git", "-C", core_root, "rev-parse", "HEAD")
raise "source Git pin unavailable" unless status.success?
artifact = {
  "grammar" => "PlurimathHtmlSource", "version" => "0.1.0", "edition" => 2,
  "shape" => "parsanol-tree/v2",
  "entries" => { "document" => { "root" => "expression", "grammar" => graph, "bindings" => [] } },
  "provenance" => {
    "source_language" => "ruby-parser-graph", "core_head" => head.strip,
    "grammar_source_sha256" => source_sha, "matching_close_tag_line" => source_line,
    "serializer_sha256" => Digest::SHA256.file(serializer_file).hexdigest,
    "producer_sha256" => Digest::SHA256.file(__FILE__).hexdigest,
    "decimal" => ".", "lowering" => "explicit captured ASCII tag comparison plus original boundary/scope",
    "validated" => "source graph serialized; native/C/WASM replay receipts are separate"
  }
}
artifact["checksum"] = Parsanol::PARG::Compiler.checksum(artifact)
File.write(output, JSON.pretty_generate(artifact) + "\n")
puts JSON.generate(atoms: graph.fetch("atoms").size, ascii_state_matches: ascii_count, checksum: artifact["checksum"], output_bytes: File.size(output))
