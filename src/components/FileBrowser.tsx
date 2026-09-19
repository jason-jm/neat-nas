import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import {
  ArrowUp,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  CircleAlert,
  Copy,
  Download,
  Eye,
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
  Folder,
  FolderDown,
  FolderOpen,
  FolderSearch,
  FolderUp,
  HardDrive,
  Inbox,
  LayoutGrid,
  List,
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Server,
  Upload,
  X,
} from "lucide-react";
import type { DownloadItem, Entry, SavedServer, Share } from "../types";
import { api } from "../api";
import { useI18n } from "../i18n";
import { fileKind, formatBytes, formatDate, type FileKind } from "../format";
import { Button, ContextMenu, StateView, type MenuItem } from "./ui";
import { isPreviewable } from "./PreviewModal";

export type BrowserView = "welcome" | "connecting" | "connect-error" | "shares" | "loading" | "files" | "files-error";
export type ViewMode = "list" | "grid";

interface Props {
  server: SavedServer | null;
  hasServers: boolean;
  share: string | null;
  path: string;
  shares: Share[];
  sharesError: string | null;
  entries: Entry[];
  view: BrowserView;
  viewMode: ViewMode;
  showHidden: boolean;
  errorText: string | null;
  canBack: boolean;
  canForward: boolean;
  onViewMode: (mode: ViewMode) => void;
  onBack: () => void;
  onForward: () => void;
  onUp: () => void;
  onOpenShare: (name: string) => void;
  onOpenDir: (path: string) => void;
  onGoRoot: () => void;
  onRefresh: () => void;
  onRetry: () => void;
  onEditServer: () => void;
  onAddServer: () => void;
  onDownload: (items: DownloadItem[], chooseFolder: boolean) => void;
  onUpload: (directory: boolean) => void;
  onPreview: (entry: Entry, ordered: Entry[]) => void;
  onDragOut: (items: DownloadItem[]) => void;
  onCopied: () => void;
  /** Extra toolbar controls rendered before the refresh button (transfers). */
  toolbarExtra?: ReactNode;
  /** Window controls at the far right (frameless windows on Windows/Linux). */
  toolbarEnd?: ReactNode;
}

type SortKey = "name" | "modified" | "size";

/** One row/cell in the browser: a share at the root, or a file/folder inside one. */
interface Item {
  key: string;
  name: string;
  isDir: boolean;
  isShare: boolean;
  comment: string;
  size: number;
  modified: number | null;
  entry: Entry | null;
}

const ICONS: Record<FileKind, typeof File> = {
  folder: Folder,
  image: FileImage,
  video: FileVideo,
  audio: FileAudio,
  archive: FileArchive,
  doc: FileText,
  sheet: FileSpreadsheet,
  code: FileCode,
  file: File,
};

function ItemIcon({ item, size = 16 }: { item: Item; size?: number }) {
  if (item.isShare) {
    return (
      <span className="file-icon share">
        <HardDrive size={size} />
      </span>
    );
  }
  const kind = fileKind(item.name, item.isDir);
  const Icon = ICONS[kind];
  return (
    <span className={`file-icon ${kind}`}>
      <Icon size={size} />
    </span>
  );
}

const toItem = (e: Entry): DownloadItem => ({ path: e.path, name: e.name, isDir: e.isDir });
const isHidden = (name: string) => name.startsWith(".") || name === "Thumbs.db" || name === "desktop.ini";

// ── Thumbnails: module-level cache so navigating back is instant ────────

const thumbCache = new Map<string, string | null>();
const thumbInflight = new Map<string, Promise<string | null>>();

function thumbKey(serverId: string, share: string, e: Entry) {
  return `${serverId}|${share}|${e.path}|${e.size}|${e.modified ?? 0}`;
}

