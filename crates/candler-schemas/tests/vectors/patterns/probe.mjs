// Replays cases.toml under ECMAScript (`RegExp` with the `u` flag, as JSON Schema validators use).
// Run: node probe.mjs. It reads cases.toml with a minimal reader for the two shapes the file uses:
// `[table]`, `key = '<literal>'` and `key = [ "<basic string>", ... ]`. Exits 1 on any disagreement.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const text = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "cases.toml"), "utf8");
const basic = (s) =>
  JSON.parse(s.replace(/\\u([0-9a-fA-F]{4})/g, (_, h) => "\\u" + h)); // TOML and JSON agree on \n \r \t \" \\ \uXXXX here
const tables = {};
let table = null;
let list = null;
for (const raw of text.split("\n")) {
  const line = raw.trim();
  if (line === "" || line.startsWith("#")) continue;
  let m;
  if ((m = line.match(/^\[(\w+)\]$/))) {
    table = tables[m[1]] = {};
  } else if ((m = line.match(/^pattern = '(.*)'$/))) {
    table.pattern = m[1];
  } else if ((m = line.match(/^(accept|refuse) = \[$/))) {
    list = table[m[1]] = [];
  } else if (line === "]") {
    list = null;
  } else if (list && (m = line.match(/^("(?:[^"\\]|\\.)*"),$/))) {
    list.push(basic(m[1]));
  } else {
    throw new Error(`unreadable line: ${line}`);
  }
}
let bad = 0;
for (const [name, t] of Object.entries(tables)) {
  const re = new RegExp(t.pattern, "u");
  for (const [expected, key] of [[true, "accept"], [false, "refuse"]]) {
    for (const s of t[key]) {
      if (re.test(s) !== expected) {
        bad += 1;
        console.log(`MISMATCH ${name} ${key} ${JSON.stringify(s)}`);
      }
    }
  }
  console.log(`node ${process.version}: ${name}: ${t.accept.length} accept, ${t.refuse.length} refuse`);
}
process.exit(bad ? 1 : 0);
