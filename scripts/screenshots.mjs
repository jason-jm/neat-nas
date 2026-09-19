// Renders product screenshots from the in-browser mock (npm run dev) with the
// locally installed Chrome, in light and dark and in the given UI languages.
//
//   npm run dev &                      # Vite on http://localhost:1420
//   node scripts/screenshots.mjs       # -> docs/assets/screenshots/<lang>/<name>.png
//
// The app's dev autopilot (see App.tsx) drives each state; ?theme= forces the
// appearance. Each image is 1120×720 CSS px at 2× (2240×1440).
import puppeteer from "puppeteer-core";
import { mkdirSync } from "node:fs";

const CHROME = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const BASE = process.env.BASE ?? "http://localhost:1420/";
const LANGS = (process.env.LANGS ?? "en,zh-CN").split(",");
const OUT = "docs/assets/screenshots";

const thumbsLoaded = () => {
  const imgs = [...document.querySelectorAll(".content img")];
  return imgs.length >= 10 && imgs.every((i) => i.complete && i.naturalWidth > 0);
};
const SHOTS = [
  { name: "grid-dark", q: "share=photo,open=2025/Kyoto,grid", theme: "dark", ready: thumbsLoaded },
  { name: "grid-light", q: "share=photo,open=2025/Kyoto,grid", theme: "light", ready: thumbsLoaded },
  { name: "list-light", q: "share=home", theme: "light", ready: () => document.body.innerText.includes("Documents") },
  { name: "list-dark", q: "share=home", theme: "dark", ready: () => document.body.innerText.includes("Documents") },
  { name: "quicklook-dark", q: "share=photo,open=2025/Kyoto,grid,preview=DSC_0005.jpg", theme: "dark", ready: () => { const i = document.querySelector(".preview-panel img"); return !!i && i.complete && i.naturalWidth > 0; } },
  { name: "quicklook-light", q: "share=photo,open=2025/Kyoto,grid,preview=DSC_0005.jpg", theme: "light", ready: () => { const i = document.querySelector(".preview-panel img"); return !!i && i.complete && i.naturalWidth > 0; } },
  { name: "transfers-light", q: "share=home,open=Downloads,download=ubuntu-24.04.iso,transfers", theme: "light", ready: () => !!document.querySelector(".transfers-pop") && /MB/.test(document.querySelector(".transfers-pop").innerText), settle: 1500 },
  { name: "transfers-dark", q: "share=home,open=Downloads,download=ubuntu-24.04.iso,transfers", theme: "dark", ready: () => !!document.querySelector(".transfers-pop") && /MB/.test(document.querySelector(".transfers-pop").innerText), settle: 1500 },
  { name: "discover-light", q: "share=home,add", theme: "light", ready: () => document.body.innerText.includes("Office iMac") },
  { name: "discover-dark", q: "share=home,add", theme: "dark", ready: () => document.body.innerText.includes("Office iMac") },
  { name: "settings-dark", q: "share=home,settings", theme: "dark", ready: () => !!document.querySelector(".modal select"), settle: 400 },
  { name: "settings-light", q: "share=home,settings", theme: "light", ready: () => !!document.querySelector(".modal select"), settle: 400 },
];

const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ["--hide-scrollbars", "--force-device-scale-factor=2"] });
try {
  for (const lang of LANGS) {
    mkdirSync(`${OUT}/${lang}`, { recursive: true });
    for (const shot of SHOTS) {
      const context = await browser.createBrowserContext();
      const page = await context.newPage();
      await page.setViewport({ width: 1120, height: 720, deviceScaleFactor: 2 });
      await page.evaluateOnNewDocument((l) => localStorage.setItem("neatnas.locale", l), lang);
      const url = `${BASE}?autopilot=${encodeURIComponent(shot.q)}&theme=${shot.theme}`;
      await page.goto(url, { waitUntil: "networkidle0" });
      try {
        await page.waitForFunction(shot.ready, { timeout: 20000, polling: 200 });
      } catch {
        console.warn(`  ${lang}/${shot.name}: ready condition timed out, capturing anyway`);
      }
      await new Promise((r) => setTimeout(r, shot.settle ?? 700));
      await page.screenshot({ path: `${OUT}/${lang}/${shot.name}.png` });
      console.log(`  ${lang}/${shot.name}.png`);
      await context.close();
    }
  }
} finally {
  await browser.close();
}
