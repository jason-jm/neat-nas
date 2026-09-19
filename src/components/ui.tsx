import {
  type ButtonHTMLAttributes,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { CircleAlert, LoaderCircle, X } from "lucide-react";

type Variant = "default" | "primary" | "ghost" | "danger" | "danger-primary";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: "sm" | "md";
  iconOnly?: boolean;
  loading?: boolean;
};

export function Button({ variant = "default", size = "md", iconOnly, loading, className, children, disabled, ...rest }: ButtonProps) {
  const classes = [
    "btn",
    variant === "danger-primary" ? "danger primary" : variant === "default" ? "" : variant,
    size === "sm" ? "sm" : "",
    iconOnly ? "icon" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" className={classes} disabled={disabled || loading} {...rest}>
      {loading ? <LoaderCircle size={14} className="spin" /> : children}
    </button>
  );
}

export function Modal({
  title,
  onClose,
  children,
  footer,
  width,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return createPortal(
    <div
      className="backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal" style={width ? { width } : undefined} role="dialog" aria-modal="true">
        <div className="modal-head">
          <h2>{title}</h2>
          <Button variant="ghost" iconOnly onClick={onClose} aria-label="Close">
            <X size={16} />
          </Button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

export type MenuItem =
  | { separator: true }
  | { separator?: false; label: string; icon?: ReactNode; onClick: () => void; danger?: boolean; disabled?: boolean };

export function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: MenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos({
      x: Math.max(8, Math.min(x, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(y, window.innerHeight - r.height - 8)),
    });
  }, [x, y]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && e.target instanceof Node && ref.current.contains(e.target)) return;
      onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  return createPortal(
    <div className="ctx" ref={ref} style={{ left: pos.x, top: pos.y }} role="menu">
      {items.map((item, i) =>
        item.separator ? (
          <div key={i} className="ctx-sep" />
        ) : (
          <div
            key={i}
            role="menuitem"
            className={`ctx-item${item.danger ? " danger" : ""}${item.disabled ? " disabled" : ""}`}
            onClick={() => {
              if (item.disabled) return;
              onClose();
              item.onClick();
            }}
          >
            {item.icon}
            {item.label}
          </div>
        ),
      )}
    </div>,
    document.body,
  );
}

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error";
}

export function Toasts({ toasts }: { toasts: Toast[] }) {
  if (toasts.length === 0) return null;
  return createPortal(
    <div className="toasts">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`}>
          {t.kind === "error" && <CircleAlert size={14} />}
          {t.text}
        </div>
      ))}
    </div>,
    document.body,
  );
}

export function StateView({
  icon,
  title,
  text,
  error,
  children,
}: {
  icon: ReactNode;
  title: string;
  text?: string;
  error?: boolean;
  children?: ReactNode;
}) {
  return (
    <div className={`state${error ? " error" : ""}`}>
      <div className="big-icon">{icon}</div>
      <h3>{title}</h3>
      {text && <p>{text}</p>}
      {children && <div className="actions">{children}</div>}
    </div>
  );
}
