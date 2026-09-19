import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppError,
  Backend,
  ConnectionInfo,
  DiscoveredServer,
  DownloadItem,
  Entry,
  FileDropEvent,
  SavedServer,
  ServerInput,
  Settings,
  TransferProgress,
  UpdateInfo,
  UpdateProgress,
} from "./types";
import { mock } from "./mock";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const isWindows = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

const tauriBackend: Backend = {
  listServers: () => invoke<SavedServer[]>("list_servers"),
  testConnection: (input: ServerInput) => invoke<ConnectionInfo>("test_connection", { input }),
  addServer: (input: ServerInput) => invoke<SavedServer>("add_server", { input }),
  updateServer: (id: string, input: ServerInput) => invoke<SavedServer>("update_server", { id, input }),
  removeServer: (id: string) => invoke<void>("remove_server", { id }),
  disconnectServer: (serverId: string) => invoke<void>("disconnect_server", { serverId }),
  rememberShare: (serverId: string, share: string | null) => invoke<void>("remember_share", { serverId, share }),
  listShares: (serverId: string) => invoke<ConnectionInfo>("list_shares", { serverId }),
  listDir: (serverId: string, share: string, path: string) => invoke<Entry[]>("list_dir", { serverId, share, path }),
  startDownload: (serverId: string, share: string, items: DownloadItem[], destDir?: string | null) =>
    invoke<TransferProgress[]>("start_download", { serverId, share, items, destDir: destDir ?? null }),
  startUpload: (serverId: string, share: string, destDir: string, paths: string[]) =>
    invoke<TransferProgress[]>("start_upload", { serverId, share, destDir, paths }),
  cancelTransfer: (taskId: string) => invoke<boolean>("cancel_transfer", { taskId }),
  resumeTransfer: (taskId: string) => invoke<TransferProgress>("resume_transfer", { taskId }),
  removeTransfer: (taskId: string) => invoke<void>("remove_transfer", { taskId }),
  listTransfers: () => invoke<TransferProgress[]>("list_transfers"),
  clearFinishedTransfers: () => invoke<void>("clear_finished_transfers"),
  thumbnail: (serverId: string, share: string, path: string, size: number, mtime: number) =>
    invoke<string>("thumbnail", { serverId, share, path, size, mtime }),
  startDragOut: (serverId: string, share: string, items: DownloadItem[]) =>
    invoke<void>("start_drag_out", { serverId, share, items }),
  previewUrl: (serverId: string, share: string, path: string) => {
    const encoded = path.split("/").filter(Boolean).map(encodeURIComponent).join("/");
    const tail = `${encodeURIComponent(serverId)}/${encodeURIComponent(share)}/${encoded}`;
    return isWindows ? `http://nasfile.localhost/${tail}` : `nasfile://localhost/${tail}`;
  },
  pickFiles: async (directory: boolean) => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ directory, multiple: true });
    if (!picked) return null;
    return Array.isArray(picked) ? picked : [picked];
  },
  onFileDrop: async (cb: (e: FileDropEvent) => void) => {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    return getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === "enter") cb({ type: "enter", paths: p.paths });
      else if (p.type === "over") cb({ type: "over" });
      else if (p.type === "drop") cb({ type: "drop", paths: p.paths });
      else cb({ type: "leave" });
    });
  },
  discoverServers: (timeoutMs?: number) => invoke<DiscoveredServer[]>("discover_servers", { timeoutMs: timeoutMs ?? null }),
  getSettings: () => invoke<Settings>("get_settings"),
  setDownloadDir: (path: string | null) => invoke<void>("set_download_dir", { path }),
  onTransferProgress: (cb) => listen<TransferProgress>("transfer:progress", (e) => cb(e.payload)),
  pickFolder: async (defaultPath?: string) => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ directory: true, multiple: false, defaultPath });
    return typeof picked === "string" ? picked : null;
  },
  revealItem: async (path: string) => {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  },
  appVersion: async () => {
    const { getVersion } = await import("@tauri-apps/api/app");
    return getVersion();
  },
  checkForUpdate: async (): Promise<UpdateInfo | null> => {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    pendingUpdate = update;
    if (!update) return null;
    return { version: update.version, currentVersion: update.currentVersion, notes: update.body ?? "", date: update.date ?? null };
  },
  installUpdate: async (onProgress: (p: UpdateProgress) => void) => {
    if (!pendingUpdate) throw { code: "no_update", message: "No update has been checked for" };
    let downloaded = 0;
    let total: number | null = null;
    await pendingUpdate.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? null;
        onProgress({ phase: "downloading", downloaded: 0, total });
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        onProgress({ phase: "downloading", downloaded, total });
      } else if (event.event === "Finished") {
        onProgress({ phase: "installing" });
      }
    });
    onProgress({ phase: "done" });
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  },
};

/** The Update handle from the last successful check; needed to install it. */
let pendingUpdate: import("@tauri-apps/plugin-updater").Update | null = null;

export const api: Backend = isTauri ? tauriBackend : mock;

/** Dev aid: write a line into the Rust log (no-op outside Tauri). */
export function logToBackend(message: string) {
  console.info(`[neatnas] ${message}`);
  if (!isTauri) return;
  invoke("frontend_log", { level: "warn", message }).catch(() => undefined);
}

export function errorOf(e: unknown): AppError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    const err = e as { code: unknown; message: unknown };
    return { code: String(err.code), message: String(err.message) };
  }
  if (e instanceof Error) return { code: "unknown", message: e.message };
  return { code: "unknown", message: String(e) };
}
