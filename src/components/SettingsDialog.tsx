import { useState } from "react";
import { CircleCheck, Download, FolderOpen, LoaderCircle, RefreshCw, RotateCcw } from "lucide-react";
import type { Settings, UpdateInfo, UpdateProgress } from "../types";
import { api, errorOf } from "../api";
import { useI18n, type LocalePref } from "../i18n";
import { Button, Modal } from "./ui";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "latest" }
  | { kind: "available"; info: UpdateInfo }
  | { kind: "installing"; info: UpdateInfo; progress: UpdateProgress }
  | { kind: "error"; message: string };

interface Props {
  settings: Settings | null;
  showHidden: boolean;
  onShowHidden: (on: boolean) => void;
  version: string;
  update: UpdateState;
  onCheckUpdate: () => void;
  onInstallUpdate: () => void;
  onClose: () => void;
  onSettingsChanged: (s: Settings) => void;
  onError: (text: string) => void;
}

export function SettingsDialog({ settings, showHidden, onShowHidden, version, update, onCheckUpdate, onInstallUpdate, onClose, onSettingsChanged, onError }: Props) {
  const { t, errorText, localePref, setLocalePref } = useI18n();
  const [busy, setBusy] = useState(false);

  const apply = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
      onSettingsChanged(await api.getSettings());
    } catch (e) {
      const err = errorOf(e);
      onError(errorText(err.code, err.message));
    } finally {
      setBusy(false);
    }
  };

  const choose = () =>
    apply(async () => {
      const dir = await api.pickFolder(settings?.downloadDir);
      if (dir) await api.setDownloadDir(dir);
    });

  const reset = () => apply(() => api.setDownloadDir(null));

  return (
    <Modal
      title={t("settings.title")}
      onClose={onClose}
      width={480}
      footer={
        <Button variant="primary" onClick={onClose}>
          {t("settings.close")}
        </Button>
      }
    >
      <div className="field">
        <label>{t("settings.downloadDir")}</label>
        <div className="settings-row">
          <span className="path" title={settings?.downloadDir ?? ""}>
            {settings?.downloadDir ?? "…"}
          </span>
          <Button onClick={choose} disabled={busy}>
            <FolderOpen size={14} />
            {t("settings.choose")}
          </Button>
          {settings?.downloadDirIsCustom && (
            <Button variant="ghost" iconOnly onClick={reset} disabled={busy} title={t("settings.reset")}>
              <RotateCcw size={14} />
            </Button>
          )}
        </div>
        <span className="hint">{t("settings.downloadDirHint")}</span>
      </div>
      <label className="check-row">
        <input type="checkbox" checked={showHidden} onChange={(e) => onShowHidden(e.target.checked)} />
        <span>
          {t("settings.showHidden")}
          <span className="hint" style={{ display: "block" }}>{t("settings.showHiddenHint")}</span>
        </span>
      </label>
      <div className="field">
        <label>{t("settings.language")}</label>
        <select className="input" value={localePref} onChange={(e) => setLocalePref(e.target.value as LocalePref)}>
          <option value="system">{t("settings.system")}</option>
          <option value="en">English</option>
          <option value="zh-CN">简体中文</option>
        </select>
      </div>
      <div className="field">
        <label>{t("settings.version", { v: version })}</label>
        <div className="settings-row">
          {update.kind === "available" || update.kind === "installing" ? (
            <span className="hint" style={{ flex: 1 }}>
              {update.kind === "installing"
                ? update.progress.phase === "downloading"
                  ? t("settings.downloading", { p: update.progress.total ? Math.round((update.progress.downloaded / update.progress.total) * 100) : 0 })
                  : update.progress.phase === "installing"
                    ? t("settings.installing")
                    : t("settings.restarting")
                : t("settings.updateAvailable", { v: update.info.version })}
              {update.kind === "available" && update.info.notes && <pre className="release-notes">{update.info.notes}</pre>}
            </span>
          ) : (
            <span className="hint" style={{ flex: 1 }}>
              {update.kind === "latest" && (
                <>
                  <CircleCheck size={13} style={{ verticalAlign: -2, marginRight: 4, color: "var(--success)" }} />
                  {t("settings.upToDate")}
                </>
              )}
              {update.kind === "error" && `${t("settings.updateFailed")} ${update.message}`}
            </span>
          )}
          {update.kind === "available" ? (
            <Button variant="primary" onClick={onInstallUpdate}>
              <Download size={14} />
              {t("settings.installUpdate")}
            </Button>
          ) : update.kind === "installing" ? (
            <Button disabled>
              <LoaderCircle size={14} className="spin" />
            </Button>
          ) : (
            <Button onClick={onCheckUpdate} loading={update.kind === "checking"}>
              <RefreshCw size={14} />
              {t("settings.checkUpdate")}
            </Button>
          )}
        </div>
      </div>
      {settings?.configPath && (
        <div className="field">
          <label>{t("settings.configPath")}</label>
          <span className="path settings-row" style={{ display: "block" }} title={settings.configPath}>
            {settings.configPath}
          </span>
        </div>
      )}
    </Modal>
  );
}
