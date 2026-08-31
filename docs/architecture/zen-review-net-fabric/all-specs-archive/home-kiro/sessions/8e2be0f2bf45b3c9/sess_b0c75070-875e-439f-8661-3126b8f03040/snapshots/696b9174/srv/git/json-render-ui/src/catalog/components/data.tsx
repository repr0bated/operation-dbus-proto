interface KvProps {
  props: { label: string; value: unknown; kind?: string | null };
}

export function Kv({ props }: KvProps) {
  const display =
    props.value === null || props.value === undefined
      ? "—"
      : typeof props.value === "object"
        ? JSON.stringify(props.value)
        : String(props.value);

  return (
    <div className="flex items-baseline justify-between py-1 text-sm">
      <span className="text-neutral-400">{props.label}</span>
      <span className="text-neutral-100 font-mono text-xs">{display}</span>
    </div>
  );
}

interface TextProps {
  props: { text: string; className?: string | null };
}

export function Text({ props }: TextProps) {
  return <p className={props.className ?? "text-sm text-neutral-300"}>{props.text}</p>;
}

interface BadgeProps {
  props: { label: string; tone?: "default" | "ok" | "warn" | "danger" | "info" | null };
}

const TONE_CLASSES: Record<string, string> = {
  default: "bg-neutral-800 text-neutral-300",
  ok: "bg-emerald-900/60 text-emerald-300",
  warn: "bg-amber-900/60 text-amber-300",
  danger: "bg-red-900/60 text-red-300",
  info: "bg-sky-900/60 text-sky-300",
};

export function Badge({ props }: BadgeProps) {
  const cls = TONE_CLASSES[props.tone ?? "default"] ?? TONE_CLASSES.default;
  return (
    <span className={`inline-block px-2 py-0.5 rounded text-xs font-medium ${cls}`}>
      {props.label}
    </span>
  );
}

interface StatProps {
  props: {
    label: string;
    value: string;
    unit?: string | null;
    trend?: "up" | "down" | "flat" | null;
  };
}

const TREND_ICON: Record<string, string> = { up: "\u2191", down: "\u2193", flat: "\u2192" };

export function Stat({ props }: StatProps) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-xs text-neutral-500">{props.label}</span>
      <span className="text-lg font-semibold text-neutral-100">
        {props.value}
        {props.unit && <span className="text-sm text-neutral-400 ml-1">{props.unit}</span>}
        {props.trend && (
          <span className="text-sm ml-1">{TREND_ICON[props.trend] ?? ""}</span>
        )}
      </span>
    </div>
  );
}
