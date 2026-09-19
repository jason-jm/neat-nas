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

interface Props {
  serverId: string;
  share: string;
  entries: Entry[];
  index: number;
  onIndexChange: (i: number) => void;
  onClose: () => void;
  onDownload: (entry: Entry) => void;
}

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
    fetch(url, { headers: { Range: `bytes=0-${MAX_TEXT - 1}` } })
      .then(async (r) => {
        if (!r.ok && r.status !== 206) throw new Error(`${r.status}`);
        const body = await r.text();
        if (!cancelled) setText({ body, truncated: entry.size > MAX_TEXT });
      })
      .catch((e) => !cancelled && setFailed(String(e)))
      .finally(() => !cancelled && setTextLoading(false));
    return () => {
      cancelled = true;
    };
  }, [entry, kind, url]);

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

  const body = () => {
    if (failed) {
      return (
        <div className="preview-empty">
          <div className="big-icon">
            <FileQuestion size={32} />
          </div>
          <div>{t("preview.failed")}</div>
          <div style={{ fontSize: 12, opacity: 0.7 }}>{failed}</div>
        </div>
      );
    }
    switch (kind) {
      case "image":
        return (
          <>
            {loading && <LoaderCircle className="spin" size={28} color="#fff" style={{ position: "absolute" }} />}
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
          <LoaderCircle className="spin" size={28} color="#fff" />
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
      className="preview-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="preview-head">
        <span className="title">
          <span className="name">{entry.name}</span>
          <span className="meta">
            {entry.isDir ? "" : formatBytes(entry.size)} · {formatDate(entry.modified, locale)} · {t("preview.of", { i: index + 1, n: entries.length })}
          </span>
        </span>
        <Button onClick={() => onDownload(entry)} title={t("browser.download")}>
          <Download size={14} />
          {t("browser.download")}
        </Button>
        <Button iconOnly onClick={onClose} title="Esc">
          <X size={16} />
        </Button>
      </div>
      <div className="preview-body" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
        <button type="button" className="preview-nav prev" disabled={index === 0} onClick={() => onIndexChange(index - 1)}>
          <ChevronLeft size={20} />
        </button>
        {body()}
        <button type="button" className="preview-nav next" disabled={index >= entries.length - 1} onClick={() => onIndexChange(index + 1)}>
          <ChevronRight size={20} />
        </button>
      </div>
    </div>,
    document.body,
  );
}
