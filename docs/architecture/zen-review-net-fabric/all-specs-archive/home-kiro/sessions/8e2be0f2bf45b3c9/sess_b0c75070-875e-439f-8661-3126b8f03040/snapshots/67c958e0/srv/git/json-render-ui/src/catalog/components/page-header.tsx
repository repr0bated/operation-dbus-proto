interface PageHeaderRenderProps {
  element: { props: { title: string; subtitle?: string | null } };
}

export function PageHeader({ element }: PageHeaderRenderProps) {
  const { title, subtitle } = element.props;
  return (
    <div>
      <h1 className="text-xl font-semibold text-neutral-100">{title}</h1>
      {subtitle && <p className="text-sm text-neutral-400 mt-0.5">{subtitle}</p>}
    </div>
  );
}
