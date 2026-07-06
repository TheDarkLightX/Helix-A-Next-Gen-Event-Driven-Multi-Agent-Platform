import type { ReactNode } from "react";

type PanelSpan = 3 | 4 | 5 | 6 | 7 | 8 | 12;

const spanClass: Record<PanelSpan, string> = {
  3: "col-span-3",
  4: "col-span-4",
  5: "col-span-5",
  6: "col-span-6",
  7: "col-span-7",
  8: "col-span-8",
  12: "col-span-12",
};

type PanelProps = {
  title?: string;
  eyebrow?: string;
  span?: PanelSpan;
  hero?: boolean;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
};

export function Panel({
  title,
  eyebrow,
  span = 6,
  hero = false,
  actions,
  children,
  className = "",
}: PanelProps) {
  const classes = [
    "hx-panel",
    spanClass[span],
    hero ? "hx-panel-hero" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <article className={classes}>
      {(title || eyebrow || actions) && (
        <header className="hx-panel-header">
          <div className="hx-panel-heading">
            {eyebrow && <span className="hx-eyebrow">{eyebrow}</span>}
            {title && <h3 className="hx-panel-title">{title}</h3>}
          </div>
          {actions && <div className="hx-panel-actions">{actions}</div>}
        </header>
      )}
      <div className="hx-panel-body">{children}</div>
    </article>
  );
}
