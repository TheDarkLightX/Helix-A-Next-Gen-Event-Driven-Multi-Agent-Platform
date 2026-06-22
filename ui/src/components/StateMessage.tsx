import type { ReactNode } from "react";

type StateMessageProps = {
  icon?: string;
  title: string;
  description?: string;
  children?: ReactNode;
};

export function EmptyState({ icon = "empty", title, description, children }: StateMessageProps) {
  return (
    <div className="hx-state hx-state--empty">
      <span className="hx-state-icon" aria-hidden>{icon}</span>
      <p className="hx-state-title">{title}</p>
      {description && <p className="hx-state-desc">{description}</p>}
      {children}
    </div>
  );
}

export function LoadingState({ title = "Loading...", description }: { title?: string; description?: string }) {
  return (
    <div className="hx-state hx-state--loading">
      <span className="hx-spinner" aria-hidden />
      <p className="hx-state-title">{title}</p>
      {description && <p className="hx-state-desc">{description}</p>}
    </div>
  );
}

export function ErrorState({ title = "Something went wrong", description, children }: StateMessageProps) {
  return (
    <div className="hx-state hx-state--error">
      <span className="hx-state-icon" aria-hidden>!</span>
      <p className="hx-state-title">{title}</p>
      {description && <p className="hx-state-desc">{description}</p>}
      {children}
    </div>
  );
}

type StatusLineProps = {
  children: ReactNode;
  tone?: "default" | "ok" | "warn" | "danger";
};

export function StatusLine({ children, tone = "default" }: StatusLineProps) {
  const toneClass =
    tone === "ok"
      ? "hx-status--ok"
      : tone === "warn"
        ? "hx-status--warn"
        : tone === "danger"
          ? "hx-status--danger"
          : "";
  return <p className={`hx-status-line ${toneClass}`.trim()}>{children}</p>;
}
