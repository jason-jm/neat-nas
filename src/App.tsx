import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FolderUp } from "lucide-react";
import type { DownloadItem, Entry, SavedServer, Settings, Share, TransferProgress, Unsubscribe } from "./types";
import { api, errorOf, isTauri, logToBackend } from "./api";
import { useI18n } from "./i18n";
import { parentPath } from "./format";
import { Sidebar, type ServerStatus } from "./components/Sidebar";
import { FileBrowser, type BrowserView, type ViewMode } from "./components/FileBrowser";
import { TransfersButton, TransfersPopover, useAutoOpen } from "./components/Transfers";
import { AddServerDialog } from "./components/AddServerDialog";
import { SettingsDialog, type UpdateState } from "./components/SettingsDialog";
import { PreviewModal, isPreviewable } from "./components/PreviewModal";
import { Toasts, type Toast } from "./components/ui";
import { WindowControls } from "./components/WindowControls";
import { isMac, platform } from "./platform";
import { applyTheme, readTheme, type ThemePref } from "./prefs";

interface Nav {
  share: string | null;
  path: string;
}

type Dialog = null | { kind: "add" } | { kind: "edit"; server: SavedServer } | { kind: "settings" };

const FINISHED = new Set(["done", "cancelled", "error", "interrupted"]);
const LAST_SERVER_KEY = "neatnas.lastServer";
const VIEW_MODE_KEY = "neatnas.viewMode";
const SHOW_HIDDEN_KEY = "neatnas.showHidden";
let toastSeq = 0;

function readViewMode(): ViewMode {
  try {
    return localStorage.getItem(VIEW_MODE_KEY) === "grid" ? "grid" : "list";
  } catch {
    return "list";
  }
}

function readShowHidden(): boolean {
  try {
    return localStorage.getItem(SHOW_HIDDEN_KEY) === "1";
  } catch {
    return false;
  }
}

