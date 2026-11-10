import fs from "node:fs";
import {numericKernel} from "../bindings/js/kernel.js";
const kernel = await numericKernel(fs.readFileSync(new URL("../target/wasm32-unknown-unknown/debug/plurimath_numeric.wasm", import.meta.url)));
const fixture = JSON.parse(fs.readFileSync(process.argv[2] ?? new URL("../fixtures/numeric-v1.json", import.meta.url), "utf8"));
const failures = [];
for (const row of fixture.cases) {
  const actual = kernel.evaluateJson(row.request_json ?? JSON.stringify(row.request));
  if (actual !== row.expected_wire) failures.push({id: row.id, actual, expected: row.expected_wire});
}
console.log(JSON.stringify({runtime: "Node-WASM", executed: fixture.cases.length, passed: fixture.cases.length-failures.length, failures}));
if (failures.length) process.exitCode=1;
