type CodeBlockProps = {
  children: string;
  maxHeight?: string;
  label?: string;
};

export function CodeBlock({ children, maxHeight = "320px", label }: CodeBlockProps) {
  return (
    <div className="hx-code-block">
      {label && <span className="hx-code-label">{label}</span>}
      <pre className="hx-code-pre" style={{ maxHeight }}>
        {children}
      </pre>
    </div>
  );
}
