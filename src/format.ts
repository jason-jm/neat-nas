export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "—";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function formatSpeed(bps: number): string {
  return `${formatBytes(bps)}/s`;
}

export function formatEta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  if (seconds < 60) return `${Math.ceil(seconds)}s`;
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  if (m < 60) return `${m}m ${s}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

export function formatDate(ms: number | null, locale: string): string {
  if (!ms) return "—";
  const d = new Date(ms);
  const now = new Date();
  const sameYear = d.getFullYear() === now.getFullYear();
  return new Intl.DateTimeFormat(locale, {
    year: sameYear ? undefined : "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(d);
}

export type FileKind = "folder" | "image" | "video" | "audio" | "archive" | "doc" | "code" | "sheet" | "file";

const KINDS: Record<string, FileKind> = {};
const add = (kind: FileKind, exts: string) => exts.split(" ").forEach((e) => (KINDS[e] = kind));
add("image", "jpg jpeg png gif webp heic heif bmp tiff tif svg raw cr2 nef arw dng psd");
add("video", "mp4 mkv mov avi wmv m4v webm flv ts m2ts mpg mpeg");
add("audio", "mp3 flac aac m4a wav ogg opus wma aiff alac");
add("archive", "zip rar 7z tar gz bz2 xz dmg iso pkg");
add("doc", "pdf doc docx txt md rtf pages odt epub ppt pptx key");
add("sheet", "xls xlsx csv numbers ods");
add("code", "js ts tsx jsx json yaml yml toml py rs go java kt swift c cpp h html css sh");

export function fileKind(name: string, isDir: boolean): FileKind {
  if (isDir) return "folder";
  const i = name.lastIndexOf(".");
  if (i < 0) return "file";
  return KINDS[name.slice(i + 1).toLowerCase()] ?? "file";
}

export function parentPath(p: string): string {
  const i = p.lastIndexOf("/");
  return i < 0 ? "" : p.slice(0, i);
}

export function baseName(p: string): string {
  const i = p.lastIndexOf("/");
  return i < 0 ? p : p.slice(i + 1);
}
