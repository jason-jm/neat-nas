import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  ArrowDownToLine,
  ArrowDownUp,
  ArrowUpFromLine,
  CircleAlert,
  CircleCheck,
  CircleX,
  FolderOpen,
  Inbox,
  LoaderCircle,
  Pause,
  Play,
  RotateCcw,
  X,
} from "lucide-react";
import type { TransferProgress } from "../types";
import { useI18n } from "../i18n";
import { formatBytes, formatEta, formatSpeed } from "../format";
import { Button } from "./ui";

const ACTIVE = new Set(["queued", "scanning", "running"]);
const RESUMABLE = new Set(["cancelled", "error", "interrupted"]);

export const isActiveTransfer = (t: TransferProgress) => ACTIVE.has(t.status);

/** Toolbar button: a progress ring while anything is moving, plus a count badge. */
export function TransfersButton({
  transfers,
  open,
  onClick,
}: {
  transfers: TransferProgress[];
  open: boolean;
  onClick: () => void;
}) {
  const { t } = useI18n();
  const active = transfers.filter(isActiveTransfer);
  const total = active.reduce((a, x) => a + x.bytesTotal, 0);
  const done = active.reduce((a, x) => a + Math.min(x.bytesDone, x.bytesTotal), 0);
  const fraction = active.length === 0 ? 0 : total > 0 ? done / total : 0;
  const r = 10;
  const c = 2 * Math.PI * r;
  return (
    <button
      type="button"
      className={`btn ghost icon transfers-btn${open ? " on" : ""}${active.length ? " busy" : ""}`}
      onClick={onClick}
      title={active.length ? t("transfers.active", { n: active.length }) : t("transfers.title")}
      aria-label={t("transfers.title")}
    >
      {active.length > 0 && (
        <svg className="ring" viewBox="0 0 24 24" width="24" height="24">
          <circle cx="12" cy="12" r={r} className="track" />
          <circle
            cx="12"
            cy="12"
            r={r}
            className={`bar${fraction === 0 ? " indeterminate" : ""}`}
            strokeDasharray={`${c}`}
            strokeDashoffset={`${c * (1 - Math.max(0.04, fraction))}`}
          />
        </svg>
      )}
      <ArrowDownUp size={15} />
      {active.length > 0 && <span className="count">{active.length}</span>}
    </button>
  );
}

interface PopoverProps {
  transfers: TransferProgress[];
  onCancel: (taskId: string) => void;
  onResume: (taskId: string) => void;
  onRemove: (taskId: string) => void;
  onReveal: (path: string) => void;
  onClear: () => void;
  onClose: () => void;
  /** Called when the pointer enters, so an auto-opened popover stays put. */
  onHover: () => void;
}

