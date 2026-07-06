type BadgeTone = "default" | "ok" | "warn" | "danger" | "info" | "neutral" | "accent";

const toneClass: Record<BadgeTone, string> = {
  default: "hx-badge--default",
  ok: "hx-badge--ok",
  warn: "hx-badge--warn",
  danger: "hx-badge--danger",
  info: "hx-badge--info",
  neutral: "hx-badge--neutral",
  accent: "hx-badge--default",
};

type BadgeProps = {
  tone?: BadgeTone;
  children: React.ReactNode;
  title?: string;
};

export function Badge({ tone = "default", children, title }: BadgeProps) {
  return (
    <span className={`hx-badge ${toneClass[tone]}`} title={title}>
      {children}
    </span>
  );
}

type TagProps = {
  children: React.ReactNode;
};

export function Tag({ children }: TagProps) {
  return <span className="hx-tag">{children}</span>;
}

export function severityTone(severity: string): BadgeTone {
  const s = severity.toLowerCase();
  if (s === "critical" || s === "high") return "danger";
  if (s === "medium") return "warn";
  if (s === "low") return "ok";
  return "neutral";
}
