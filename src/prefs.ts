import { isTauri } from "./api";

export type ThemePref = "auto" | "light" | "dark";
/** What a double-click (or Enter) does with a file; every file type behaves the same. */
export type DoubleClickAction = "preview" | "download";

const THEME_KEY = "neatnas.theme";
const DOUBLE_CLICK_KEY = "neatnas.doubleClick";

export function readDoubleClick(): DoubleClickAction {
  try {
    if (localStorage.getItem(DOUBLE_CLICK_KEY) === "download") return "download";
  } catch {
    /* storage unavailable */
  }
  return "preview";
}

export function writeDoubleClick(action: DoubleClickAction) {
  try {
    if (action === "preview") localStorage.removeItem(DOUBLE_CLICK_KEY);
    else localStorage.setItem(DOUBLE_CLICK_KEY, action);
  } catch {
    /* storage unavailable */
  }
}

export function readTheme(): ThemePref {
  try {
    // `?theme=dark|light` forces a theme for screenshots and dev runs.
    const q = new URLSearchParams(window.location.search).get("theme");
    if (q === "light" || q === "dark") return q;
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
