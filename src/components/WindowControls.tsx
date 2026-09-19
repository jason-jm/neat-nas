import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { isTauri } from "../api";

/** Minimize / maximize / close for the frameless window on Windows and Linux. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const win = getCurrentWindow();
      setMaximized(await win.isMaximized().catch(() => false));
      const off = await win.onResized(async () => setMaximized(await win.isMaximized().catch(() => false)));
      if (disposed) off();
      else unlisten = off;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const act = async (what: "minimize" | "toggle" | "close") => {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    if (what === "minimize") await win.minimize();
    else if (what === "toggle") await win.toggleMaximize();
    else await win.close();
  };

  return (
    <div className="win-controls">
      <button type="button" className="win-btn" onClick={() => act("minimize")} aria-label="Minimize">
        <Minus size={14} />
      </button>
      <button type="button" className="win-btn" onClick={() => act("toggle")} aria-label={maximized ? "Restore" : "Maximize"}>
        {maximized ? <Copy size={12} style={{ transform: "scaleX(-1)" }} /> : <Square size={12} />}
      </button>
      <button type="button" className="win-btn close" onClick={() => act("close")} aria-label="Close">
        <X size={15} />
      </button>
    </div>
  );
}
