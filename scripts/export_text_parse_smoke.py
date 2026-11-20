"""Extract six text-stage cases from an immutable shared corpus (requires PyYAML)."""
import argparse
import hashlib
import json
from pathlib import Path
from urllib.request import urlopen

import yaml

SOURCE = "plurimath/plurimath-testsuite"
REVISION = "025eff6747230fed73da1316f2dc9aebb6e98801"
SELECTIONS = [
    ("asciimath", "000006", "frac.yaml", ["frac-simple", "frac-numeric"]),
    ("latex", "000004", "numbers.yaml", ["latex-number-integer", "latex-number-decimal"]),
    ("unicode", "000005", "fences.yaml", ["unicode-fence-round", "unicode-fence-square"]),
]
TARGET = Path(__file__).resolve().parents[1] / "plurimath-facade/fixtures/text-parse-smoke.json"


def export():
    payloads, rows = [], []
    for stage, generation, filename, ids in SELECTIONS:
        path = f"stages/{stage}/generations/{generation}/payloads/{filename}"
        url = f"https://raw.githubusercontent.com/{SOURCE}/{REVISION}/{path}"
        with urlopen(url, timeout=60) as response:
            raw = response.read()
        payloads.append({"path": path, "sha256": hashlib.sha256(raw).hexdigest()})
        cases = {row["id"]: row for row in yaml.safe_load(raw)["cases"]}
        for row_id in ids:
            case = cases[row_id]
            rows.append({
                "id": row_id,
                "format": stage,
                "input": case["input"],
                "expected": {key: case["expected"][key] for key in ("asciimath", "latex", "unicodemath")},
            })
    return json.dumps({
        "source": SOURCE,
        "revision": REVISION,
        "purpose": "Focused facade routing and writer bytes; not full corpus or generated-class parity",
        "payloads": payloads,
        "rows": rows,
    }, ensure_ascii=False, indent=2) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Verify the committed extraction without writing")
    args = parser.parse_args()
    content = export()
    if args.check:
        if TARGET.read_text() != content:
            raise SystemExit("Shared-corpus extraction differs")
        print("Six cases from three pinned shared-corpus payloads verified")
    else:
        TARGET.write_text(content)
        print("Exported six shared-corpus cases")