/** Floating panel under the toolbar, Chrome-downloads style. */
export function TransfersPopover({ transfers, onCancel, onResume, onRemove, onReveal, onClear, onClose, onHover }: PopoverProps) {
  const { t, errorText } = useI18n();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && e.target instanceof Node && ref.current.contains(e.target)) return;
      if (e.target instanceof Element && e.target.closest(".transfers-btn")) return;
      onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const active = transfers.filter(isActiveTransfer).length;
  const finished = transfers.filter((x) => x.status === "done").length;

  const subtitle = (x: TransferProgress): { text: string; cls: string } => {
    switch (x.status) {
      case "running": {
        const parts = [`${formatBytes(x.bytesDone)} / ${formatBytes(x.bytesTotal)}`];
        if (x.speedBps > 0) {
          parts.push(formatSpeed(x.speedBps));
          const eta = formatEta((x.bytesTotal - x.bytesDone) / x.speedBps);
          if (eta) parts.push(eta);
        }
        if (x.isDir) parts.push(t("transfers.files", { done: x.filesDone, total: x.filesTotal }));
        if (x.attempt > 1) parts.push(t("transfers.attempt", { n: x.attempt }));
        return { text: parts.join(" · "), cls: "" };
      }
      case "scanning":
        return { text: x.attempt > 1 ? `${t("status.scanning")} · ${t("transfers.attempt", { n: x.attempt })}` : t("status.scanning"), cls: "" };
      case "done":
        return { text: `${formatBytes(x.bytesDone)} · ${x.kind === "upload" ? `${x.share}/${x.remotePath}` : x.destPath}`, cls: "" };
      case "error":
        return { text: errorText(x.errorCode ?? "unknown", x.error ?? ""), cls: "error" };
      case "interrupted":
        return { text: `${t("status.interrupted")} · ${formatBytes(x.bytesDone)} / ${formatBytes(x.bytesTotal)} · ${errorText(x.errorCode ?? "connection_lost", x.error ?? "")}`, cls: "warn" };
      case "cancelled":
        return { text: `${t("status.cancelled")} · ${formatBytes(x.bytesDone)} / ${formatBytes(x.bytesTotal)}`, cls: "warn" };
      default:
        return { text: t(`status.${x.status}`), cls: "" };
    }
  };

  const icon = (x: TransferProgress): ReactNode => {
    switch (x.status) {
      case "done":
        return <CircleCheck size={18} />;
      case "error":
        return <CircleAlert size={18} />;
      case "cancelled":
        return <Pause size={18} />;
      case "interrupted":
        return <CircleAlert size={18} />;
      case "queued":
        return x.kind === "upload" ? <ArrowUpFromLine size={18} /> : <ArrowDownToLine size={18} />;
      default:
        return <LoaderCircle size={18} className="spin" />;
    }
  };

  return (
    <div className="transfers-pop" ref={ref} onMouseEnter={onHover} role="dialog" aria-label={t("transfers.title")}>
      <div className="transfers-head">
        <span className="title">
          {t("transfers.title")}
          {active > 0 && <span className="badge">{active}</span>}
        </span>
        {finished > 0 && (
          <Button variant="ghost" size="sm" onClick={onClear}>
            {t("transfers.clear")}
          </Button>
        )}
        <Button variant="ghost" size="sm" iconOnly onClick={onClose} title="Esc">
          <X size={14} />
        </Button>
      </div>
      {transfers.length === 0 ? (
        <div className="transfers-empty">
          <Inbox size={26} />
          <span>{t("transfers.empty")}</span>
        </div>
      ) : (
        <div className="transfers-list">
          {transfers.map((x) => {
            const pct = x.bytesTotal > 0 ? Math.min(100, (x.bytesDone / x.bytesTotal) * 100) : 0;
            const indeterminate = x.status === "queued" || x.status === "scanning";
            const sub = subtitle(x);
            return (
              <div key={x.taskId} className="transfer">
                <span className={`t-icon ${x.status}${x.kind === "upload" && ACTIVE.has(x.status) ? " upload" : ""}`}>{icon(x)}</span>
                <span className="t-main">
                  <div className="t-name" title={x.name}>
                    {x.kind === "upload" ? <ArrowUpFromLine size={11} className="t-kind" /> : <ArrowDownToLine size={11} className="t-kind" />}
                    {x.name}
                  </div>
                  <div className={`t-sub ${sub.cls}`} title={sub.text}>
                    {sub.text}
                  </div>
                  {(ACTIVE.has(x.status) || RESUMABLE.has(x.status)) && (
                    <div className={`progress${indeterminate ? " indeterminate" : ""}`}>
                      <div style={indeterminate ? undefined : { width: `${pct}%`, opacity: ACTIVE.has(x.status) ? 1 : 0.45 }} />
                    </div>
                  )}
                </span>
                <span className="t-actions">
                  {ACTIVE.has(x.status) && (
                    <Button variant="ghost" size="sm" iconOnly onClick={() => onCancel(x.taskId)} title={t("transfers.pause")}>
                      <Pause size={14} />
                    </Button>
                  )}
                  {RESUMABLE.has(x.status) && (
                    <>
                      <Button variant="ghost" size="sm" iconOnly onClick={() => onResume(x.taskId)} title={x.status === "error" ? t("transfers.retry") : t("transfers.resume")}>
                        {x.status === "error" ? <RotateCcw size={14} /> : <Play size={14} />}
                      </Button>
                      <Button variant="ghost" size="sm" iconOnly onClick={() => onRemove(x.taskId)} title={t("transfers.discard")}>
                        <CircleX size={14} />
                      </Button>
                    </>
                  )}
                  {x.status === "done" && (
                    <>
                      {x.kind === "download" && (
                        <Button variant="ghost" size="sm" iconOnly onClick={() => onReveal(x.destPath)} title={t("transfers.reveal")}>
                          <FolderOpen size={14} />
                        </Button>
                      )}
                      <Button variant="ghost" size="sm" iconOnly onClick={() => onRemove(x.taskId)} title={t("transfers.remove")}>
                        <X size={14} />
                      </Button>
                    </>
                  )}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** Keeps the popover open briefly after a new transfer starts, unless hovered. */
export function useAutoOpen(transfers: TransferProgress[], setOpen: (open: boolean) => void) {
  const known = useRef<Set<string>>(new Set());
  const timer = useRef<number | null>(null);
  const hovered = useRef(false);
  const [initialised, setInitialised] = useState(false);

  useEffect(() => {
    if (!initialised) {
      // Transfers restored from disk at boot should not pop the panel.
      transfers.forEach((x) => known.current.add(x.taskId));
      if (transfers.length > 0 || known.current.size > 0) setInitialised(true);
      return;
    }
    const fresh = transfers.filter((x) => !known.current.has(x.taskId) && isActiveTransfer(x));
    transfers.forEach((x) => known.current.add(x.taskId));
    if (fresh.length === 0) return;
    setOpen(true);
    hovered.current = false;
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      if (!hovered.current) setOpen(false);
    }, 4000);
  }, [transfers, initialised, setOpen]);

  useEffect(() => {
    const t = window.setTimeout(() => setInitialised(true), 1500);
    return () => window.clearTimeout(t);
  }, []);

  return {
    onHover: () => {
      hovered.current = true;
      if (timer.current) {
        window.clearTimeout(timer.current);
        timer.current = null;
      }
    },
  };
}
