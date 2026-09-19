import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, Download, FileQuestion, LoaderCircle, X } from "lucide-react";
import { createPortal } from "react-dom";
import type { Entry } from "../types";
import { api } from "../api";
import { useI18n } from "../i18n";
import { fileKind, formatBytes, formatDate } from "../format";
import { Button } from "./ui";

export type PreviewKind = "image" | "video" | "audio" | "pdf" | "text" | "none";

const TEXT_EXT = new Set(["txt", "md", "markdown", "csv", "tsv", "json", "yaml", "yml", "toml", "ini", "cfg", "conf", "log", "xml", "html", "htm", "css", "js", "ts", "tsx", "jsx", "py", "rs", "go", "java", "kt", "swift", "c", "cpp", "h", "sh", "sql", "srt", "vtt", "nfo"]);
const VIDEO_EXT = new Set(["mp4", "m4v", "mov", "webm", "mkv", "ogv"]);
const AUDIO_EXT = new Set(["mp3", "m4a", "aac", "wav", "flac", "ogg", "opus", "aiff"]);
const IMAGE_EXT = new Set(["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "avif", "heic", "heif", "tif", "tiff"]);
const MAX_TEXT = 1024 * 1024;

export function previewKind(entry: Entry): PreviewKind {
  if (entry.isDir) return "none";
  const ext = entry.name.split(".").pop()?.toLowerCase() ?? "";
  if (IMAGE_EXT.has(ext)) return "image";
  if (VIDEO_EXT.has(ext)) return "video";
  if (AUDIO_EXT.has(ext)) return "audio";
  if (ext === "pdf") return "pdf";
  if (TEXT_EXT.has(ext) || (!entry.name.includes(".") && entry.size < 512 * 1024)) return "text";
  return "none";
}

export const isPreviewable = (entry: Entry) => previewKind(entry) !== "none";

/** Legacy code page to assume for text that is not UTF-8, by UI language. */
function legacyEncoding(locale: string): string {
  if (locale === "zh-CN") return "gb18030";
  if (locale === "zh-TW") return "big5";
  if (locale === "ja") return "shift_jis";
  if (locale === "ko") return "euc-kr";
  return "windows-1252";
}

/**
 * Decodes text for display: UTF-16 with a BOM, then strict UTF-8, then the
 * legacy code page for the UI language (NFO files and old notes on a NAS
 * are often GBK/Big5). A truncated read may end mid-character, so a UTF-8
 * failure is retried without the last few bytes before giving up on it.
 */
function decodeText(buffer: ArrayBuffer, truncated: boolean, locale: string): string {
  const bytes = new Uint8Array(buffer);
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) return new TextDecoder("utf-16le").decode(bytes.subarray(2));
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) return new TextDecoder("utf-16be").decode(bytes.subarray(2));
  const utf8 = new TextDecoder("utf-8", { fatal: true });
  try {
    return utf8.decode(bytes);
  } catch {
    /* not valid UTF-8 as a whole */
  }
  if (truncated) {
    try {
      return utf8.decode(bytes.subarray(0, Math.max(0, bytes.length - 3)));
    } catch {
      /* still not UTF-8 */
    }
  }
  try {
    return new TextDecoder(legacyEncoding(locale)).decode(bytes);
  } catch {
    return new TextDecoder("utf-8").decode(bytes);
  }
}

interface Props {
  serverId: string;
  share: string;
  entries: Entry[];
  index: number;
  onIndexChange: (i: number) => void;
  onClose: () => void;
  onDownload: (entry: Entry) => void;
}

/**
 * Quick Look: an opaque panel floating over the browser with margins all
 * round, so the window behind stays visible and the panel never reaches
 * the native window controls. Title, pager, download and close live in
 * the panel's own header; the stage below shows the file.
 */
