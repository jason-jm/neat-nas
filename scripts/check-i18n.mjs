// Verifies that every locale in src/locales has exactly the keys of en.json,
// no empty strings, the same {placeholders} per key, and is registered in
// src/locales/index.ts. Runs as the first step of `npm run build`.
import { readdirSync, readFileSync } from "node:fs";

const dir = new URL("../src/locales/", import.meta.url);
const files = readdirSync(dir).filter((f) => f.endsWith(".json")).sort();
const load = (f) => JSON.parse(readFileSync(new URL(f, dir), "utf8"));
const placeholders = (s) => (s.match(/\{[a-zA-Z]+\}/g) ?? []).sort().join(" ");
const en = load("en.json");
const index = readFileSync(new URL("index.ts", dir), "utf8");
let problems = 0;
const report = (msg) => {
  console.error(msg);
  problems++;
};

for (const f of files) {
  if (!index.includes(`"./${f}"`)) report(`${f}: not imported in src/locales/index.ts`);
  if (f === "en.json") continue;
  const d = load(f);
  for (const k of Object.keys(en)) if (!(k in d)) report(`${f}: missing "${k}"`);
  for (const [k, v] of Object.entries(d)) {
    if (!(k in en)) report(`${f}: unknown key "${k}"`);
    else if (typeof v !== "string" || !v.trim()) report(`${f}: empty "${k}"`);
    else if (placeholders(v) !== placeholders(en[k]))
      report(`${f}: placeholders differ in "${k}": [${placeholders(v)}] vs [${placeholders(en[k])}]`);
  }
}

if (problems) {
  console.error(`check-i18n: ${problems} problem(s)`);
  process.exit(1);
}
console.log(`check-i18n: ${files.length} locales, ${Object.keys(en).length} keys, all consistent`);
