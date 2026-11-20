#!/usr/bin/env node
// Exercise the official wasm-bindgen web glue and the owned ABI-v1 JSON wire.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { parseArgs } from 'node:util';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const { values } = parseArgs({ options: {
  wasm: { type: 'string' }, glue: { type: 'string' }, native: { type: 'string' },
  'wasm-only': { type: 'boolean', default: false },
  cases: { type: 'string', default: new URL('./facade_wasm_cases.json', import.meta.url).pathname },
  oracle: { type: 'string', default: new URL('../plurimath-facade/fixtures/native-slice-ruby.json', import.meta.url).pathname },
} });
assert(values.wasm && values.glue, '--wasm and --glue are required');
assert(Boolean(values.native) !== values['wasm-only'], 'choose --native EXECUTABLE or explicit --wasm-only');
const cases = JSON.parse(readFileSync(values.cases, 'utf8'));
const oracle = JSON.parse(readFileSync(values.oracle, 'utf8'));
assert.equal(cases.schema, 'plurimath-facade-wasm-smoke/1');
assert(cases.parse?.length > 0 && cases.refusals?.length > 0 && cases.writers?.length > 0,
  'parse, refusal and writer selections must be nonempty');
assert(oracle.rows?.length > 0, 'the frozen Ruby oracle must be nonempty');
for (const format of ['asciimath', 'latex', 'unicode']) {
  assert(cases.parse.some(row => row.format === format), `missing ${format} parse coverage`);
}
const bytes = readFileSync(values.wasm);
const { default: init } = await import(pathToFileURL(resolve(values.glue)).href);
const wasm = await init({ module_or_path: bytes });
for (const name of ['plurimath_facade_abi_version', 'plurimath_facade_alloc',
  'plurimath_facade_dealloc', 'plurimath_facade_json', 'plurimath_facade_free']) {
  assert.equal(typeof wasm[name], 'function', `missing ${name}`);
}
assert(wasm.memory instanceof WebAssembly.Memory, 'missing WASM memory');
assert.equal(wasm.plurimath_facade_abi_version(), 1);
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const counts = { requests: 0, nativeComparisons: 0, rubyModels: 0, rubyWriters: 0, refusals: 0 };
function wasmRequest(wire) {
  const input = encoder.encode(wire);
  const pointer = wasm.plurimath_facade_alloc(input.length);
  assert(pointer, 'input allocation failed');
  let result = 0;
  try {
    new Uint8Array(wasm.memory.buffer, pointer, input.length).set(input);
    result = wasm.plurimath_facade_json(pointer, input.length);
    assert(result, 'missing owned result');
    // A request can grow memory, so acquire a fresh view after the call.
    const memory = new Uint8Array(wasm.memory.buffer);
    assert(result < memory.length, 'result outside memory');
    const end = memory.indexOf(0, result);
    assert(end >= result, 'unterminated result');
    return decoder.decode(memory.subarray(result, end));
  } finally {
    if (result) wasm.plurimath_facade_free(result);
    wasm.plurimath_facade_dealloc(pointer, input.length);
  }
}
function request(value, label, raw = false) {
  const wire = raw ? value : JSON.stringify(value);
  const actual = wasmRequest(wire);
  if (values.native) {
    const native = spawnSync(resolve(values.native), [], {
      input: `${wire}\n`, encoding: 'utf8', timeout: 30_000, maxBuffer: 32 * 1024 * 1024,
    });
    assert.ifError(native.error);
    assert.equal(native.status, 0, `${label}: native process: ${native.stderr}`);
    assert(native.stdout.endsWith('\n'), `${label}: incomplete native response`);
    assert.equal(actual, native.stdout.slice(0, -1), `${label}: native/WASM wire bytes`);
    counts.nativeComparisons++;
  }
  const response = JSON.parse(actual);
  assert.equal(response.abi_version, 1, `${label}: response ABI`);
  assert.equal(typeof response.ok, 'boolean', `${label}: missing status`);
  counts.requests++;
  return response;
}
function parse(row) {
  const reply = request({ op: 'parse', format: row.format, input: row.input }, row.id);
  assert.equal(reply.ok, true, `${row.id}: ${JSON.stringify(reply.error)}`);
  assert(reply.model && typeof reply.model === 'object', `${row.id}: missing model`);
  return reply.model;
}
function write(model, format, label) {
  const reply = request({ op: 'write', format, model }, `${label}/${format}`);
  assert.equal(reply.ok, true, `${label}/${format}: ${JSON.stringify(reply.error)}`);
  assert.equal(typeof reply.output, 'string', `${label}/${format}: missing output`);
  return reply.output;
}
for (const row of oracle.rows) {
  const model = parse(row);
  assert.deepEqual(model, row.model, `${row.id}: frozen Ruby model`);
  counts.rubyModels++;
  for (const format of cases.writers) {
    assert(Object.hasOwn(row.writers, format), `${row.id}/${format}: missing Ruby writer oracle`);
    const expected = row.writers[format];
    assert.equal(write(model, format, row.id), expected, `${row.id}/${format}: Ruby writer bytes`);
    counts.rubyWriters++;
  }
}
for (const row of cases.parse) {
  const model = parse(row);
  for (const format of cases.writers) write(model, format, row.id);
}
for (const row of cases.refusals) {
  const reply = request(row.wire, row.id, true);
  assert.equal(reply.ok, false, `${row.id}: malformed request must be refused`);
  assert.equal(typeof reply.error?.code, 'string', `${row.id}: missing refusal code`);
  counts.refusals++;
}
// Verify the same instance remains usable after malformed requests and frees.
parse(cases.parse[0]);
if (values.native) assert.equal(counts.nativeComparisons, counts.requests);
console.log(JSON.stringify({ runtime: 'Node-WASM',
  nativeDifferential: Boolean(values.native),
  wasmSha256: createHash('sha256').update(bytes).digest('hex'),
  rubySource: oracle.source, ...counts,
}));
