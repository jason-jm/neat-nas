import { useCallback, useEffect, useState, type FormEvent } from "react";
import { CircleAlert, CircleCheck, Eye, EyeOff, HardDrive, LoaderCircle, Lock, Radar } from "lucide-react";
import type { DiscoveredServer, SavedServer, ServerInput } from "../types";
import { api, errorOf } from "../api";
import { useI18n } from "../i18n";
import { Button, Modal } from "./ui";

interface Props {
  mode: "add" | "edit";
  server?: SavedServer;
  onClose: () => void;
  onSaved: (server: SavedServer, isNew: boolean) => void;
}

export function AddServerDialog({ mode, server, onClose, onSaved }: Props) {
  const { t, errorText } = useI18n();
  const [name, setName] = useState(server?.name ?? "");
  const [host, setHost] = useState(server?.host ?? "");
  const [port, setPort] = useState(String(server?.port ?? 445));
  const [username, setUsername] = useState(server?.username ?? "");
  const [password, setPassword] = useState("");
  const [domain, setDomain] = useState(server?.domain ?? "");
  const [showPassword, setShowPassword] = useState(false);
  const [discovered, setDiscovered] = useState<DiscoveredServer[]>([]);
  const [scanning, setScanning] = useState(false);
  const [scanned, setScanned] = useState(false);
  const [picked, setPicked] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; text: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scan = useCallback(async () => {
    setScanning(true);
    try {
      setDiscovered(await api.discoverServers(2500));
    } catch {
      setDiscovered([]);
    } finally {
      setScanning(false);
      setScanned(true);
    }
  }, []);

  useEffect(() => {
    if (mode === "add") void scan();
  }, [mode, scan]);

  const pick = (d: DiscoveredServer) => {
    setHost(d.hostname || d.addresses[0] || "");
    setPort(String(d.port || 445));
    if (!name.trim()) setName(d.name);
    setPicked(d.hostname);
    setTestResult(null);
  };

  const input = (): ServerInput => ({
    name: name.trim() || undefined,
    host: host.trim(),
    port: Number(port) || 445,
    username: username.trim(),
    password,
    domain: domain.trim() || undefined,
  });

  const canSubmit = host.trim().length > 0 && username.trim().length > 0 && (password.length > 0 || mode === "edit");
  const canTest = host.trim().length > 0 && username.trim().length > 0 && password.length > 0;

  const test = async () => {
    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      const info = await api.testConnection(input());
      setTestResult({
        ok: true,
        text: info.sharesError
          ? t("add.testOkNoShares", { dialect: info.dialect, err: info.sharesError })
          : t("add.testOk", { dialect: info.dialect, n: info.shares.length }),
      });
    } catch (e) {
      const err = errorOf(e);
      setTestResult({ ok: false, text: errorText(err.code, err.message) });
    } finally {
      setTesting(false);
    }
  };

  const save = async (e?: FormEvent) => {
    e?.preventDefault();
    if (!canSubmit || saving) return;
    setSaving(true);
    setError(null);
    try {
      const saved = mode === "add" ? await api.addServer(input()) : await api.updateServer(server!.id, input());
      onSaved(saved, mode === "add");
    } catch (err) {
      const e2 = errorOf(err);
      setError(errorText(e2.code, e2.message));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={mode === "add" ? t("add.title") : t("add.editTitle")}
      onClose={onClose}
      footer={
        <>
          <span className="left">
            <Button onClick={test} disabled={!canTest} loading={testing}>
              {t("add.test")}
            </Button>
          </span>
          <Button variant="ghost" onClick={onClose}>
            {t("add.cancel")}
          </Button>
          <Button variant="primary" onClick={() => save()} disabled={!canSubmit} loading={saving}>
            {mode === "add" ? t("add.saveAndOpen") : t("add.save")}
          </Button>
        </>
      }
    >
      {mode === "add" && (
        <>
          <div className="subhead">
            <span>{t("add.discovered")}</span>
            <Button variant="ghost" size="sm" onClick={scan} disabled={scanning}>
              {scanning ? <LoaderCircle size={13} className="spin" /> : <Radar size={13} />}
              {scanning ? t("add.scanning") : t("add.rescan")}
            </Button>
          </div>
          {discovered.length > 0 ? (
            <div className="discovered">
              {discovered.map((d) => (
                <div
                  key={d.hostname}
                  className={`discovered-item${picked === d.hostname ? " selected" : ""}`}
                  onClick={() => pick(d)}
                >
                  <span className="icon">
                    <HardDrive size={16} />
                  </span>
                  <span className="meta">
                    <div className="name">{d.name}</div>
                    <div className="addr">
                      {d.hostname}
                      {d.addresses.length > 0 && ` · ${d.addresses.join(", ")}`}
                    </div>
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <div className="hint">{scanned && !scanning ? t("add.nothingFound") : t("add.scanning")}</div>
          )}
          <div className="subhead">
            <span>{t("add.manual")}</span>
          </div>
        </>
      )}

      <form onSubmit={save} style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <div className="input-row">
          <div className="field">
            <label>{t("add.host")}</label>
            <input
              className="input"
              value={host}
              onChange={(e) => {
                setHost(e.target.value);
                setPicked(null);
                setTestResult(null);
              }}
              placeholder={t("add.hostPlaceholder")}
              autoFocus={mode === "edit"}
              spellCheck={false}
              autoCapitalize="off"
            />
          </div>
          <div className="field narrow">
            <label>{t("add.port")}</label>
            <input className="input" value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))} inputMode="numeric" />
          </div>
        </div>
        <div className="input-row">
          <div className="field">
            <label>{t("add.username")}</label>
            <input
              className="input"
              value={username}
              onChange={(e) => {
                setUsername(e.target.value);
                setTestResult(null);
              }}
              spellCheck={false}
              autoCapitalize="off"
            />
          </div>
          <div className="field">
            <label>{t("add.password")}</label>
            <div className="password-wrap">
              <input
                className="input"
                type={showPassword ? "text" : "password"}
                value={password}
                onChange={(e) => {
                  setPassword(e.target.value);
                  setTestResult(null);
                }}
                placeholder={mode === "edit" ? "••••••••" : ""}
              />
              <Button variant="ghost" iconOnly onClick={() => setShowPassword((s) => !s)} tabIndex={-1}>
                {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
              </Button>
            </div>
            {mode === "edit" && <span className="hint">{t("add.keepPassword")}</span>}
          </div>
        </div>
        <div className="input-row">
          <div className="field">
            <label>{t("add.name")}</label>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder={t("add.namePlaceholder")} />
          </div>
          <div className="field">
            <label>{t("add.domain")}</label>
            <input className="input" value={domain} onChange={(e) => setDomain(e.target.value)} spellCheck={false} />
          </div>
        </div>
        <button type="submit" hidden />
      </form>

      {testResult && (
        <div className={`callout ${testResult.ok ? "success" : "error"}`}>
          {testResult.ok ? <CircleCheck size={14} /> : <CircleAlert size={14} />}
          <span>{testResult.text}</span>
        </div>
      )}
      {error && (
        <div className="callout error">
          <CircleAlert size={14} />
          <span>{error}</span>
        </div>
      )}
      <div className="hint" style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <Lock size={12} />
        {t("add.secure")}
      </div>
    </Modal>
  );
}
