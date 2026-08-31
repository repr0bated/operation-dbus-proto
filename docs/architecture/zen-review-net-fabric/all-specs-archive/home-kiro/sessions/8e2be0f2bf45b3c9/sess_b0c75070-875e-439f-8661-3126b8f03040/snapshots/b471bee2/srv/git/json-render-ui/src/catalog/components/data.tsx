interface KvRenderProps {
  element: { props: { label: string; value: unknown; kind?: string | null } };
}

export function Kv({ element }: KvRenderProps) {
  const { label, value } = element.props;
  const display =
    value === null || value === undefined
      ? "\u2014"
      : typeof value === "object"
        ? JSON.stringify(value)
        : String(value);

  return (
    <div className="flex items-baseline justify-between py-1 text-sm">
      <span className="text-neutral-400">{label}</span>
      <span className="text-neutral-100 font-mono text-xs">{display}</span>
    </div>
  );
}

interface TextRenderProps {
  element: { props: { text: string; className?: string | null } };
}

export function Text({ element }: TextRenderProps) {
  return <p className={element.props.className ?? "text-sm text-neutral-300"}>{element.props.text}</p>;
}

interface BadgeRenderProps {
  element: { props: { label: string; tone?: "default" | "ok" | "warn" | "danger" | "info" | null } };
}

const TONE_CLASSES: Record<string, string> = {
  default: "bg-neutral-800 text-neutral-300",
  ok: "bg-emerald-900/60 text-emerald-300",
  warn: "bg-amber-900/60 text-amber-300",
  danger: "bg-red-900/60 text-red-300",
  info: "bg-sky-900/60 text-sky-300",
};

export function Badge({ element }: BadgeRenderProps) {
  const { label, tone } = element.props;
  const cls = TONE_CLASSES[tone ?? "default"] ?? TONE_CLASSES.default;
  return (
    <span className={`inline-block px-2 py-0.5 rounded text-xs font-medium ${cls}`}>
      {label}
    </span>
  );
}

interface StatRenderProps {
  element: {
    props: {
      label: string;
      value: string;
      unit?: string | null;
      trend?: "up" | "down" | "flat" | null;
    };
  };
}

const TREND_ICON: Record<string, string> = { up: "\u2191", down: "\u2193", flat: "\u2192" };

export function Stat({ element }: StatRenderProps) {
  const { label, value, unit, trend } = element.props;
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-xs text-neutral-500">{label}</span>
      <span className="text-lg font-semibold text-neutral-100">
        {value}
        {unit && <span className="text-sm text-neutral-400 ml-1">{unit}</span>}
        {trend && <span className="text-sm ml-1">{TREND_ICON[trend] ?? ""}</span>}
      </span>
    </div>
  );
}
