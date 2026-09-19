import { useState, type MouseEvent } from "react";
import { HardDrive, Pencil, Plus, Settings, Trash2, Unplug } from "lucide-react";
import type { SavedServer } from "../types";
import { useI18n } from "../i18n";
import { Button, ContextMenu, type MenuItem } from "./ui";

export type ServerStatus = "idle" | "connecting" | "connected" | "error";

interface Props {
  servers: SavedServer[];
  selectedId: string | null;
  statuses: Record<string, ServerStatus>;
  isMac: boolean;
  onSelect: (id: string) => void;
  onAdd: () => void;
  onEdit: (server: SavedServer) => void;
  onRemove: (server: SavedServer) => void;
  onDisconnect: (server: SavedServer) => void;
  onSettings: () => void;
  updateAvailable?: boolean;
}

export function Sidebar(p: Props) {
  const { t } = useI18n();
  const [menu, setMenu] = useState<{ x: number; y: number; server: SavedServer } | null>(null);

  const openMenu = (e: MouseEvent, server: SavedServer) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, server });
  };

  const menuItems = (s: SavedServer): MenuItem[] => [
    { label: t("server.edit"), icon: <Pencil size={14} />, onClick: () => p.onEdit(s) },
    {
      label: t("server.disconnect"),
      icon: <Unplug size={14} />,
      onClick: () => p.onDisconnect(s),
      disabled: p.statuses[s.id] !== "connected",
    },
    { separator: true },
    { label: t("server.remove"), icon: <Trash2 size={14} />, danger: true, onClick: () => p.onRemove(s) },
  ];

  return (
    <aside className="sidebar">
      <div className={`sidebar-head${p.isMac ? " mac" : ""}`} data-tauri-drag-region>
        <Button variant="ghost" iconOnly title={t("sidebar.add")} onClick={p.onAdd}>
          <Plus size={16} />
        </Button>
      </div>
      <div className="server-list">
        {p.servers.length === 0 && (
          <div className="sidebar-empty">
            <strong>{t("sidebar.empty")}</strong>
            {t("sidebar.emptyHint")}
          </div>
        )}
        {p.servers.map((s) => (
          <div
            key={s.id}
            className={`server-item${s.id === p.selectedId ? " active" : ""}`}
            onClick={() => p.onSelect(s.id)}
            onContextMenu={(e) => openMenu(e, s)}
            title={`${s.username}@${s.host}:${s.port}`}
          >
            <span className="icon">
              <HardDrive size={15} />
            </span>
            <span className="meta">
              <div className="name">{s.name}</div>
              <div className="host">{s.host}</div>
            </span>
            <span className={`status-dot ${p.statuses[s.id] ?? "idle"}`} />
          </div>
        ))}
      </div>
      <div className="sidebar-foot">
        <Button variant="ghost" onClick={p.onSettings}>
          <Settings size={15} />
          {t("sidebar.settings")}
          {p.updateAvailable && <span className="badge update">{t("sidebar.updateBadge")}</span>}
        </Button>
      </div>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.server)} onClose={() => setMenu(null)} />}
    </aside>
  );
}
