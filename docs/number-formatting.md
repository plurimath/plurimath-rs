# Number formatting

`write` accepts an explicit `options.formatterOptions` object. Settings apply to that request and its child writers. Number model classes and fields remain intact.

```json
{"op":"write","format":"asciimath","model":{"class":"Math::Formula","fields":{"value":[{"class":"Math::Number","fields":{"value":"1910"}}]}},"options":{"formatterOptions":{"options":{"base":2,"group":" ","groupDigits":8}}}}
```

Output: `111 01110110_(2)`.

The default `profile: "standard"` follows Ruby `Formatter::Standard`, including its explicit symbol defaults. Its members are `locale`, `precision`, `stringFormat`, and `options`. The alternative `profile: "base"` follows Ruby `NumberFormatter`; its members are `locale`, `precision`, `localizeNumber`, and `localizerSymbols`. The two source classes have different defaults.

Named symbol options accept source snake_case keys or the corresponding documented camelCase forms: `basePrefix`, `basePostfix`, `hexCapital`, `groupDigits`, `fractionGroup`, `fractionGroupDigits`, `digitCount`, `paddingDigits`, `paddingGroupDigits`, `numberSign`, and `exponentSign`. Duplicate aliases are rejected. String values remain strings.

The four text writers also accept `options.format`, following the source caller's number format hash. MathML and OMML expose no corresponding public Formula keyword and reject it. OMML number formatting uses plain formatted text in `t_tag` and font contexts; MathML uses the source structural number representation. Parent operators and their spacing retain their own behavior.

Omitting `formatterOptions`, or setting it to `null` or `false`, keeps ordinary rendering. No process configuration or thread-local formatter is consulted.

Canonical option values currently support null, booleans, strings, and integers within i64. Float spelling and Array/Hash inspection require additional Ruby type information and remain explicit `unported_capability` refusals. Integer options outside i64 and expansions beyond 1,000,000 digits return a native `resource_limit`; these are declared native limits, not Ruby error equivalence.

Formatter construction errors retain their source class. Number-rendering errors follow Formula's `wrap_render_error` and return `Plurimath::Math::ParseError` with a structured underlying cause. Direct Number rendering retains the underlying source class.

Reference: Ruby `0a1df68a5903d43da13b1e1ef85b6c0523feb1b4`, the 27 formatter/Number/Formula/configuration source files. `fixtures/formatter/ruby-standard66.json` freezes all 396 raw writer outputs. Composite/error fixtures are independent source observations. Comparisons do not normalize XML, entities, or whitespace.
