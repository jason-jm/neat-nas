import { isTauri } from "./api";

export type ThemePref = "auto" | "light" | "dark";

const THEME_KEY = "neatnas.theme";

export function readTheme(): ThemePref {
  try {
    const s = localStorage.getItem(THEME_KEY);
    if (s === "light" || s === "dark") return s;
  } catch {
    /* storage unavailable */
  }
  return "auto";
}

const darkQuery = typeof window !== "undefined" && "matchMedia" in window ? window.matchMedia("(prefers-color-scheme: dark)") : null;
let themePref: ThemePref = "auto";

function paintTheme() {
  const dark = themePref === "dark" || (themePref === "auto" && !!darkQuery?.matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

darkQuery?.addEventListener("change", () => {
  if (themePref === "auto") paintTheme();
});

/**
 * Applies a theme preference to the page (via `data-theme` on <html>, see
 * styles.css) and, inside Tauri, to the native window so dialogs and the
 * window chrome follow. `index.html` paints the stored choice before the
 * bundle loads so there is no flash on start-up.
 */
export function applyTheme(pref: ThemePref) {
  themePref = pref;
  paintTheme();
  try {
    if (pref === "auto") localStorage.removeItem(THEME_KEY);
    else localStorage.setItem(THEME_KEY, pref);
  } catch {
    /* storage unavailable */
  }
  if (isTauri) {
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(pref === "auto" ? null : pref))
      .catch(() => undefined);
  }
}
