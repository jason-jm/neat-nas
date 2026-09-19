// In-browser stand-in for the Rust backend, used when the UI is opened in a
// plain browser (design work, screenshots, UI tests). Never bundled into the
// desktop app's code path.

import type {
  Backend,
  ConnectionInfo,
  DiscoveredServer,
  DownloadItem,
  Entry,
  FileDropEvent,
  SavedServer,
  ServerInput,
  Settings,
  Share,
  TransferProgress,
} from "./types";

const SERVERS_KEY = "neatnas.mock.servers";
const DIR_KEY = "neatnas.mock.downloadDir";
const DEFAULT_DIR = "/Users/you/Downloads";

function loadServers(): SavedServer[] {
  try {
    const raw = localStorage.getItem(SERVERS_KEY);
    if (raw) return JSON.parse(raw) as SavedServer[];
  } catch {
    /* ignore */
  }
  return [
    {
      id: "demo",
      name: "DiskStation",
      host: "diskstation.local",
      port: 445,
      username: "alice",
      domain: "",
      lastShare: null,
      addedAt: Date.now() - 86400000 * 12,
    },
  ];
}
function persist(servers: SavedServer[]) {
  try {
    localStorage.setItem(SERVERS_KEY, JSON.stringify(servers));
  } catch {
    /* ignore */
  }
}
let servers = loadServers();

const shares: Share[] = [
  { name: "home", comment: "Personal files" },
  { name: "photo", comment: "Photo library" },
  { name: "video", comment: "Movies and TV" },
  { name: "music", comment: "" },
  { name: "backup", comment: "Time Machine and PC backups" },
];

type Spec = { [name: string]: Spec | number };
const tree: Record<string, Spec> = {
  home: {
    Documents: {
      "Tax 2025.pdf": 1_240_000,
      "Resume.docx": 88_000,
      Projects: { "neatnas-notes.md": 4_200, "budget.xlsx": 210_000, "app.tsx": 9_800 },
    },
    Downloads: { "ubuntu-24.04.iso": 5_900_000_000, "setup.dmg": 180_000_000 },
    Archive: Object.fromEntries(Array.from({ length: 60 }, (_, i) => [`backup-part-${String(i + 1).padStart(2, "0")}.bin`, 1_000_000 * (i + 1)])),
    "读书笔记.md": 12_000,
    "IMG_4021.HEIC": 3_400_000,
    "empty.txt": 0,
  },
  photo: {
    "2024": {
      Tokyo: { "DSC_0001.jpg": 4_100_000, "DSC_0002.jpg": 3_900_000, "DSC_0003.jpg": 4_400_000 },
      Home: { "birthday.mov": 320_000_000 },
    },
    "2025": { "Kyoto.zip": 1_200_000_000 },
  },
  video: {
    Movies: { "Interstellar (2014).mkv": 14_000_000_000 },
    TV: { "Severance S02E01.mkv": 2_100_000_000 },
  },
  music: { "Daft Punk - Discovery": { "01 One More Time.flac": 32_000_000 }, "podcast.mp3": 55_000_000 },
  backup: { "MacBook.sparsebundle": {}, "PC-Backup-2025-09.zip": 40_000_000_000 },
};

const DAY = 86_400_000;
const NOW = Date.now();

function listAt(share: string, path: string): Entry[] {
  let node: Spec | number | undefined = tree[share];
  if (node === undefined) throw { code: "not_found", message: `Share ${share} not found` };
  for (const seg of path.split("/").filter(Boolean)) {
    if (typeof node === "number") throw { code: "not_a_directory", message: path };
    node = node[seg];
    if (node === undefined) throw { code: "not_found", message: path };
  }
  if (typeof node === "number") throw { code: "not_a_directory", message: path };
  return Object.entries(node)
    .map(([name, v], i) => ({
      name,
      path: path ? `${path}/${name}` : name,
      isDir: typeof v !== "number",
      size: typeof v === "number" ? v : 0,
      modified: NOW - Math.round((i * 3 + name.length) * DAY * 0.7),
      created: NOW - 30 * DAY,
    }))
    .sort((a, b) => Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name));
}

