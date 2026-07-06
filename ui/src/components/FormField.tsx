import type { InputHTMLAttributes, SelectHTMLAttributes, TextareaHTMLAttributes, ReactNode } from "react";

type FieldWrapperProps = {
  label: string;
  full?: boolean;
  hint?: string;
  children: ReactNode;
};

export function FormField({ label, full = false, hint, children }: FieldWrapperProps) {
  return (
    <label className={`hx-field ${full ? "hx-field--full" : ""}`}>
      <span className="hx-field-label">{label}</span>
      {children}
      {hint && <span className="hx-field-hint">{hint}</span>}
    </label>
  );
}

type InputProps = InputHTMLAttributes<HTMLInputElement>;

export function Input(props: InputProps) {
  return <input className="hx-input" {...props} />;
}

type SelectProps = SelectHTMLAttributes<HTMLSelectElement> & {
  children: ReactNode;
};

export function Select({ children, ...rest }: SelectProps) {
  return (
    <select className="hx-select" {...rest}>
      {children}
    </select>
  );
}

type TextareaProps = TextareaHTMLAttributes<HTMLTextAreaElement>;

export function Textarea(props: TextareaProps) {
  return <textarea className="hx-textarea" {...props} />;
}

type CheckboxFieldProps = {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  full?: boolean;
};

export function CheckboxField({ label, checked, onChange, full = false }: CheckboxFieldProps) {
  return (
    <label className={`hx-field hx-field--row ${full ? "hx-field--full" : ""}`}>
      <input
        type="checkbox"
        className="hx-checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="hx-field-label">{label}</span>
    </label>
  );
}

type FormGridProps = {
  children: ReactNode;
};

export function FormGrid({ children }: FormGridProps) {
  return <div className="hx-form-grid">{children}</div>;
}
