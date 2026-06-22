import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

const variantClass: Record<Variant, string> = {
  primary: "hx-btn--primary",
  secondary: "hx-btn--secondary",
  ghost: "hx-btn--ghost",
  danger: "hx-btn--danger",
};

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  children: ReactNode;
};

export function Button({
  variant = "primary",
  children,
  className = "",
  ...rest
}: ButtonProps) {
  return (
    <button className={`hx-btn ${variantClass[variant]} ${className}`} {...rest}>
      {children}
    </button>
  );
}