function sizeOf(share: string, path: string): number {
  let node: Spec | number | undefined = tree[share];
  for (const seg of path.split("/").filter(Boolean)) {
    if (typeof node !== "object") return 0;
    node = node[seg];
  }
  const walk = (n: Spec | number | undefined): number =>
    typeof n === "number" ? n : n ? Object.values(n).reduce<number>((a, c) => a + walk(c), 0) : 0;
  return walk(node);
}

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

const subscribers = new Set<(p: TransferProgress) => void>();
const registry = new Map<string, TransferProgress>();
const emit = (p: TransferProgress) => {
  registry.set(p.taskId, { ...p });
  subscribers.forEach((fn) => fn({ ...p }));
};
const cancelled = new Set<string>();

function newTransfer(kind: "download" | "upload", serverId: string, share: string, name: string, isDir: boolean, remotePath: string, localPath: string, destPath: string, bytesTotal: number): TransferProgress {
  return {
    taskId: `t-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    kind,
    serverId,
    share,
    name,
    isDir,
    remotePath,
    localPath,
    destPath,
    status: "queued",
    bytesDone: 0,
    bytesTotal,
    filesDone: 0,
    filesTotal: isDir ? 4 : 1,
    currentFile: remotePath,
    error: null,
    errorCode: null,
    startedAt: Date.now(),
    updatedAt: Date.now(),
    speedBps: 0,
    attempt: 1,
  };
}

function svgDataUrl(label: string, hue: number, w = 320, h = 240): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue},70%,55%)"/><stop offset="1" stop-color="hsl(${(hue + 60) % 360},70%,35%)"/></linearGradient></defs><rect width="${w}" height="${h}" fill="url(#g)"/><circle cx="${w * 0.7}" cy="${h * 0.35}" r="${h * 0.12}" fill="rgba(255,255,255,0.7)"/><text x="12" y="${h - 14}" font-family="sans-serif" font-size="14" fill="rgba(255,255,255,0.85)">${label.replace(/[<&>]/g, "")}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

const hueOf = (s: string) => [...s].reduce((a, c) => (a * 31 + c.charCodeAt(0)) % 360, 7);

function simulate(p: TransferProgress) {
  const total = Math.min(p.bytesTotal, 600_000_000);
  p.bytesTotal = total;
  let elapsed = 0;
  const speed = 48_000_000; // bytes per second
  const tick = 100;
  const timer = setInterval(() => {
    if (cancelled.has(p.taskId)) {
      cancelled.delete(p.taskId);
      clearInterval(timer);
      emit({ ...p, status: "cancelled", speedBps: 0 });
      return;
    }
    if (p.name.includes("flaky") && p.bytesDone > total / 3 && p.attempt < 2) {
      clearInterval(timer);
      emit({ ...p, status: "interrupted", speedBps: 0, error: "connection lost", errorCode: "connection_lost" });
      return;
    }
    elapsed += tick;
    if (elapsed < 400) {
      emit({ ...p, status: "scanning" });
      return;
    }
    p.status = "running";
    p.bytesDone = Math.min(total, p.bytesDone + (speed * tick) / 1000);
    p.speedBps = speed;
    p.filesDone = Math.floor((p.bytesDone / Math.max(total, 1)) * p.filesTotal);
    if (p.bytesDone >= total) {
      clearInterval(timer);
      emit({ ...p, status: "done", filesDone: p.filesTotal });
      return;
    }
    emit(p);
  }, tick);
}

export const mock: Backend = {
  async listServers() {
    await delay(60);
    return servers.slice();
  },
  async testConnection(input: ServerInput): Promise<ConnectionInfo> {
    await delay(700);
    if (input.password === "wrong") throw { code: "auth_failed", message: "STATUS_LOGON_FAILURE" };
    if (input.host.includes("timeout")) throw { code: "timed_out", message: "connect timed out" };
    return { dialect: "SMB 3.1.1", shares, sharesError: null };
  },
  async addServer(input) {
    await mock.testConnection(input);
    const s: SavedServer = {
      id: `srv-${Date.now()}`,
      name: input.name?.trim() || input.host,
      host: input.host,
      port: input.port ?? 445,
      username: input.username,
      domain: input.domain ?? "",
      lastShare: null,
      addedAt: Date.now(),
    };
    servers = [...servers, s];
    persist(servers);
    return s;
  },
  async updateServer(id, input) {
    await delay(400);
    const old = servers.find((s) => s.id === id);
    if (!old) throw { code: "not_found", message: "Server not found" };
    const s: SavedServer = { ...old, name: input.name?.trim() || input.host, host: input.host, port: input.port ?? 445, username: input.username, domain: input.domain ?? "" };
    servers = servers.map((x) => (x.id === id ? s : x));
    persist(servers);
    return s;
  },
  async removeServer(id) {
    servers = servers.filter((s) => s.id !== id);
    persist(servers);
  },
  async disconnectServer() {},
  async rememberShare(serverId, share) {
    servers = servers.map((s) => (s.id === serverId ? { ...s, lastShare: share } : s));
    persist(servers);
  },
  async listShares(serverId): Promise<ConnectionInfo> {
    await delay(500);
    const s = servers.find((x) => x.id === serverId);
    if (!s) throw { code: "not_found", message: "Server not found" };
    if (s.host.includes("timeout")) throw { code: "timed_out", message: "connect timed out" };
    if (s.host.includes("noshares")) return { dialect: "SMB 3.0.2", shares: [], sharesError: "STATUS_ACCESS_DENIED" };
    return { dialect: "SMB 3.1.1", shares, sharesError: null };
  },
  async listDir(_serverId, share, path) {
    await delay(180);
    return listAt(share, path);
  },
  async startDownload(serverId, share, items: DownloadItem[], destDir) {
    await delay(80);
    const dir = destDir ?? localStorage.getItem(DIR_KEY) ?? DEFAULT_DIR;
    return items.map((item) => {
      const p = newTransfer("download", serverId, share, item.name, item.isDir, item.path, `${dir}/${item.name}`, `${dir}/${item.name}`, Math.max(sizeOf(share, item.path), 1));
      registry.set(p.taskId, { ...p });
      setTimeout(() => simulate(p), 50);
      return { ...p };
    });
  },
  async startUpload(serverId, share, destDir, paths) {
    await delay(80);
    return paths.map((local) => {
      const name = local.split(/[\\/]/).pop() ?? local;
      const isDir = !name.includes(".");
      const remote = destDir ? `${destDir}/${name}` : name;
      const p = newTransfer("upload", serverId, share, name, isDir, remote, local, destDir, 90_000_000 + (name.length % 5) * 40_000_000);
      registry.set(p.taskId, { ...p });
      setTimeout(() => simulate(p), 50);
      return { ...p };
    });
  },
  async cancelTransfer(taskId) {
    cancelled.add(taskId);
    return true;
  },
  async resumeTransfer(taskId) {
    const p = registry.get(taskId);
    if (!p) throw { code: "not_found", message: "Transfer not found" };
    const next: TransferProgress = { ...p, status: "queued", attempt: p.attempt + 1, error: null, errorCode: null };
    registry.set(taskId, next);
    setTimeout(() => simulate(next), 50);
    return { ...next };
  },
  async removeTransfer(taskId) {
    registry.delete(taskId);
  },
  async listTransfers() {
    return [...registry.values()].reverse();
  },
  async clearFinishedTransfers() {
    for (const [id, p] of registry) if (p.status === "done") registry.delete(id);
  },
  async thumbnail(_serverId, _share, path, size) {
    await delay(150 + (size % 400));
    const name = path.split("/").pop() ?? path;
    if (!/\.(jpe?g|png|gif|webp|heic|bmp|tiff?)$/i.test(name)) throw { code: "no_thumbnail", message: "not an image" };
    return svgDataUrl(name, hueOf(name));
  },
  async startDragOut(_serverId, _share, items) {
    console.info("[mock] drag out", items.map((i) => i.path));
  },
  previewUrl(_serverId, _share, path) {
    const name = path.split("/").pop() ?? path;
    if (/\.(jpe?g|png|gif|webp|heic|bmp|tiff?)$/i.test(name)) return svgDataUrl(name, hueOf(name), 1600, 1000);
    if (/\.(txt|md|csv|json|tsx?|rs|log)$/i.test(name)) return `data:text/plain;charset=utf-8,${encodeURIComponent(`# ${name}\n\nThis is mock text content for the preview.\n\n` + "line\n".repeat(60))}`;
    return "";
  },
  async pickFiles(directory) {
    const answer = window.prompt(directory ? "Folder paths (mock, comma separated)" : "File paths (mock, comma separated)", directory ? "/Users/you/Pictures/Trip" : "/Users/you/Desktop/report.pdf, /Users/you/Desktop/flaky-video.mp4");
    return answer ? answer.split(",").map((s) => s.trim()).filter(Boolean) : null;
  },
  async onFileDrop(cb: (e: FileDropEvent) => void) {
    // In a browser, HTML5 drag events stand in for the OS drop events.
    const enter = (e: DragEvent) => { e.preventDefault(); cb({ type: "enter", paths: [] }); };
    const over = (e: DragEvent) => { e.preventDefault(); cb({ type: "over" }); };
    const leave = () => cb({ type: "leave" });
    const drop = (e: DragEvent) => { e.preventDefault(); cb({ type: "drop", paths: [...(e.dataTransfer?.files ?? [])].map((f) => `/Users/you/Desktop/${f.name}`) }); };
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragover", over);
    window.addEventListener("dragleave", leave);
    window.addEventListener("drop", drop);
    return () => {
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragover", over);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("drop", drop);
    };
  },
  async discoverServers(timeoutMs = 2000): Promise<DiscoveredServer[]> {
    await delay(Math.min(timeoutMs, 1400));
    return [
      { name: "DiskStation", hostname: "DiskStation.local", addresses: ["192.168.50.10"], port: 445 },
      { name: "Office iMac", hostname: "office-imac.local", addresses: ["192.168.50.23"], port: 445 },
    ];
  },
  async getSettings(): Promise<Settings> {
    const custom = localStorage.getItem(DIR_KEY);
    return {
      downloadDir: custom ?? DEFAULT_DIR,
      downloadDirIsCustom: !!custom,
      configPath: "~/Library/Application Support/com.neatnas.app/config.json",
      platform: "macos",
      devAutopilot: new URLSearchParams(window.location.search).get("autopilot"),
    };
  },
  async setDownloadDir(path) {
    if (path) localStorage.setItem(DIR_KEY, path);
    else localStorage.removeItem(DIR_KEY);
  },
  async onTransferProgress(cb) {
    subscribers.add(cb);
    return () => subscribers.delete(cb);
  },
  async pickFolder(defaultPath) {
    return window.prompt("Folder path (mock)", defaultPath ?? DEFAULT_DIR);
  },
  async revealItem(path) {
    console.info("[mock] reveal", path);
  },
  async appVersion() {
    return "1.0.0";
  },
  async checkForUpdate() {
    await delay(900);
    if (!new URLSearchParams(window.location.search).has("update")) return null;
    return { version: "1.1.0", currentVersion: "1.0.0", notes: "- Faster thumbnails\n- Fixes a crash when a share goes offline", date: "2026-10-01" };
  },
  async installUpdate(onProgress) {
    const total = 24_000_000;
    for (let done = 0; done < total; done += 3_000_000) {
      onProgress({ phase: "downloading", downloaded: done, total });
      await delay(150);
    }
    onProgress({ phase: "installing" });
    await delay(600);
    onProgress({ phase: "done" });
  },
};
