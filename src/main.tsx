import React, { Component, type ErrorInfo, type ReactNode } from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { isTauri } from "./api";
import "./styles.css";

/** Ship frontend failures to the Rust log so they show up in `tauri dev`. */
function report(level: "error" | "warn", message: string) {
  if (!isTauri) return;
  invoke("frontend_log", { level, message }).catch(() => undefined);
}

window.addEventListener("error", (e) => report("error", `${e.message} @ ${e.filename}:${e.lineno}`));
window.addEventListener("unhandledrejection", (e) => report("error", `unhandled rejection: ${String(e.reason?.stack ?? e.reason)}`));

class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null; info: string }> {
  state = { error: null as Error | null, info: "" };
  static getDerivedStateFromError(error: Error) {
    return { error, info: "" };
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    report("error", `render crash: ${error.stack ?? error.message}\n${info.componentStack ?? ""}`);
    this.setState({ info: info.componentStack ?? "" });
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div style={{ padding: 24, fontFamily: "ui-monospace, Menlo, monospace", fontSize: 12, whiteSpace: "pre-wrap", userSelect: "text" }}>
        <h2 style={{ fontFamily: "system-ui", marginTop: 0 }}>Neat NAS hit an error</h2>
        {String(this.state.error.stack ?? this.state.error.message)}
        {"\n"}
        {this.state.info}
      </div>
    );
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
