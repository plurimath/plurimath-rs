#!/usr/bin/env python3
"""Convert the Ruby source-oracle model graphs to canonical native writer models.

usage: convert.py ORACLE_JSONL OUT_JSONL [--facade DIR]

Mechanical only. The class map is derived from the oracle class list and the
facade registry (every Math:: class spelled in src/ and fixtures/); a class or
field the facade has never seen is a loud error, never a guess. Anonymous
classes are refused. Rows whose source parse failed are emitted as refusals
without a model.
"""
import json, re, sys, pathlib, hashlib

def registry(facade):
    seen = {}
    pat = re.compile(r'"(Math::[A-Za-z0-9_:]+)"')
    for p in list(pathlib.Path(facade, 'src').rglob('*')) + list(pathlib.Path(facade, 'fixtures').rglob('*.json')):
        if p.is_file() and p.suffix in ('.rs', '.json'):
            for m in pat.findall(p.read_text(errors='replace')):
                seen[m] = seen.get(m, 0) + 1
    return seen

def fields_registry(facade):
    """class -> field names seen in fixtures' canonical models."""
    out = {}
    def walk(v):
        if isinstance(v, dict):
            if isinstance(v.get('class'), str) and isinstance(v.get('fields'), dict):
                out.setdefault(v['class'], set()).update(v['fields'])
            for x in v.values(): walk(x)
        elif isinstance(v, list):
            for x in v: walk(x)
    for p in pathlib.Path(facade, 'fixtures').rglob('*.json'):
        try: walk(json.loads(p.read_text()))
        except Exception: pass
    return out

def canon(name):
    assert name.startswith('Plurimath::Math::'), name
    return name[len('Plurimath::'):]

def convert_row(row, reg, freg, problems):
    ruby = row['ruby']
    if not ruby['parse'].get('ok'):
        return None
    graph = ruby['model']
    nodes = graph['nodes']
    classes = graph.get('classes', {})
    def build(i, stack=()):
        if i in stack: raise ValueError('cycle')
        n = nodes[i]
        info = classes.get(n['class'], {})
        if info.get('anonymous'):
            problems.append((row['id'], 'anonymous class', n['class']))
        cls = canon(n['class'])
        if cls not in reg:
            problems.append((row['id'], 'class unknown to facade registry', cls))
        fields = {}
        for k, v in n['fields'].items():
            if cls in freg and k not in freg[cls]:
                problems.append((row['id'], 'field not in any facade fixture', f'{cls}.{k}'))
            fields[k] = value(v, stack + (i,))
        return {'class': cls, 'fields': fields}
    def value(v, stack):
        if isinstance(v, dict):
            if set(v) == {'$node'}: return build(v['$node'], stack)
            if any(k.startswith('$') for k in v):
                problems.append((row['id'], 'unsupported tagged value', list(v)))
            return {k: value(x, stack) for k, x in v.items()}
        if isinstance(v, list): return [value(x, stack) for x in v]
        return v
    return build(graph['root'])

def expected_writers(row, base):
    """Recorded Ruby outcomes: default calls, replaced by the follow-up reruns where
    the call differs (display_style: false for flag 0; the 305 to_unicodemath rerun
    with a longer timeout)."""
    w = json.loads(json.dumps(row['ruby'].get('writers')))
    if w is None:
        return None
    f = pathlib.Path(base, 'followup', 'writers-display', row['id'] + '.json')
    if f.exists():
        d = json.loads(f.read_text())
        assert d['display_flag'] == row['display_flag'] == '0'
        for k, v in d['writers'].items():
            w[k] = {x: y for x, y in v.items() if x != 'same_as_default'}
    elif row['display_flag'] != '1':
        raise SystemExit(f"flag {row['display_flag']} row {row['id']} lacks display rerun")
    sup = pathlib.Path(base, 'data', 'supplement-' + row['id'].split('-')[-1] + '-to_unicodemath.json')
    if sup.exists():
        d = json.loads(sup.read_text())
        assert d['id'] == row['id']
        w['to_unicodemath'] = d['result']
    return w

def main():
    src, out = sys.argv[1], sys.argv[2]
    facade = sys.argv[4] if len(sys.argv) > 4 and sys.argv[3] == '--facade' else '.'
    base = str(pathlib.Path(src).parent.parent)
    reg, freg = registry(facade), fields_registry(facade)
    problems, n_ok, n_ref = [], 0, 0
    with open(out, 'w') as o:
        for line in open(src):
            row = json.loads(line)
            model = convert_row(row, reg, freg, problems)
            if model is None:
                n_ref += 1
            else:
                n_ok += 1
            o.write(json.dumps({'id': row['id'], 'display_flag': row['display_flag'],
                                'model': model, 'source_writers': expected_writers(row, base)},
                               ensure_ascii=False) + '\n')
    print(json.dumps({'rows_with_model': n_ok, 'source_refusals': n_ref,
                      'problems': len(problems), 'sha256': hashlib.sha256(open(src,'rb').read()).hexdigest()}))
    seen = {}
    for p in problems: seen.setdefault((p[1], str(p[2])), []).append(p[0])
    for (kind, what), ids in sorted(seen.items()):
        print(f'{kind}: {what} ({len(ids)} uses, first {ids[0]})')
    sys.exit(1 if problems else 0)
main()
