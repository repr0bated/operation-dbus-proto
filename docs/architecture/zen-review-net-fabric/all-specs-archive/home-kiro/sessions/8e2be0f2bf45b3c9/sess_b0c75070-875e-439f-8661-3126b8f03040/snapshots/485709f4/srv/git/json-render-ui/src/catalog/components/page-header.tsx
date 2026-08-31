interface PageHeaderProps {
  props: {
    title: string;
    subtitle?: string | null;
  };
}

export function PageHeader({ props }: PageHeaderProps) {
  return (
    <div>
      <h1 className="text-xl font-semibold text-neutral-100">{props.title}</h1>
      {props.subtitle && (
        <p className="text-sm text-neutral-400 mt-0.5">{props.subtitle}</p>
      )}
    </div>
  );
}