export default function App() {
  const { t, errorText } = useI18n();

  const [servers, setServers] = useState<SavedServer[]>([]);
  const [statuses, setStatuses] = useState<Record<string, ServerStatus>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [shares, setShares] = useState<Share[]>([]);
  const [sharesError, setSharesError] = useState<string | null>(null);
  const [nav, setNav] = useState<Nav>({ share: null, path: "" });
  const [history, setHistory] = useState<{ back: Nav[]; forward: Nav[] }>({ back: [], forward: [] });
  const [entries, setEntries] = useState<Entry[]>([]);
  const [view, setView] = useState<BrowserView>("welcome");
  const [viewMode, setViewModeState] = useState<ViewMode>(readViewMode);
  const [showHidden, setShowHiddenState] = useState<boolean>(readShowHidden);
  const [theme, setThemeState] = useState<ThemePref>(readTheme);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [transfers, setTransfers] = useState<TransferProgress[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [preview, setPreview] = useState<{ entries: Entry[]; index: number } | null>(null);
  const [version, setVersion] = useState("");
  const [transfersOpen, setTransfersOpen] = useState(false);
  const { onHover: onTransfersHover } = useAutoOpen(transfers, setTransfersOpen);
  const [update, setUpdate] = useState<UpdateState>({ kind: "idle" });
  const [dropActive, setDropActive] = useState(false);

  const requestSeq = useRef(0);
  const navRef = useRef(nav);
  navRef.current = nav;
  const selectedRef = useRef(selectedId);
  selectedRef.current = selectedId;
  const refreshTimer = useRef<number | null>(null);

  const selectedServer = useMemo(() => servers.find((s) => s.id === selectedId) ?? null, [servers, selectedId]);

  const notify = useCallback((text: string, kind: "info" | "error" = "info") => {
    const id = ++toastSeq;
    setToasts((list) => [...list, { id, text, kind }]);
    window.setTimeout(() => setToasts((list) => list.filter((x) => x.id !== id)), kind === "error" ? 5000 : 3200);
  }, []);

  const describe = useCallback(
    (e: unknown) => {
      const err = errorOf(e);
      return errorText(err.code, err.message);
    },
    [errorText],
  );

  const setViewMode = (mode: ViewMode) => {
    setViewModeState(mode);
    try {
      localStorage.setItem(VIEW_MODE_KEY, mode);
    } catch {
      /* storage unavailable */
    }
  };

  const setShowHidden = (on: boolean) => {
    setShowHiddenState(on);
    try {
      localStorage.setItem(SHOW_HIDDEN_KEY, on ? "1" : "0");
    } catch {
      /* storage unavailable */
    }
  };
  const setTheme = (pref: ThemePref) => {
    setThemeState(pref);
    applyTheme(pref);
  };

  // ── Navigation ────────────────────────────────────────────────────────
  const loadDir = useCallback(
    async (serverId: string, share: string, path: string, quiet = false) => {
      const req = ++requestSeq.current;
      if (!quiet) setView("loading");
      setErrorMsg(null);
      try {
        const list = await api.listDir(serverId, share, path);
        if (req !== requestSeq.current) return;
        setEntries(list);
        setView("files");
      } catch (e) {
        if (req !== requestSeq.current) return;
        setErrorMsg(describe(e));
        setView("files-error");
      }
    },
    [describe],
  );

  const go = useCallback(
    (target: Nav, push = true) => {
      const serverId = selectedRef.current;
      if (!serverId) return;
      const current = navRef.current;
      if (push && (current.share !== target.share || current.path !== target.path)) {
        setHistory((h) => ({ back: [...h.back, current], forward: [] }));
      }
      setNav(target);
      navRef.current = target;
      setPreview(null);
      if (target.share) {
        void loadDir(serverId, target.share, target.path);
        if (target.share !== current.share) api.rememberShare(serverId, target.share).catch(() => undefined);
      } else {
        requestSeq.current++;
        setEntries([]);
        setView("shares");
      }
    },
    [loadDir],
  );

  const connect = useCallback(
    async (server: SavedServer, openLastShare: boolean) => {
      const req = ++requestSeq.current;
      setSelectedId(server.id);
      selectedRef.current = server.id;
      try {
        localStorage.setItem(LAST_SERVER_KEY, server.id);
      } catch {
        /* storage unavailable */
      }
      setNav({ share: null, path: "" });
      navRef.current = { share: null, path: "" };
      setHistory({ back: [], forward: [] });
      setEntries([]);
      setShares([]);
      setSharesError(null);
      setErrorMsg(null);
      setPreview(null);
      setStatuses((s) => ({ ...s, [server.id]: "connecting" }));
      setView("connecting");
      try {
        const info = await api.listShares(server.id);
        if (req !== requestSeq.current) return;
        setShares(info.shares);
        setSharesError(info.sharesError);
        setStatuses((s) => ({ ...s, [server.id]: "connected" }));
        if (openLastShare && server.lastShare) go({ share: server.lastShare, path: "" }, false);
        else setView("shares");
      } catch (e) {
        if (req !== requestSeq.current) return;
        setStatuses((s) => ({ ...s, [server.id]: "error" }));
        setErrorMsg(describe(e));
        setView("connect-error");
      }
    },
    [describe, go],
  );

  // ── Boot ──────────────────────────────────────────────────────────────
  // On launch, reconnect to the NAS used last time (or the only one saved) so
  // the files are on screen without a click.
  const booted = useRef(false);
  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    api
      .listServers()
      .then((list) => {
        setServers(list);
        let lastId: string | null = null;
        try {
          lastId = localStorage.getItem(LAST_SERVER_KEY);
        } catch {
          /* storage unavailable */
        }
        const target = list.find((s) => s.id === lastId) ?? (list.length === 1 ? list[0] : undefined);
        if (target) void connect(target, true);
      })
      .catch((e) => notify(describe(e), "error"));
    api.getSettings().then(setSettings).catch(() => undefined);
    api.listTransfers().then(setTransfers).catch(() => undefined);
    api.appVersion().then(setVersion).catch(() => undefined);
  }, [notify, describe, connect]);

  // index.html painted the stored theme already; this also tells the native window.
  useEffect(() => {
    applyTheme(readTheme());
  }, []);

  // ── Updates ───────────────────────────────────────────────────────────
  const checkUpdate = useCallback(
    async (quiet: boolean) => {
      setUpdate({ kind: "checking" });
      try {
        const info = await api.checkForUpdate();
        if (info) {
          setUpdate({ kind: "available", info });
          if (quiet) notify(t("toast.updateAvailable", { v: info.version }));
        } else {
          setUpdate(quiet ? { kind: "idle" } : { kind: "latest" });
        }
      } catch (e) {
        setUpdate(quiet ? { kind: "idle" } : { kind: "error", message: errorOf(e).message });
      }
    },
    [notify, t],
  );

  const installUpdate = useCallback(async () => {
    if (update.kind !== "available") return;
    const info = update.info;
    setUpdate({ kind: "installing", info, progress: { phase: "downloading", downloaded: 0, total: null } });
    try {
      await api.installUpdate((progress) => setUpdate({ kind: "installing", info, progress }));
    } catch (e) {
      setUpdate({ kind: "error", message: errorOf(e).message });
      notify(describe(e), "error");
    }
  }, [update, notify, describe]);

  // A quiet check shortly after launch; only the real app talks to the updater.
  useEffect(() => {
    if (!isTauri) return;
    const timer = window.setTimeout(() => void checkUpdate(true), 5000);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Transfer events ───────────────────────────────────────────────────
  const scheduleRefresh = useCallback(() => {
    if (refreshTimer.current) window.clearTimeout(refreshTimer.current);
    refreshTimer.current = window.setTimeout(() => {
      const serverId = selectedRef.current;
      const current = navRef.current;
      if (serverId && current.share) void loadDir(serverId, current.share, current.path, true);
    }, 400);
  }, [loadDir]);

  useEffect(() => {
    let unlisten: Unsubscribe | undefined;
    let disposed = false;
    api
      .onTransferProgress((p) => {
        setTransfers((list) => {
          const i = list.findIndex((x) => x.taskId === p.taskId);
          if (i < 0) return [p, ...list];
          if (FINISHED.has(list[i].status) && !FINISHED.has(p.status) && list[i].attempt >= p.attempt) return list; // stale, late update
          const next = list.slice();
          next[i] = p;
          return next;
        });
        if (p.status === "done") {
          notify(p.kind === "upload" ? t("toast.uploaded", { name: p.name }) : t("toast.done", { name: p.name }));
          const current = navRef.current;
          if (p.kind === "upload" && p.serverId === selectedRef.current && p.share === current.share && p.destPath === current.path) scheduleRefresh();
        }
        if (p.status === "error") notify(`${t("toast.failed", { name: p.name })} · ${errorText(p.errorCode ?? "unknown", p.error ?? "")}`, "error");
        if (p.status === "interrupted") notify(`${t("status.interrupted")}: ${p.name} · ${errorText(p.errorCode ?? "connection_lost", p.error ?? "")}`, "error");
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [notify, t, errorText, scheduleRefresh]);

  // ── OS file drops (upload) ────────────────────────────────────────────
  useEffect(() => {
    let unlisten: Unsubscribe | undefined;
    let disposed = false;
    api
      .onFileDrop((e) => {
        if (e.type === "enter" || e.type === "over") setDropActive(true);
        else if (e.type === "leave") setDropActive(false);
        else if (e.type === "drop") {
          setDropActive(false);
          const serverId = selectedRef.current;
          const current = navRef.current;
          if (!serverId || !current.share || e.paths.length === 0) return;
          api
            .startUpload(serverId, current.share, current.path, e.paths)
            .then((queued) => {
              setTransfers((list) => [...queued.filter((q) => !list.some((x) => x.taskId === q.taskId)), ...list]);
            })
            .catch((err) => notify(describe(err), "error"));
        }
      })
      .then((u) => {
        if (disposed) u();
        else unlisten = u;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [notify, describe, t]);

  const selectServer = useCallback(
    (id: string) => {
      const server = servers.find((s) => s.id === id);
      if (!server) return;
      if (id === selectedId && statuses[id] === "connected") return;
      void connect(server, true);
    },
    [servers, selectedId, statuses, connect],
  );

  // Navigation must not happen inside a state updater: React invokes
  // updaters twice in development, which double-fetched every listing.
  const back = () => {
    const prev = history.back[history.back.length - 1];
    if (!prev) return;
    const current = navRef.current;
    setHistory({ back: history.back.slice(0, -1), forward: [current, ...history.forward] });
    go(prev, false);
  };
  const forward = () => {
    const next = history.forward[0];
    if (!next) return;
    const current = navRef.current;
    setHistory({ back: [...history.back, current], forward: history.forward.slice(1) });
    go(next, false);
  };
  const up = () => {
    if (!nav.share) return;
    if (nav.path) go({ share: nav.share, path: parentPath(nav.path) });
    else go({ share: null, path: "" });
  };
  const refresh = () => {
    if (!selectedServer) return;
    if (nav.share) void loadDir(selectedServer.id, nav.share, nav.path);
    else void connect(selectedServer, false);
  };
  const retry = () => {
    if (!selectedServer) return;
    if (view === "connect-error") void connect(selectedServer, true);
    else refresh();
  };

  // ── Transfers ─────────────────────────────────────────────────────────
  const mergeQueued = (queued: TransferProgress[]) =>
    setTransfers((list) => [...queued.filter((q) => !list.some((x) => x.taskId === q.taskId)), ...list]);

  const download = useCallback(
    async (items: DownloadItem[], chooseFolder: boolean) => {
      const serverId = selectedRef.current;
      const share = navRef.current.share;
      if (!serverId || !share || items.length === 0) return;
      let dest: string | null = null;
      if (chooseFolder) {
        dest = await api.pickFolder(settings?.downloadDir);
        if (!dest) return;
      }
      try {
        const queued = await api.startDownload(serverId, share, items, dest);
        mergeQueued(queued);
      } catch (e) {
        notify(describe(e), "error");
      }
    },
    [settings, notify, t, describe],
  );

  const upload = useCallback(
    async (directory: boolean) => {
      const serverId = selectedRef.current;
      const current = navRef.current;
      if (!serverId || !current.share) return;
      const paths = await api.pickFiles(directory);
      if (!paths || paths.length === 0) return;
      try {
        const queued = await api.startUpload(serverId, current.share, current.path, paths);
        mergeQueued(queued);
      } catch (e) {
        notify(describe(e), "error");
      }
    },
    [notify, t, describe],
  );

  const dragOut = useCallback(
    (items: DownloadItem[]) => {
      const serverId = selectedRef.current;
      const share = navRef.current.share;
      if (!serverId || !share) return;
      api.startDragOut(serverId, share, items).catch((e) => {
        const err = errorOf(e);
        notify(err.code === "unsupported" ? t("toast.dragUnsupported") : describe(e), "error");
      });
    },
    [notify, t, describe],
  );

  const cancelTransfer = (taskId: string) => api.cancelTransfer(taskId).catch((e) => notify(describe(e), "error"));
  const resumeTransfer = (taskId: string) =>
    api
      .resumeTransfer(taskId)
      .then((p) => {
        setTransfers((list) => list.map((x) => (x.taskId === p.taskId ? p : x)));
        notify(t("toast.resumed", { name: p.name }));
      })
      .catch((e) => notify(describe(e), "error"));
  const removeTransfer = (taskId: string) =>
    api
      .removeTransfer(taskId)
      .then(() => setTransfers((list) => list.filter((x) => x.taskId !== taskId)))
      .catch((e) => notify(describe(e), "error"));
  const reveal = (path: string) => api.revealItem(path).catch((e) => notify(describe(e), "error"));
  const clearFinished = () =>
    api
      .clearFinishedTransfers()
      .then(() => setTransfers((list) => list.filter((x) => x.status !== "done")))
      .catch((e) => notify(describe(e), "error"));

  // ── Preview ───────────────────────────────────────────────────────────
  const openPreview = (entry: Entry, ordered: Entry[]) => {
    const list = ordered.filter((e) => !e.isDir && isPreviewable(e));
    const index = list.findIndex((e) => e.path === entry.path);
    // Files Quick Look cannot render still open, on their own, with the notice.
    if (index >= 0) setPreview({ entries: list, index });
    else if (!entry.isDir) setPreview({ entries: [entry], index: 0 });
  };

  // ── Server management ─────────────────────────────────────────────────
  const onSaved = (server: SavedServer, isNew: boolean) => {
    setServers((list) => (isNew ? [...list, server] : list.map((s) => (s.id === server.id ? server : s))));
    setDialog(null);
    notify(t("toast.saved"));
    void connect(server, !isNew);
  };

  const removeServer = async (server: SavedServer) => {
    if (!window.confirm(t("server.removeConfirm", { name: server.name }))) return;
    try {
      await api.removeServer(server.id);
      setServers((list) => list.filter((s) => s.id !== server.id));
      if (selectedId === server.id) {
        requestSeq.current++;
        setSelectedId(null);
        setView("welcome");
        setNav({ share: null, path: "" });
        try {
          localStorage.removeItem(LAST_SERVER_KEY);
        } catch {
          /* storage unavailable */
        }
      }
    } catch (e) {
      notify(describe(e), "error");
    }
  };

  const disconnect = async (server: SavedServer) => {
    await api.disconnectServer(server.id).catch(() => undefined);
    setStatuses((s) => ({ ...s, [server.id]: "idle" }));
    if (selectedId === server.id) {
      requestSeq.current++;
      setSelectedId(null);
      setView("welcome");
      setNav({ share: null, path: "" });
    }
  };

  // ── Dev autopilot ─────────────────────────────────────────────────────
  // NEATNAS_DEV_AUTOPILOT="open=Photos/2024,grid,preview=IMG_0003.jpg"
  // replays UI steps once the listing is on screen, so the real app can be
  // screenshotted by tooling. Inert unless the backend passes the string.
  const autopilotSteps = useRef<string[] | null>(null);
  const [autoTick, setAutoTick] = useState(0);
  useEffect(() => {
    if (!settings?.devAutopilot || autopilotSteps.current) return;
    autopilotSteps.current = settings.devAutopilot.split(",").map((s) => s.trim()).filter(Boolean);
    setAutoTick((n) => n + 1);
  }, [settings]);
  useEffect(() => {
    const steps = autopilotSteps.current;
    if (!steps || steps.length === 0 || view !== "files") return;
    const step = steps[0];
    logToBackend(`autopilot armed step=${step} tick=${autoTick} entries=${entries.length}`);
    const timer = window.setTimeout(() => {
      steps.shift();
      logToBackend(`autopilot step=${step} view=${view} share=${nav.share} path=${nav.path} entries=${entries.length} dom=${document.body.innerText.replace(/\s+/g, " ").slice(0, 160)}`);
      if (step === "grid") setViewMode("grid");
      else if (step === "list") setViewMode("list");
      else if (step.startsWith("open=")) {
        if (nav.share) go({ share: nav.share, path: step.slice(5) });
      } else if (step.startsWith("preview=")) {
        const target = entries.find((e) => e.name === step.slice(8));
        if (target) openPreview(target, entries);
      }
      // Advance even when the step changed no state (e.g. mode already set).
      window.setTimeout(() => setAutoTick((n) => n + 1), 900);
    }, 700);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, entries, autoTick]);

  const dropTarget = nav.share ? `${nav.share}/${nav.path}`.replace(/\/$/, "") : null;

  return (
    <div className="app">
      <Sidebar
        servers={servers}
        selectedId={selectedId}
        statuses={statuses}
        isMac={isMac}
        onSelect={selectServer}
        onAdd={() => setDialog({ kind: "add" })}
        onEdit={(server) => setDialog({ kind: "edit", server })}
        onRemove={removeServer}
        onDisconnect={disconnect}
        onSettings={() => setDialog({ kind: "settings" })}
        updateAvailable={update.kind === "available"}
      />
      <section className="main" style={{ position: "relative" }}>
        <FileBrowser
          server={selectedServer}
          hasServers={servers.length > 0}
          share={nav.share}
          path={nav.path}
          shares={shares}
          sharesError={sharesError}
          entries={entries}
          view={view}
          viewMode={viewMode}
          showHidden={showHidden}
          errorText={errorMsg}
          canBack={history.back.length > 0}
          canForward={history.forward.length > 0}
          onViewMode={setViewMode}
          onBack={back}
          onForward={forward}
          onUp={up}
          onOpenShare={(name) => go({ share: name, path: "" })}
          onOpenDir={(path) => nav.share && go({ share: nav.share, path })}
          onGoRoot={() => go({ share: null, path: "" })}
          onRefresh={refresh}
          onRetry={retry}
          onEditServer={() => selectedServer && setDialog({ kind: "edit", server: selectedServer })}
          onAddServer={() => setDialog({ kind: "add" })}
          onDownload={download}
          onUpload={upload}
          onPreview={openPreview}
          onDragOut={dragOut}
          onCopied={() => notify(t("toast.copied"))}
          toolbarExtra={<TransfersButton transfers={transfers} open={transfersOpen} onClick={() => setTransfersOpen((o) => !o)} />}
          toolbarEnd={platform !== "macos" ? <WindowControls /> : undefined}
        />
        {transfersOpen && (
          <TransfersPopover
            transfers={transfers}
            onCancel={cancelTransfer}
            onResume={resumeTransfer}
            onRemove={removeTransfer}
            onReveal={reveal}
            onClear={clearFinished}
            onClose={() => setTransfersOpen(false)}
            onHover={onTransfersHover}
          />
        )}
        {dropActive && (
          <div className={`drop-overlay${dropTarget ? "" : " blocked"}`}>
            <div className="big-icon">
              <FolderUp size={30} />
            </div>
            {dropTarget ? t("browser.dropHere", { dir: dropTarget }) : t("browser.dropNoTarget")}
          </div>
        )}
      </section>

      {preview && selectedId && nav.share && (
        <PreviewModal
          serverId={selectedId}
          share={nav.share}
          entries={preview.entries}
          index={preview.index}
          onIndexChange={(index) => setPreview((p) => (p ? { ...p, index } : p))}
          onClose={() => setPreview(null)}
          onDownload={(entry) => download([{ path: entry.path, name: entry.name, isDir: entry.isDir }], false)}
        />
      )}
      {dialog?.kind === "add" && <AddServerDialog mode="add" onClose={() => setDialog(null)} onSaved={onSaved} />}
      {dialog?.kind === "edit" && <AddServerDialog mode="edit" server={dialog.server} onClose={() => setDialog(null)} onSaved={onSaved} />}
      {dialog?.kind === "settings" && (
        <SettingsDialog
          settings={settings}
          showHidden={showHidden}
          onShowHidden={setShowHidden}
          theme={theme}
          onTheme={setTheme}
          version={version}
          update={update}
          onCheckUpdate={() => void checkUpdate(false)}
          onInstallUpdate={() => void installUpdate()}
          onClose={() => setDialog(null)}
          onSettingsChanged={setSettings}
          onError={(text) => notify(text, "error")}
        />
      )}
      <Toasts toasts={toasts} />
    </div>
  );
}
