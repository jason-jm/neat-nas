import { useSyncExternalStore } from "react";
import { dicts, isLocale, matchLocale, type Locale } from "./locales";

export { LOCALES, type Locale } from "./locales";

const STORAGE_KEY = "neatnas.locale";

export type LocalePref = Locale | "system";

function systemLocale(): Locale {
  const langs = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || "en"];
  return matchLocale(langs);
}

function readPref(): LocalePref {
  try {
    const s = localStorage.getItem(STORAGE_KEY);
    if (isLocale(s)) return s;
  } catch {
    /* storage unavailable */
  }
  return "system";
}

let pref: LocalePref = readPref();
let current: Locale = pref === "system" ? systemLocale() : pref;
const listeners = new Set<() => void>();

export function setLocalePref(next: LocalePref) {
  pref = next;
  current = next === "system" ? systemLocale() : next;
  try {
    if (next === "system") localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, next);
  } catch {
    /* storage unavailable */
  }
  listeners.forEach((fn) => fn());
}

export function getLocalePref(): LocalePref {
  return pref;
}

export function translate(key: string, vars?: Record<string, string | number>): string {
  let s = dicts[current][key] ?? dicts.en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

/** Friendly text for a backend error code, falling back to the raw message. */
export function errorText(code: string, fallback: string): string {
  const key = `error.${code}`;
  return dicts[current][key] ?? dicts.en[key] ?? fallback ?? translate("error.unknown");
}

function subscribe(fn: () => void) {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

export function useI18n() {
  const locale = useSyncExternalStore(subscribe, () => current, () => current);
  return { locale, t: translate, errorText, setLocalePref, localePref: pref };
}
