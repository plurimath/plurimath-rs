# Typed source values, version 1

The existing `{class, fields}` model remains the canonical parser-created JSON
profile. Named option/attribute domains interpret object keys as source Symbols.
JSON string **values** remain Strings. Arrays, including pairs, retain their
literal value kinds. Nested `Mpadded.options[:mpadded]` is an explicitly declared
named-key domain; arbitrary nested hashes remain literal.

`plurimath.ruby-values/1` is an additive, explicit UTF-8 typed-value-tree profile.
It preserves String/Symbol kinds, arbitrary Integer decimal values, Float IEEE
bits, Hash entry order and key kinds, arrays, and represented Core node fields.
It does not preserve Ruby object identity, shared mutable references, binary
String encodings or arbitrary non-Core objects. Cycles are refused by the
candidate producer. It does not claim full Ruby writer parity.

## Native requests

`{"op":"source_model","model":DOCUMENT}` validates and returns the document.
`{"op":"write","source_profile":"plurimath.ruby-values/1","model":DOCUMENT,
"format":"mathml"}` writes through the same native core and existing writers.
No Ruby or Node process is used during evaluation. Omitting `source_profile`
retains the existing canonical request behavior.

A document has exactly `schema`, `producer`, and `root` members. `schema` is the
profile identifier. Producer provenance includes nonempty `implementation`,
`source_commit`, `runtime`, and `codec_sha256` strings. These are producer
assertions, not authenticated signatures. The native writer reference remains
the pinned Plurimath Ruby implementation and runtime used to qualify the library.

Values are tagged arrays:

| Source kind | Wire |
| --- | --- |
| nil | `["nil"]` |
| Boolean | `["bool", true]` |
| Integer | `["integer", "9007199254740993"]` |
| Float | `["float", "3ff0000000000000"]` |
| String | `["string", "width"]` |
| Symbol | `["symbol", "width"]` |
| Array | `["array", [VALUE, ...]]` |
| Hash | `["hash", [[KEY, VALUE], ...]]` |
| Core node | `["node", "Math::Number", [["value", VALUE], ...]]` |

Integers use canonical decimal spelling; Floats use exactly sixteen hexadecimal
IEEE-754 binary64 digits. Hash duplicate keys and duplicate node field names
refuse. Array/Hash/Core-object Hash keys are unsupported; identity-sensitive NaN
key behavior is not qualified. Node class identifiers
must use the `Math::` namespace; this is structural validation, not a guarantee
that every class and state has a supported writer.

The adapter builds verified node-level `source_values` annotations. Their raw
typed fields remain available through `Model::source_field` and
`source_values::field`; `Model::field` is only the compatibility projection.
Hashes and Symbols use opaque projections, so they cannot silently become a
legacy object or model. Unsupported provenance-sensitive writers refuse before
falling back to legacy procedures.

## Accessors and supported operations

`decode(value, domain, profile)` and `hash` accept explicit `Profile` and
`Domain`. `in_domain` rebinds only canonical Hash keys; it never changes typed
keys or String values. `get_symbol` and `get_string` perform distinct lookups.
`equal` and `eql` compare supported scalar/Array/Hash values without depending on
Hash insertion order. General Core-object equality is not qualified.

`source_value` requests support `roundtrip`, `to_s`, and `inspect` actions. Ruby
4.0.1 Hash inspection uses Symbol labels and String-key arrows. Unqualified
non-ASCII Symbol inspection, identity-bearing object inspection and Float display
currently return explicit unsupported errors; their values still roundtrip.

The qualified writer slice includes both variants of the seven layout/n-ary
collision pairs and two Number Array/Hash pairs. This profile fixes their type
ambiguity rather than changing archived canonical oracles. Other formatter,
transformation and writer contexts retain explicit capability limits.
