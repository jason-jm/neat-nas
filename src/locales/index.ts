import en from "./en.json";
import zhCN from "./zh-CN.json";
import zhTW from "./zh-TW.json";
import ja from "./ja.json";
import ko from "./ko.json";
import de from "./de.json";
import es from "./es.json";
import fr from "./fr.json";
import ptBR from "./pt-BR.json";
import ru from "./ru.json";

export type Dict = Record<string, string>;

/**
 * Supported UI languages, in the order the language picker lists them.
 * To add one: drop `<code>.json` next to this file (same keys as en.json,
 * `scripts/check-i18n.mjs` verifies that), import it here and add a row.
 */
export const LOCALES = [
  { code: "en", name: "English" },
  { code: "zh-CN", name: "简体中文" },
  { code: "zh-TW", name: "繁體中文" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "de", name: "Deutsch" },
  { code: "es", name: "Español" },
  { code: "fr", name: "Français" },
  { code: "pt-BR", name: "Português (Brasil)" },
  { code: "ru", name: "Русский" },
] as const;

export type Locale = (typeof LOCALES)[number]["code"];

export const dicts: Record<Locale, Dict> = {
  en,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  ja,
  ko,
  de,
  es,
  fr,
  "pt-BR": ptBR,
  ru,
};

export function isLocale(s: string | null | undefined): s is Locale {
  return LOCALES.some((l) => l.code === s);
}

/**
 * Best supported locale for a list of BCP 47 tags, most preferred first
 * (as `navigator.languages` reports them). Chinese picks Traditional for
 * Hant/TW/HK/MO tags and Simplified otherwise; any Portuguese maps to the
 * Brazilian dictionary; other languages match on the base language.
 */
export function matchLocale(preferred: readonly string[]): Locale {
  for (const raw of preferred) {
    const tag = raw.trim().toLowerCase();
    if (!tag) continue;
    const parts = tag.split(/[-_]/);
    if (parts[0] === "zh") {
      return parts.some((p) => p === "hant" || p === "tw" || p === "hk" || p === "mo") ? "zh-TW" : "zh-CN";
    }
    if (parts[0] === "pt") return "pt-BR";
    const hit = LOCALES.find((l) => l.code.toLowerCase() === parts[0]);
    if (hit) return hit.code;
  }
  return "en";
}
