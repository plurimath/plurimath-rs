"""Reproduce a focused XML stage byte-oracle sample from pinned shared payloads.

This copies recorded XML bytes/refusals, never regenerates expected output.
Requires PyYAML. --check fetches and verifies the committed sample without writing.
"""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request

import yaml

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "plurimath-facade/fixtures/xml-stage-smoke.json"
REPOSITORY = "plurimath/plurimath-testsuite"
# (language, revision, generation, payload suffix, payload SHA-256, row suffixes)
SOURCES = [('mathml',
  '674126a07fa1cf89c24bc50fadb305299e1d1b18',
  3,
  'tokens',
  '6a2a6fee16103c73a90ed31fed41b7bdf0838b7d669db1c657d54cd22d828a00',
  ['tokens-001', 'tokens-006', 'tokens-030', 'tokens-031']),
 ('mathml',
  '674126a07fa1cf89c24bc50fadb305299e1d1b18',
  3,
  'fractions',
  '49ed145d3649adae507c0f51ab391c318a86e6f2956f6f548fcdc8b97da3d6c9',
  ['fractions-003']),
 ('mathml',
  '674126a07fa1cf89c24bc50fadb305299e1d1b18',
  3,
  'rejections',
  'fe416473e7407f4ff9f1a6b4693e65f1eb21d8fbfb32f62eceace4bb94e32cc2',
  ['bare-text']),
 ('omml',
  '9767c37a8d7bb21a0694780ecfcbc4f1fbd9eb39',
  2,
  'runs',
  '8d5af13a72562502cdbc247f6acceac78d6e390d2cac5ad1247bbbb8774e0c82',
  ['runs-019', 'runs-023', 'runs-024']),
 ('omml',
  '9767c37a8d7bb21a0694780ecfcbc4f1fbd9eb39',
  2,
  'rejections',
  '8a82db8d675b8d98a03721740edd853796611856f55a581a644ae407a804768c',
  ['bare-text', 'unbound-prefix'])]


def export():
    rows = []
    sources = []
    for language, revision, generation, suffix, digest, selected in SOURCES:
        path = f"stages/{language}/generations/{generation:06d}/payloads/model-roundtrip-{suffix}.yaml"
        url = f"https://raw.githubusercontent.com/{REPOSITORY}/{revision}/{path}"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read()
        if hashlib.sha256(data).hexdigest() != digest:
            raise ValueError(f"payload checksum mismatch: {path}")
        document = yaml.safe_load(data)
        if document["boundary"] != "model-roundtrip":
            raise ValueError(f"unexpected boundary: {path}")
        by_id = {row["id"]: row for row in document["cases"]}
        source_index = len(sources)
        sources.append({"revision": revision, "path": path, "sha256": digest})
        for suffix_id in selected:
            row_id = f"{language}-{suffix_id}.model-roundtrip"
            row = by_id[row_id]
            if row.get("options") not in (None, {}):
                raise ValueError(f"unsupported options in {row_id}")
            expected = row["expected"]
            if "refusal" in expected:
                output = {"refusal": expected["refusal"]}
            else:
                output = {"output": expected["output"]["str"]}
            rows.append({"id": row_id, "format": language, "source": source_index,
                         "input": row["input"]["str"], "expected": output})
    if len({row["id"] for row in rows}) != len(rows) or not rows:
        raise ValueError("empty or duplicate sample")
    return json.dumps({"repository": REPOSITORY, "scope": "stage XML output bytes and refusals; not common-model wire parity",
                       "sources": sources, "cases": rows}, ensure_ascii=False, indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = export()
    if args.check:
        if OUT.read_text(encoding="utf-8") != content:
            raise SystemExit("committed XML stage sample differs from the pinned shared payloads")
    else:
        OUT.write_text(content, encoding="utf-8")
    document = json.loads(content)
    counts = {language: sum(row["format"] == language for row in document["cases"])
              for language in ("mathml", "omml")}
    print(json.dumps({"cases": counts, "payloads_verified": len(document["sources"])}))


if __name__ == "__main__":
    main()
