// Renders product screenshots from the in-browser mock (npm run dev) with the
// locally installed Chrome, in light and dark and in the given UI languages.
//
//   npm run dev &                      # Vite on http://localhost:1420
//   node scripts/screenshots.mjs       # -> docs/assets/screenshots/<lang>/<name>.png
//
// The app's dev autopilot (see App.tsx) drives each state and ?theme= forces
// the appearance. "hero" is the whole 1120×720 window at 2×; every other shot
// is a 4:3 close-up of one feature, rendered at 3× so it stays crisp when the
// website zooms in on it.
import puppeteer from "puppeteer-core";
import { mkdirSync } from "node:fs";

const CHROME = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const BASE = process.env.BASE ?? "http://localhost:1420/";
const LANGS = (process.env.LANGS ?? "en,zh-CN").split(",");
const OUT = "docs/assets/screenshots";
const VIEW = { width: 1120, height: 720 };

const thumbsLoaded = () => {
  const imgs = [...document.querySelectorAll(".content img")];
  return imgs.length >= 10 && imgs.every((i) => i.complete && i.naturalWidth > 0);
};
const previewLoaded = () => {
  const i = document.querySelector(".preview-panel img");
  return !!i && i.complete && i.naturalWidth > 0;
};

// Each `clip` runs in the page and returns the rectangle (CSS px) to keep.
const SHOTS = [
  { name: "grid", q: "share=photo,open=2025/Kyoto,grid", ready: thumbsLoaded, full: true },
  { name: "sidebar", q: "share=home", ready: () => document.body.innerText.includes("Documents"),
    clip: () => { const r = document.querySelector(".server-item").getBoundingClientRect(); return { x: r.x, y: r.y, right: r.right, bottom: r.bottom }; }, pad: 14 },
  { name: "thumbs", q: "share=photo,open=2025/Kyoto,grid", ready: thumbsLoaded,
    clip: () => { const c = [...document.querySelectorAll("[data-index]")]; const pick = [c[0], c[2], c[6], c[8]].filter(Boolean); return pick.reduce((a, el) => { const r = el.getBoundingClientRect(); return a ? { x: Math.min(a.x, r.x), y: Math.min(a.y, r.y), right: Math.max(a.right, r.right), bottom: Math.max(a.bottom, r.bottom) } : { x: r.x, y: r.y, right: r.right, bottom: r.bottom }; }, null); }, pad: 10 },
  { name: "quicklook", q: "share=photo,open=2025/Kyoto,grid,preview=DSC_0005.jpg", ready: previewLoaded,
    clip: () => { const r = document.querySelector(".preview-panel").getBoundingClientRect(); return { x: r.x, y: r.y, right: r.x + 560, bottom: r.y + 420 }; }, pad: 0 },
  { name: "transfers", q: "share=home,open=Downloads,download=ubuntu-24.04.iso,transfers", settle: 1500,
    ready: () => { const p = document.querySelector(".transfers-pop"); return !!p && /MB/.test(p.innerText); },
    clip: () => { const r = document.querySelector(".transfers-pop").getBoundingClientRect(); return { x: r.x, y: r.y, right: r.right, bottom: r.bottom }; }, pad: 18 },
  { name: "discover", q: "share=home,add", ready: () => document.body.innerText.includes("Office iMac"),
    clip: () => { const r = document.querySelector(".discovered").getBoundingClientRect(); return { x: r.x, y: r.y - 44, right: r.right, bottom: r.bottom }; }, pad: 14 },
  { name: "settings", q: "share=home,settings", settle: 400, ready: () => !!document.querySelector(".modal select"),
    clip: () => { const f = document.querySelectorAll(".modal .field"); const a = f[2].getBoundingClientRect(), b = f[3].getBoundingClientRect(); return { x: Math.min(a.x, b.x), y: a.y, right: Math.max(a.right, b.right), bottom: b.bottom }; }, pad: 14 },
];

/** Grows `r` (padded) to a 4:3 box around its centre, kept inside the viewport. */
function box43(r, pad) {
  const x0 = r.x - pad, y0 = r.y - pad, x1 = (r.right ?? r.x + r.width) + pad, y1 = (r.bottom ?? r.y + r.height) + pad;
  let w = x1 - x0, h = y1 - y0;
  if (w / h > 4 / 3) h = (w * 3) / 4; else w = (h * 4) / 3;
  w = Math.min(w, VIEW.width); h = Math.min(h, VIEW.height);
  let x = (x0 + x1) / 2 - w / 2, y = (y0 + y1) / 2 - h / 2;
  x = Math.max(0, Math.min(x, VIEW.width - w)); y = Math.max(0, Math.min(y, VIEW.height - h));
  return { x: Math.round(x), y: Math.round(y), width: Math.round(w), height: Math.round(h) };
}

const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ["--hide-scrollbars"] });
try {
  for (const lang of LANGS) {
    mkdirSync(`${OUT}/${lang}`, { recursive: true });
    for (const shot of SHOTS) {
      for (const theme of ["dark", "light"]) {
        const context = await browser.createBrowserContext();
        const page = await context.newPage();
        await page.setViewport({ ...VIEW, deviceScaleFactor: shot.full ? 2 : 3 });
        await page.evaluateOnNewDocument((l) => localStorage.setItem("neatnas.locale", l), lang);
        await page.goto(`${BASE}?autopilot=${encodeURIComponent(shot.q)}&theme=${theme}`, { waitUntil: "networkidle0" });
        try {
          await page.waitForFunction(shot.ready, { timeout: 20000, polling: 200 });
        } catch {
          console.warn(`  ${lang}/${shot.name}-${theme}: ready condition timed out, capturing anyway`);
        }
        await new Promise((r) => setTimeout(r, shot.settle ?? 700));
        const file = `${OUT}/${lang}/${shot.name}-${theme}.png`;
        if (shot.full) await page.screenshot({ path: file });
        else {
          const rect = await page.evaluate(shot.clip);
          await page.screenshot({ path: file, clip: box43(rect, shot.pad ?? 12) });
        }
        console.log(`  ${file}`);
        await context.close();
      }
    }
  }
} finally {
  await browser.close();
}
