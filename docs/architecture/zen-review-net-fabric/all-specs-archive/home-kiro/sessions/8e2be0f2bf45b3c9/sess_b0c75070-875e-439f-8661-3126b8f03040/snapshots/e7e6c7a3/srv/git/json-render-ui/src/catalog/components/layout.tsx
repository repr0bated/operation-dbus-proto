import type { ReactNode } from "react";

interface ContainerRenderProps {
  element: { props: { className?: string | null } };
  children?: ReactNode;
}

export function Container({ element, children }: ContainerRenderProps) {
  return <div className={element.props?.className ?? "flex flex-col gap-4"}>{children}</div>;
}

interface GridRenderProps {
  element: { props: { cols: number; gap?: number | null; className?: string | null } };
  children?: ReactNode;
}

export function Grid({ element, children }: GridRenderProps) {
  const { cols, gap, className } = element.props;
  const style = {
    display: "grid",
    gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
    gap: `${gap ?? 16}px`,
  };
  return (
    <div style={style} className={className ?? ""}>
      {children}
    </div>
  );
}

interface CardRenderProps {
  element: { props: { title?: string | null; className?: string | null } };
  children?: ReactNode;
}

export function Card({ element, children }: CardRenderProps) {
  const { title, className } = element.props;
  return (
    <div className={`rounded-lg border border-neutral-800 bg-neutral-900/50 p-4 ${className ?? ""}`}>
      {title && <h3 className="text-sm font-medium text-neutral-300 mb-3">{title}</h3>}
      {children}
    </div>
  );
}
