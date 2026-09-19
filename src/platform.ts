export type Platform = "macos" | "windows" | "linux";

/** Best-effort platform from the user agent; `?platform=windows` overrides it for UI previews. */
export const platform: Platform = (() => {
  try {
    const forced = new URLSearchParams(window.location.search).get("platform");
    if (forced === "macos" || forced === "windows" || forced === "linux") return forced;
  } catch {
    /* no window */
  }
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (/Windows/i.test(ua)) return "windows";
  if (/Mac/i.test(ua)) return "macos";
  return "linux";
})();

export const isMac = platform === "macos";
export const isWindows = platform === "windows";
