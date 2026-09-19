// Mirrors the DTOs defined in src-tauri/src (serde camelCase).

export interface SavedServer {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  domain: string;
  lastShare: string | null;
  addedAt: number;
}

export interface ServerInput {
  name?: string;
  host: string;
  port?: number;
  username: string;
  password: string;
  domain?: string;
}

export interface Share {
  name: string;
  comment: string;
}

export interface Entry {
  name: string;
  /** Share-relative path, `/` separated, no leading slash. */
  path: string;
  size: number;
  isDir: boolean;
  modified: number | null;
  created: number | null;
}

export interface ConnectionInfo {
  dialect: string;
  shares: Share[];
  sharesError: string | null;
}

export interface DiscoveredServer {
  name: string;
  hostname: string;
  addresses: string[];
  port: number;
}

export type TransferStatus = "queued" | "scanning" | "running" | "done" | "cancelled" | "error" | "interrupted";
export type TransferKind = "download" | "upload";

export interface TransferProgress {
  taskId: string;
  kind: TransferKind;
  serverId: string;
  share: string;
  name: string;
  isDir: boolean;
  /** Download: source on the share. Upload: final destination on the share. */
  remotePath: string;
  /** Download: final local path. Upload: local source path. */
  localPath: string;
  destPath: string;
  status: TransferStatus;
  bytesDone: number;
  bytesTotal: number;
  filesDone: number;
  filesTotal: number;
  currentFile: string;
  error: string | null;
  errorCode: string | null;
  startedAt: number;
  updatedAt: number;
  speedBps: number;
  attempt: number;
}

export interface Settings {
  downloadDir: string;
  downloadDirIsCustom: boolean;
  configPath: string;
  platform: string;
  /** Dev-only scripted steps (see App autopilot). */
  devAutopilot?: string | null;
}

export type FileDropEvent =
  | { type: "enter"; paths: string[] }
  | { type: "over" }
  | { type: "drop"; paths: string[] }
  | { type: "leave" };

export interface AppError {
  code: string;
  message: string;
}

export interface DownloadItem {
  path: string;
  name: string;
  isDir: boolean;
}

export type Unsubscribe = () => void;

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string;
  date: string | null;
}

export type UpdateProgress = { phase: "downloading"; downloaded: number; total: number | null } | { phase: "installing" } | { phase: "done" };

export interface Backend {
  listServers(): Promise<SavedServer[]>;
  testConnection(input: ServerInput): Promise<ConnectionInfo>;
  addServer(input: ServerInput): Promise<SavedServer>;
  updateServer(id: string, input: ServerInput): Promise<SavedServer>;
  removeServer(id: string): Promise<void>;
  disconnectServer(serverId: string): Promise<void>;
  rememberShare(serverId: string, share: string | null): Promise<void>;
  listShares(serverId: string): Promise<ConnectionInfo>;
  listDir(serverId: string, share: string, path: string): Promise<Entry[]>;
  startDownload(serverId: string, share: string, items: DownloadItem[], destDir?: string | null): Promise<TransferProgress[]>;
  startUpload(serverId: string, share: string, destDir: string, paths: string[]): Promise<TransferProgress[]>;
  cancelTransfer(taskId: string): Promise<boolean>;
  resumeTransfer(taskId: string): Promise<TransferProgress>;
  removeTransfer(taskId: string): Promise<void>;
  listTransfers(): Promise<TransferProgress[]>;
  clearFinishedTransfers(): Promise<void>;
  /** Returns a data: URL, or rejects with code `no_thumbnail` / `too_large`. */
  thumbnail(serverId: string, share: string, path: string, size: number, mtime: number): Promise<string>;
  /** Native drag of the given items to Finder/Explorer. Rejects with `unsupported` where unavailable. */
  startDragOut(serverId: string, share: string, items: DownloadItem[]): Promise<void>;
  /** URL that streams the file straight from the share (with range support). */
  previewUrl(serverId: string, share: string, path: string): string;
  pickFiles(directory: boolean): Promise<string[] | null>;
  onFileDrop(cb: (e: FileDropEvent) => void): Promise<Unsubscribe>;
  discoverServers(timeoutMs?: number): Promise<DiscoveredServer[]>;
  getSettings(): Promise<Settings>;
  setDownloadDir(path: string | null): Promise<void>;
  onTransferProgress(cb: (p: TransferProgress) => void): Promise<Unsubscribe>;
  pickFolder(defaultPath?: string): Promise<string | null>;
  appVersion(): Promise<string>;
  /** Resolves with update metadata, or null when already on the latest version. */
  checkForUpdate(): Promise<UpdateInfo | null>;
  /** Downloads and installs the update found by the last check, then relaunches. */
  installUpdate(onProgress: (p: UpdateProgress) => void): Promise<void>;
  revealItem(path: string): Promise<void>;
}
