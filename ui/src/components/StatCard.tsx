type Tone = "default" | "accent" | "ok" | "warn" | "danger" | "info";

const toneClass: Record<Tone, string> = {
  default: "",
  accent: "hx-stat--accent",
  ok: "hx-stat--ok",
  warn: "hx-stat--warn",
  danger: "hx-stat--danger",
  info: "hx-stat--info",
};

type StatCardProps = {
  label: string;
  value: string | number;
  sublabel?: string;
  tone?: Tone;
};

export function StatCard({ label, value, sublabel, tone = "default" }: StatCardProps) {
  return (
    <div className={`hx-stat ${toneClass[tone]}`}>
      <span className="hx-stat-label">{label}</span>
      <span className="hx-stat-value">{value}</span>
      {sublabel && <span className="hx-stat-sub">{sublabel}</span>}
    </div>
  );
}

type StatGridProps = {
  children: React.ReactNode;
};

export function StatGrid({ children }: StatGridProps) {
  return <div className="hx-stat-grid">{children}</div>;
}
