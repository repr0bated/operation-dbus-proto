import type { ReactNode } from "react";

interface ContainerProps {
  props: { className?: string | null };
  children?: ReactNode;
}

export function Container({ props, children }: ContainerProps) {
  return <div className={props.className ?? "flex flex-col gap-4"}>{children}</div>;
}

interface GridProps {
  props: { cols: number; gap?: number | null; className?: string | null };
  children?: ReactNode;
}

export function Grid({ props, children }: GridProps) {
  const style = {
    display: "grid",
    gridTemplateColumns: `repeat(${props.cols}, minmax(0, 1fr))`,
    gap: `${props.gap ?? 4}px`,
  };
  return (
    <div style={style} className={props.className ?? ""}>
      {children}
    </div>
  );
}

interface CardProps {
  props: { title?: string | null; className?: string | null };
  children?: ReactNode;
}

export function Card({ props, children }: CardProps) {
  return (
    <div className={`rounded-lg border border-neutral-800 bg-neutral-900/50 p-4 ${props.className ?? ""}`}>
      {props.title && (
        <h3 className="text-sm font-medium text-neutral-300 mb-3">{props.title}</h3>
      )}
      {children}
    </div>
  );
}