function loadThumb(serverId: string, share: string, e: Entry): Promise<string | null> {
  const key = thumbKey(serverId, share, e);
  const cached = thumbCache.get(key);
  if (cached !== undefined) return Promise.resolve(cached);
  const pending = thumbInflight.get(key);
  if (pending) return pending;
  const p = api
    .thumbnail(serverId, share, e.path, e.size, e.modified ?? 0)
    .then((url) => url || null)
    .catch(() => null)
    .then((v) => {
      thumbCache.set(key, v);
      thumbInflight.delete(key);
      return v;
    });
  thumbInflight.set(key, p);
  return p;
}

function useInView<T extends Element>(): [React.RefObject<T | null>, boolean] {
  const ref = useRef<T | null>(null);
  const [inView, setInView] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el || inView) return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries.some((x) => x.isIntersecting)) setInView(true);
      },
      { rootMargin: "200px" },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [inView]);
  return [ref, inView];
}

function GridCell({
  item,
  serverId,
  share,
  selected,
  index,
  onClick,
  onDoubleClick,
  onContextMenu,
  onMouseDown,
}: {
  item: Item;
  serverId: string;
  share: string;
  selected: boolean;
  index: number;
  onClick: (e: MouseEvent) => void;
  onDoubleClick: () => void;
  onContextMenu: (e: MouseEvent) => void;
  onMouseDown: (e: MouseEvent) => void;
}) {
  const [ref, inView] = useInView<HTMLDivElement>();
  const entry = item.entry;
  const key = entry ? thumbKey(serverId, share, entry) : "";
  const [thumb, setThumb] = useState<string | null | undefined>(() => (key ? thumbCache.get(key) : undefined));
  const wantsThumb = !!entry && !item.isDir && fileKind(item.name, false) === "image";
  useEffect(() => {
    if (!inView || !wantsThumb || !entry) return;
    const cached = thumbCache.get(key);
    if (cached !== undefined) {
      setThumb(cached);
      return;
    }
    let alive = true;
    loadThumb(serverId, share, entry).then((v) => alive && setThumb(v));
    return () => {
      alive = false;
    };
  }, [inView, key, wantsThumb, serverId, share, entry]);

  return (
    <div
      ref={ref}
      data-index={index}
      className={`grid-cell${selected ? " selected" : ""}`}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      onMouseDown={onMouseDown}
      title={item.comment ? `${item.name} — ${item.comment}` : item.name}
    >
      <div className={`thumb${item.isDir ? " folder" : ""}`}>
        {thumb ? <img src={thumb} alt="" draggable={false} /> : <ItemIcon item={item} size={52} />}
      </div>
      <div className="label">{item.name}</div>
      <div className="sub">{item.isShare ? item.comment : item.isDir ? "" : formatBytes(item.size)}</div>
    </div>
  );
}

// ── Browser ─────────────────────────────────────────────────────────────