export function PreviewModal({ serverId, share, entries, index, onIndexChange, onClose, onDownload }: Props) {
  const { t, locale } = useI18n();
  const entry = entries[index];
  const kind = entry ? previewKind(entry) : "none";
  const url = useMemo(() => (entry ? api.previewUrl(serverId, share, entry.path) : ""), [serverId, share, entry]);
  const [text, setText] = useState<{ body: string; truncated: boolean } | null>(null);
  const [textLoading, setTextLoading] = useState(false);
  // Which image URL has finished loading. Deriving the spinner from this
  // (instead of a "loading" flag set in an effect) avoids the race where a
  // tiny image fires `load` before the effect runs.
  const [loadedUrl, setLoadedUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const loading = kind === "text" ? textLoading : kind === "image" ? loadedUrl !== url : false;

  useEffect(() => {
    setFailed(null);
    setText(null);
    if (!entry || kind !== "text" || !url) return;
    let cancelled = false;
    setTextLoading(true);
    const truncated = entry.size > MAX_TEXT;
    fetch(url, { headers: { Range: `bytes=0-${MAX_TEXT - 1}` } })
      .then(async (r) => {
        if (!r.ok && r.status !== 206) throw new Error(`${r.status}`);
        const body = decodeText(await r.arrayBuffer(), truncated, locale);
        if (!cancelled) setText({ body, truncated });
      })
      .catch((e) => !cancelled && setFailed(String(e)))
      .finally(() => !cancelled && setTextLoading(false));
    return () => {
      cancelled = true;
    };
  }, [entry, kind, url, locale]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === " ") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowLeft" && index > 0) onIndexChange(index - 1);
      else if (e.key === "ArrowRight" && index < entries.length - 1) onIndexChange(index + 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, entries.length, onClose, onIndexChange]);

  if (!entry) return null;

  const stage = () => {
    if (failed) {
      return (
        <div className="preview-empty">
          <div className="big-icon">
            <FileQuestion size={32} />
          </div>
          <div>{t("preview.failed")}</div>
          <div className="preview-empty-detail">{failed}</div>
        </div>
      );
    }
    switch (kind) {
      case "image":
        return (
          <>
            {loading && <LoaderCircle className="spin preview-spinner" size={28} />}
            <img
              src={url}
              alt={entry.name}
              onLoad={() => setLoadedUrl(url)}
              onError={() => {
                setLoadedUrl(url);
                setFailed(fileKind(entry.name, false) === "image" ? t("preview.unsupported") : "");
              }}
              style={loading ? { opacity: 0 } : undefined}
            />
          </>
        );
      case "video":
        return <video src={url} controls autoPlay onError={() => setFailed(t("preview.unsupported"))} />;
      case "audio":
        return <audio src={url} controls autoPlay onError={() => setFailed(t("preview.unsupported"))} />;
      case "pdf":
        return <iframe src={url} title={entry.name} />;
      case "text":
        return loading ? (
          <LoaderCircle className="spin preview-spinner" size={28} />
        ) : (
          <>
            <pre>{text?.body ?? ""}</pre>
            {text?.truncated && <div className="preview-note">{t("preview.tooLarge")}</div>}
          </>
        );
      default:
        return (
          <div className="preview-empty">
            <div className="big-icon">
              <FileQuestion size={32} />
            </div>
            <div>{t("preview.unsupported")}</div>
            <Button variant="primary" onClick={() => onDownload(entry)}>
              <Download size={14} />
              {t("browser.download")}
            </Button>
          </div>
        );
    }
  };

  return createPortal(
    <div
      className="preview-scrim"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="preview-panel" role="dialog" aria-label={entry.name}>
        <div className="preview-head">
          <div className="preview-title">
            <span className="name" title={entry.name}>
              {entry.name}
            </span>
            <span className="meta">
              {formatBytes(entry.size)} · {formatDate(entry.modified, locale)}
            </span>
          </div>
          <div className="preview-pager">
            <Button variant="ghost" iconOnly disabled={index === 0} onClick={() => onIndexChange(index - 1)} title="←">
              <ChevronLeft size={16} />
            </Button>
            <span className="count">{t("preview.of", { i: index + 1, n: entries.length })}</span>
            <Button variant="ghost" iconOnly disabled={index >= entries.length - 1} onClick={() => onIndexChange(index + 1)} title="→">
              <ChevronRight size={16} />
            </Button>
          </div>
          <Button onClick={() => onDownload(entry)} title={t("browser.download")}>
            <Download size={14} />
            {t("browser.download")}
          </Button>
          <Button variant="ghost" iconOnly onClick={onClose} title="Esc">
            <X size={16} />
          </Button>
        </div>
        <div className="preview-stage">{stage()}</div>
      </div>
    </div>,
    document.body,
  );
}