export function FileBrowser(p: Props) {
  const { t, locale } = useI18n();
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; asc: boolean }>({ key: "name", asc: true });
  const [selection, setSelection] = useState<Set<string>>(new Set());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; item: Item } | null>(null);
  const [uploadMenu, setUploadMenu] = useState<{ x: number; y: number } | null>(null);
  const [manualShare, setManualShare] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  // Rubber-band selection state. Coordinates are content-relative and
  // include the scroll offset, so the box stays put while auto-scrolling.
  const marqueeRef = useRef<{ startX: number; startY: number; additive: boolean; base: Set<string>; active: boolean } | null>(null);
  const [marquee, setMarquee] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const suppressClickRef = useRef(false);
  const visibleRef = useRef<Item[]>([]);
  const dragRef = useRef<{ x: number; y: number; item: Item; started: boolean } | null>(null);

  const atRoot = p.view === "shares";
  const browsing = p.view === "shares" || p.view === "files";
  const hasServer = !!p.server;
  // While a folder loads, keep the previous listing on screen (dimmed) so the
  // chrome around it never jumps; the spinner is only for a first load.
  const loadingStale = p.view === "loading" && p.entries.length > 0;
  const showPathbar = hasServer && p.view !== "welcome" && p.view !== "connect-error";

  useEffect(() => {
    setSelection(new Set());
    setAnchor(null);
    setFilter("");
    setMenu(null);
  }, [p.share, p.path, p.server?.id]);

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f" && browsing) {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [browsing]);

  const visible = useMemo<Item[]>(() => {
    const q = filter.trim().toLowerCase();
    let list: Item[];
    if (atRoot) {
      list = p.shares.map((s) => ({ key: s.name, name: s.name, isDir: true, isShare: true, comment: s.comment, size: 0, modified: null, entry: null }));
    } else {
      list = p.entries
        .filter((e) => p.showHidden || !isHidden(e.name))
        .map((e) => ({ key: e.path, name: e.name, isDir: e.isDir, isShare: false, comment: "", size: e.size, modified: e.modified, entry: e }));
    }
    if (q) list = list.filter((i) => i.name.toLowerCase().includes(q));
    const dir = sort.asc ? 1 : -1;
    list.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      switch (sort.key) {
        case "size":
          return (a.size - b.size) * dir || a.name.localeCompare(b.name);
        case "modified":
          return ((a.modified ?? 0) - (b.modified ?? 0)) * dir || a.name.localeCompare(b.name);
        default:
          return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" }) * dir;
      }
    });
    return list;
  }, [atRoot, p.shares, p.entries, p.showHidden, filter, sort]);

  const hiddenCount = useMemo(() => (atRoot || p.showHidden ? 0 : p.entries.filter((e) => isHidden(e.name)).length), [atRoot, p.entries, p.showHidden]);
  const orderedEntries = useMemo(() => visible.flatMap((i) => (i.entry ? [i.entry] : [])), [visible]);
  const selectedItems = useMemo(() => visible.filter((i) => selection.has(i.key)), [visible, selection]);
  const selectedEntries = useMemo(() => selectedItems.flatMap((i) => (i.entry ? [i.entry] : [])), [selectedItems]);
  const selectedBytes = useMemo(() => selectedEntries.reduce((a, e) => a + (e.isDir ? 0 : e.size), 0), [selectedEntries]);
  const canDownload = !atRoot && selectedEntries.length > 0;

  const toggleSort = (key: SortKey) => setSort((s) => ({ key, asc: s.key === key ? !s.asc : key === "name" }));

  const onItemClick = (e: MouseEvent, item: Item, index: number) => {
    e.stopPropagation();
    const meta = e.metaKey || e.ctrlKey;
    if (e.shiftKey && anchor !== null) {
      const [a, b] = [Math.min(anchor, index), Math.max(anchor, index)];
      setSelection(new Set(visible.slice(a, b + 1).map((x) => x.key)));
    } else if (meta) {
      setSelection((s) => {
        const next = new Set(s);
        if (next.has(item.key)) next.delete(item.key);
        else next.add(item.key);
        return next;
      });
      setAnchor(index);
    } else {
      setSelection(new Set([item.key]));
      setAnchor(index);
    }
  };

  const activate = useCallback(
    (item: Item) => {
      if (item.isShare) p.onOpenShare(item.name);
      else if (!item.entry) return;
      else if (item.isDir) p.onOpenDir(item.entry.path);
      else if (isPreviewable(item.entry)) p.onPreview(item.entry, orderedEntries);
      else p.onDownload([toItem(item.entry)], false);
    },
    [p, orderedEntries],
  );

  const onItemContext = (e: MouseEvent, item: Item, index: number) => {
    e.preventDefault();
    e.stopPropagation();
    if (!selection.has(item.key)) {
      setSelection(new Set([item.key]));
      setAnchor(index);
    }
    setMenu({ x: e.clientX, y: e.clientY, item });
  };

  visibleRef.current = visible;

  // Clicking empty space clears the selection (rows stop propagation). A
  // click that ended a rubber-band drag must not.
  const onContentClick = () => {
    if (suppressClickRef.current) {
      suppressClickRef.current = false;
      return;
    }
    if (selection.size > 0) setSelection(new Set());
  };

  const contentPoint = (e: globalThis.MouseEvent | MouseEvent) => {
    const el = contentRef.current!;
    const r = el.getBoundingClientRect();
    return { x: e.clientX - r.left + el.scrollLeft, y: e.clientY - r.top + el.scrollTop, rect: r, el };
  };

  const onMarqueeMove = useCallback((e: globalThis.MouseEvent) => {
    const m = marqueeRef.current;
    const el = contentRef.current;
    if (!m || !el) return;
    const pt = contentPoint(e);
    const rect = pt.rect;
    // Keep the box inside the scrollable list area and below the sticky
    // column header, so it never reaches over other UI.
    const headerH = el.querySelector("thead")?.getBoundingClientRect().height ?? 0;
    const x = Math.min(Math.max(pt.x, 0), Math.max(el.scrollWidth, el.clientWidth));
    const y = Math.min(Math.max(pt.y, el.scrollTop + headerH), Math.max(el.scrollHeight, el.clientHeight));
    if (!m.active) {
      if (Math.abs(x - m.startX) < 4 && Math.abs(y - m.startY) < 4) return;
      m.active = true;
    }
    // Auto-scroll when dragging past the edges.
    if (e.clientY > rect.bottom - 24) el.scrollTop += 14;
    else if (e.clientY < rect.top + 24) el.scrollTop -= 14;
    const box = { x: Math.min(m.startX, x), y: Math.min(m.startY, y), w: Math.abs(x - m.startX), h: Math.abs(y - m.startY) };
    setMarquee(box);
    const hits = new Set<string>(m.additive ? m.base : []);
    el.querySelectorAll<HTMLElement>("[data-index]").forEach((node) => {
      const cr = node.getBoundingClientRect();
      const nx = cr.left - rect.left + el.scrollLeft;
      const ny = cr.top - rect.top + el.scrollTop;
      const overlaps = nx < box.x + box.w && nx + cr.width > box.x && ny < box.y + box.h && ny + cr.height > box.y;
      const item = visibleRef.current[Number(node.dataset.index)];
      if (!item) return;
      if (overlaps) hits.add(item.key);
      else if (!m.additive) hits.delete(item.key);
    });
    setSelection(hits);
  }, []);

  const onMarqueeUp = useCallback(() => {
    window.removeEventListener("mousemove", onMarqueeMove);
    window.removeEventListener("mouseup", onMarqueeUp);
    if (marqueeRef.current?.active) suppressClickRef.current = true;
    marqueeRef.current = null;
    document.body.classList.remove("marqueeing");
    setMarquee(null);
  }, [onMarqueeMove]);

  const onContentMouseDown = (e: MouseEvent) => {
    if (e.button !== 0 || !browsing) return;
    if (!contentRef.current) return;
    const target = e.target as HTMLElement;
    if (target.closest("[data-index], .btn, input, button, thead, form")) return;
    // Stop the WebView from starting a text selection or drag of its own;
    // preventDefault also skips the focus change, so focus explicitly.
    e.preventDefault();
    contentRef.current.focus({ preventScroll: true });
    document.body.classList.add("marqueeing");
    const { x, y } = contentPoint(e);
    marqueeRef.current = { startX: x, startY: y, additive: e.metaKey || e.ctrlKey || e.shiftKey, base: new Set(selection), active: false };
    window.addEventListener("mousemove", onMarqueeMove);
    window.addEventListener("mouseup", onMarqueeUp);
  };

  // Drag-out: a real mouse gesture (not HTML5 drag) hands the selection to
  // the native drag session once the pointer moves a few pixels.
  const onItemMouseDown = (e: MouseEvent, item: Item) => {
    if (e.button !== 0 || !item.entry) return;
    dragRef.current = { x: e.clientX, y: e.clientY, item, started: false };
  };
  const onContentMouseMove = (e: MouseEvent) => {
    const d = dragRef.current;
    if (!d || d.started || e.buttons !== 1) return;
    if (Math.abs(e.clientX - d.x) < 6 && Math.abs(e.clientY - d.y) < 6) return;
    d.started = true;
    const targets = selection.has(d.item.key) && selectedEntries.length > 0 ? selectedEntries : d.item.entry ? [d.item.entry] : [];
    if (!selection.has(d.item.key)) setSelection(new Set([d.item.key]));
    if (targets.length > 0) p.onDragOut(targets.map(toItem));
  };
  const onContentMouseUp = () => {
    dragRef.current = null;
  };

  const copyPath = async (item: Item) => {
    const text = item.isShare ? `smb://${p.server?.host ?? ""}/${item.name}` : `smb://${p.server?.host ?? ""}/${p.share ?? ""}/${item.entry?.path ?? ""}`;
    try {
      await navigator.clipboard.writeText(text);
      p.onCopied();
    } catch {
      /* clipboard unavailable */
    }
  };

  const menuItems = (item: Item): MenuItem[] => {
    if (item.isShare) {
      return [
        { label: t("menu.open"), icon: <FolderOpen size={14} />, onClick: () => p.onOpenShare(item.name) },
        { separator: true },
        { label: t("menu.copyPath"), icon: <Copy size={14} />, onClick: () => copyPath(item) },
      ];
    }
    const entry = item.entry!;
    const targets = selection.has(item.key) && selectedEntries.length > 1 ? selectedEntries : [entry];
    const items: MenuItem[] = [];
    if (entry.isDir && targets.length === 1) {
      items.push({ label: t("menu.open"), icon: <FolderOpen size={14} />, onClick: () => p.onOpenDir(entry.path) });
    }
    if (!entry.isDir && targets.length === 1) {
      items.push({ label: t("menu.preview"), icon: <Eye size={14} />, onClick: () => p.onPreview(entry, orderedEntries) });
    }
    items.push(
      { label: t("menu.download"), icon: <Download size={14} />, onClick: () => p.onDownload(targets.map(toItem), false) },
      { label: t("menu.downloadTo"), icon: <FolderDown size={14} />, onClick: () => p.onDownload(targets.map(toItem), true) },
      { separator: true },
      { label: t("menu.copyPath"), icon: <Copy size={14} />, onClick: () => copyPath(item) },
    );
    return items;
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!browsing) return;
    if (e.target instanceof HTMLInputElement) return;
    const meta = e.metaKey || e.ctrlKey;
    if (meta && e.key.toLowerCase() === "a") {
      e.preventDefault();
      setSelection(new Set(visible.map((x) => x.key)));
      return;
    }
    if (e.key === "Escape") {
      setSelection(new Set());
      return;
    }
    if (e.key === "Backspace" && !meta) {
      e.preventDefault();
      p.onUp();
      return;
    }
    if (e.key === " ") {
      e.preventDefault();
      const target = selectedEntries.find((x) => !x.isDir);
      if (target) p.onPreview(target, orderedEntries);
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (selectedItems.length === 1) activate(selectedItems[0]);
      else if (selectedEntries.length > 1) p.onDownload(selectedEntries.map(toItem), false);
      return;
    }
    const isArrow = ["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].includes(e.key);
    if (isArrow) {
      e.preventDefault();
      if (visible.length === 0) return;
      const current = anchor ?? -1;
      let step = 1;
      if (p.viewMode === "grid" && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
        const grid = contentRef.current?.querySelector<HTMLElement>(".file-grid");
        if (grid) step = Math.max(1, getComputedStyle(grid).gridTemplateColumns.split(" ").length);
      }
      const delta = e.key === "ArrowDown" || e.key === "ArrowRight" ? step : -step;
      const next = Math.max(0, Math.min(visible.length - 1, current + delta));
      setAnchor(next);
      setSelection(new Set([visible[next].key]));
      contentRef.current?.querySelector<HTMLElement>(`[data-index="${next}"]`)?.scrollIntoView({ block: "nearest" });
    }
  };

  const segments = p.path ? p.path.split("/") : [];
  const locationTitle = !p.server ? "" : !p.share ? p.server.name : segments.length ? segments[segments.length - 1] : p.share;
  const pathSegments: { label: string; icon: ReactNode; go: () => void }[] = p.server
    ? [
        { label: p.server.name, icon: <HardDrive size={12} />, go: p.onGoRoot },
        ...(p.share ? [{ label: p.share, icon: <Folder size={12} />, go: () => p.onOpenDir("") }] : []),
        ...segments.map((seg, i) => ({ label: seg, icon: <Folder size={12} />, go: () => p.onOpenDir(segments.slice(0, i + 1).join("/")) })),
      ]
    : [];
  const SortIcon = ({ k }: { k: SortKey }) => (sort.key === k ? sort.asc ? <ChevronUp size={12} /> : <ChevronDown size={12} /> : null);

  const listView = (
    <table className="file-table">
      <thead>
        <tr>
          <th className="sortable" onClick={() => toggleSort("name")}>
            <span className="th-inner">
              {t("col.name")} <SortIcon k="name" />
            </span>
          </th>
          {atRoot ? (
            <th className="comment">{t("col.comment")}</th>
          ) : (
            <>
              <th className="date sortable" onClick={() => toggleSort("modified")}>
                <span className="th-inner">
                  {t("col.modified")} <SortIcon k="modified" />
                </span>
              </th>
              <th className="size sortable" onClick={() => toggleSort("size")}>
                <span className="th-inner">
                  {t("col.size")} <SortIcon k="size" />
                </span>
              </th>
            </>
          )}
        </tr>
      </thead>
      <tbody>
        <tr className="spacer">
          <td colSpan={atRoot ? 2 : 3} />
        </tr>
        {visible.map((item, i) => (
          <tr
            key={item.key}
            data-index={i}
            className={`row${selection.has(item.key) ? " selected" : ""}`}
            onClick={(e) => onItemClick(e, item, i)}
            onDoubleClick={() => activate(item)}
            onContextMenu={(e) => onItemContext(e, item, i)}
            onMouseDown={(e) => onItemMouseDown(e, item)}
          >
            <td className="name">
              <span className="cell">
                <ItemIcon item={item} />
                <span className="file-name">{item.name}</span>
              </span>
            </td>
            {atRoot ? (
              <td className="comment">{item.comment || "—"}</td>
            ) : (
              <>
                <td className="date">{formatDate(item.modified, locale)}</td>
                <td className="size">{item.isDir ? "—" : formatBytes(item.size)}</td>
              </>
            )}
          </tr>
        ))}
      </tbody>
    </table>
  );

  const gridView = (
    <div className="file-grid">
      {visible.map((item, i) => (
        <GridCell
          key={item.key}
          item={item}
          serverId={p.server?.id ?? ""}
          share={p.share ?? ""}
          selected={selection.has(item.key)}
          index={i}
          onClick={(e) => onItemClick(e, item, i)}
          onDoubleClick={() => activate(item)}
          onContextMenu={(e) => onItemContext(e, item, i)}
          onMouseDown={(e) => onItemMouseDown(e, item)}
        />
      ))}
    </div>
  );

  const emptyShares = (
    <>
      <div style={{ padding: "14px 18px" }}>
        <div className="callout info">
          <CircleAlert size={14} />
          <span>
            <strong>{t("browser.noShares")}</strong> {t("browser.noSharesHint")}
            {p.sharesError && <div className="hint">{p.sharesError}</div>}
          </span>
        </div>
      </div>
      <form
        className="manual-share"
        onSubmit={(e) => {
          e.preventDefault();
          if (manualShare.trim()) p.onOpenShare(manualShare.trim());
        }}
      >
        <input className="input" value={manualShare} onChange={(e) => setManualShare(e.target.value)} placeholder={t("browser.sharePlaceholder")} />
        <Button type="submit" disabled={!manualShare.trim()}>
          <FolderOpen size={14} />
          {t("browser.openShare")}
        </Button>
      </form>
    </>
  );

  let body: ReactNode = null;
  switch (p.view) {
    case "welcome":
      body = (
        <StateView icon={<Server size={28} />} title={t("browser.welcome")} text={t("browser.welcomeHint")}>
          {!p.hasServers && (
            <Button variant="primary" onClick={p.onAddServer}>
              <Plus size={14} />
              {t("sidebar.add")}
            </Button>
          )}
        </StateView>
      );
      break;
    case "connecting":
      body = <StateView icon={<LoaderCircle size={26} className="spin" />} title={t("browser.connecting", { name: p.server?.name ?? "" })} />;
      break;
    case "loading":
      body = loadingStale ? (
        <div className="stale">{p.viewMode === "grid" ? gridView : listView}</div>
      ) : (
        <StateView icon={<LoaderCircle size={26} className="spin" />} title={t("browser.loading")} />
      );
      break;
    case "connect-error":
    case "files-error":
      body = (
        <StateView
          error
          icon={<CircleAlert size={28} />}
          title={p.view === "connect-error" ? t("browser.connectFailed", { name: p.server?.name ?? "" }) : t("status.error")}
          text={p.errorText ?? undefined}
        >
          <Button variant="primary" onClick={p.onRetry}>
            <RefreshCw size={14} />
            {t("browser.retry")}
          </Button>
          {p.view === "connect-error" && (
            <Button onClick={p.onEditServer}>
              <Pencil size={14} />
              {t("server.edit")}
            </Button>
          )}
        </StateView>
      );
      break;
    case "shares":
    case "files":
      if (atRoot && p.shares.length === 0) body = emptyShares;
      else if (visible.length === 0)
        body = <StateView icon={filter ? <FolderSearch size={26} /> : <Inbox size={26} />} title={filter ? t("browser.noMatch", { q: filter }) : t("browser.empty")} />;
      else body = p.viewMode === "grid" ? gridView : listView;
      break;
  }

  const statusText = (() => {
    if (p.view === "loading") return t("browser.loading");
    if (!browsing) return "";
    if (selectedItems.length > 0) {
      return atRoot
        ? t("status.selectedShares", { n: selectedItems.length })
        : t("status.selected", { n: selectedItems.length, size: formatBytes(selectedBytes) });
    }
    if (atRoot) return t("status.shares", { n: visible.length });
    const base = t("status.items", { n: visible.length });
    return hiddenCount > 0 ? `${base} · ${t("status.hidden", { n: hiddenCount })}` : base;
  })();

  return (
    <div className="browser">
      <div className={`toolbar${p.toolbarEnd ? " with-window-controls" : ""}`} data-tauri-drag-region>
        <Button variant="ghost" iconOnly onClick={p.onBack} disabled={!p.canBack} title={t("browser.back")}>
          <ChevronLeft size={17} />
        </Button>
        <Button variant="ghost" iconOnly onClick={p.onForward} disabled={!p.canForward} title={t("browser.forward")}>
          <ChevronRight size={17} />
        </Button>
        <Button variant="ghost" iconOnly onClick={p.onUp} disabled={!p.share} title={t("browser.up")}>
          <ArrowUp size={16} />
        </Button>
        <div className="location" data-tauri-drag-region title={locationTitle}>
          {p.server && (
            <>
              <span className={`file-icon ${p.share ? "folder" : "share"}`}>{p.share ? <Folder size={15} /> : <HardDrive size={15} />}</span>
              <span className="location-name">{locationTitle}</span>
            </>
          )}
        </div>
        <div className={`search${p.view === "files" ? "" : " disabled"}`}>
          <Search size={14} />
          <input
            ref={searchRef}
            value={filter}
            disabled={p.view !== "files" && !atRoot}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={t("browser.search")}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setFilter("");
                (e.target as HTMLInputElement).blur();
              }
            }}
          />
          {filter && (
            <Button variant="ghost" size="sm" iconOnly onClick={() => setFilter("")}>
              <X size={12} />
            </Button>
          )}
        </div>
        <span className="toolbar-sep" />
        <Button
          variant="ghost"
          disabled={!canDownload}
          title={canDownload ? t("browser.download") : t("browser.downloadHint")}
          onClick={() => p.onDownload(selectedEntries.map(toItem), false)}
        >
          <Download size={15} />
          <span className="label">{t("browser.download")}</span>
        </Button>
        <Button
          variant="ghost"
          disabled={!canDownload}
          title={canDownload ? t("browser.downloadTo") : t("browser.downloadHint")}
          onClick={() => p.onDownload(selectedEntries.map(toItem), true)}
        >
          <FolderDown size={15} />
          <span className="label">{t("browser.downloadTo")}</span>
        </Button>
        <Button
          variant="ghost"
          className="upload-btn"
          disabled={!p.share || p.view !== "files"}
          title={p.share ? t("browser.upload") : t("browser.uploadHint")}
          onClick={(e) => {
            const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
            setUploadMenu({ x: r.left, y: r.bottom + 4 });
          }}
        >
          <Upload size={15} />
          <span className="label">{t("browser.upload")}</span>
          <ChevronDown size={12} />
        </Button>
        <span className="toolbar-sep" />
        <div className={`segmented${hasServer ? "" : " disabled"}`}>
          <button type="button" className={`btn${p.viewMode === "list" ? " on" : ""}`} onClick={() => p.onViewMode("list")} title={t("browser.viewList")} disabled={!hasServer}>
            <List size={15} />
          </button>
          <button type="button" className={`btn${p.viewMode === "grid" ? " on" : ""}`} onClick={() => p.onViewMode("grid")} title={t("browser.viewGrid")} disabled={!hasServer}>
            <LayoutGrid size={15} />
          </button>
        </div>
        {p.toolbarExtra}
        <Button variant="ghost" iconOnly onClick={p.onRefresh} disabled={!hasServer} title={t("browser.refresh")}>
          <RefreshCw size={15} />
        </Button>
        {p.toolbarEnd}
      </div>

      {p.view === "loading" && <div className="loading-bar" aria-hidden="true" />}
      <div
        className="content"
        tabIndex={0}
        onKeyDown={onKeyDown}
        ref={contentRef}
        onClick={onContentClick}
        onMouseDown={onContentMouseDown}
        onMouseMove={onContentMouseMove}
        onMouseUp={onContentMouseUp}
        onMouseLeave={onContentMouseUp}
      >
        {body}
        {marquee && <div className="marquee" style={{ left: marquee.x, top: marquee.y, width: marquee.w, height: marquee.h }} />}
      </div>
      {showPathbar && (
        <div className="pathbar">
          <div className="segs">
            {pathSegments.map((seg, i) => (
              <span key={i} style={{ display: "contents" }}>
                {i > 0 && (
                  <span className="sep">
                    <ChevronRight size={12} />
                  </span>
                )}
                <button type="button" className={`seg${i === pathSegments.length - 1 ? " current" : ""}`} onClick={seg.go} title={seg.label}>
                  {seg.icon}
                  <span className="seg-label">{seg.label}</span>
                </button>
              </span>
            ))}
          </div>
          <span className="status">{statusText}</span>
        </div>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.item)} onClose={() => setMenu(null)} />}
      {uploadMenu && (
        <ContextMenu
          x={uploadMenu.x}
          y={uploadMenu.y}
          items={[
            { label: t("browser.uploadFiles"), icon: <Upload size={14} />, onClick: () => p.onUpload(false) },
            { label: t("browser.uploadFolder"), icon: <FolderUp size={14} />, onClick: () => p.onUpload(true) },
          ]}
          onClose={() => setUploadMenu(null)}
        />
      )}
    </div>
  );
}
