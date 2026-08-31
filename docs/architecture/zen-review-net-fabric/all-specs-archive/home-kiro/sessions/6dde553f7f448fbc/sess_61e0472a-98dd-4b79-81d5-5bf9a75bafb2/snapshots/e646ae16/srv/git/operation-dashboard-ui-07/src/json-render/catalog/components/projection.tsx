import type { ReactNode } from "react";
import { useStateValue } from "@json-render/react";
import type { El } from "./types";

function preview(value: unknown): string {
  if (value === null || value === undefined) return "\u2014";
  if (typeof value === "boolean" || typeof value === "number") return String(value);
  if (typeof value === "string") return value === "" ? '""' : value;
  try {
    const text = JSON.stringify(value, null, 2);
    return text.length > 600 ? `${text.slice(0, 600)}\n…` : text;
  } catch {
    return String(value);
  }
}

const ROLE_TONE: Record<string, string> = {
  surface: "border-sky-800 bg-sky-950/30",
  "display-value": "border-neutral-800 bg-neutral-900/40",
  "record-view": "border-neutral-800 bg-neutral-900/40",
  "state-flag": "border-neutral-800 bg-neutral-900/40",
  "collection-view": "border-neutral-800 bg-neutral-900/40",
  "value-list": "border-neutral-800 bg-neutral-900/40",
  "validation-carrier": "border-neutral-800 bg-neutral-950/50",
  "text-control": "border-amber-900/60 bg-amber-950/20",
  "record-editor": "border-amber-900/60 bg-amber-950/20",
  "editable-collection": "border-amber-900/60 bg-amber-950/20",
  "structured-control": "border-amber-900/60 bg-amber-950/20",
  "binary-control": "border-amber-900/60 bg-amber-950/20",
  "numeric-control": "border-amber-900/60 bg-amber-950/20",
  "multi-choice": "border-amber-900/60 bg-amber-950/20",
  "hydration-source": "border-neutral-800 bg-neutral-900/40",
  "trigger-binding": "border-violet-900/60 bg-violet-950/20",
  "repeat-binding": "border-neutral-800 bg-neutral-900/40",
};

type ProjectionProps = {
  pluginId: string;
  kind: "field" | "method" | "schema";
  field: string;
  label: string | null;
  subid: string;
  role: string;
};

/**
 * Render object/array data as structured children instead of raw JSON.
 * Arrays render each item; objects render key-value pairs.
 */
function renderStructuredValue(value: unknown, depth = 0): ReactNode {
  if (value === null || value === undefined) {
    return <span className="text-neutral-500 italic">—</span>;
  }

  if (typeof value === "boolean") {
    return (
      <span className={value ? "text-green-400" : "text-red-400"}>
        {value ? "true" : "false"}
      </span>
    );
  }

  if (typeof value === "number") {
    return <span className="text-cyan-400">{value}</span>;
  }

  if (typeof value === "string") {
    return <span className="text-amber-300">{value || '""'}</span>;
  }

  if (Array.isArray(value)) {
    if (value.length === 0) {
      return <span className="text-neutral-500 italic">[]</span>;
    }
    return (
      <div className="space-y-1 pl-2 border-l border-neutral-700/50">
        {value.map((item, i) => (
          <div key={i} className="flex gap-2">
            <span className="text-neutral-600 text-[10px] font-mono shrink-0">[{i}]</span>
            <div className="flex-1 min-w-0">{renderStructuredValue(item, depth + 1)}</div>
          </div>
        ))}
      </div>
    );
  }

  if (typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) {
      return <span className="text-neutral-500 italic">{"{}"}</span>;
    }
    return (
      <div className="space-y-1 pl-2 border-l border-neutral-700/50">
        {entries.map(([k, v]) => (
          <div key={k} className="flex gap-2">
            <span className="text-violet-400 text-[11px] font-mono shrink-0">{k}:</span>
            <div className="flex-1 min-w-0">{renderStructuredValue(v, depth + 1)}</div>
          </div>
        ))}
      </div>
    );
  }

  return <span className="text-neutral-400">{String(value)}</span>;
}

function ProjectionRole({
  props,
  emit,
  children,
}: {
  props: ProjectionProps;
  emit: (event: string) => void;
  children?: ReactNode;
}) {
  const plugin = useStateValue(`/plugins/${props.pluginId}`);
  const fieldValue = useStateValue(`/plugins/${props.pluginId}/${props.field}`);
  const label = props.label ?? props.field;
  const tone = ROLE_TONE[props.role] ?? "border-neutral-800 bg-neutral-900/40";

  if (props.kind === "method") {
    return (
      <button
        type="button"
        onClick={() => emit("press")}
        className={`w-full text-left rounded-md border px-3 py-2 ${tone}`}
      >
        <div className="flex items-center justify-between gap-2">
          <span className="text-sm font-mono text-neutral-100">{label}</span>
          <span className="text-[10px] uppercase tracking-wider text-neutral-500">{props.role}</span>
        </div>
        <div className="text-[10px] font-mono text-neutral-500 mt-1 break-all">{props.subid}</div>
        {children && <div className="mt-2">{children}</div>}
      </button>
    );
  }

  if (props.kind === "schema") {
    return (
      <div className={`rounded-md border px-3 py-2 ${tone}`}>
        <div className="flex items-center justify-between gap-2">
          <span className="text-[10px] uppercase tracking-wider text-neutral-500">schema</span>
          <span className="text-[10px] uppercase tracking-wider text-neutral-500">{props.role}</span>
        </div>
        <div className="text-[10px] font-mono text-neutral-400 mt-1 break-all">{props.subid}</div>
        {children && <div className="mt-2">{children}</div>}
      </div>
    );
  }

  const value = plugin === undefined ? undefined : fieldValue;

  // If children are provided, render them instead of raw value
  if (children) {
    return (
      <div className={`rounded-md border px-3 py-2 ${tone}`}>
        <div className="flex items-center justify-between gap-2 mb-1">
          <span className="text-sm font-mono text-neutral-100">{label}</span>
          <span className="text-[10px] uppercase tracking-wider text-neutral-500">{props.role}</span>
        </div>
        <div className="text-[10px] font-mono text-neutral-500 mb-2 break-all">{props.subid}</div>
        <div className="mt-2">{children}</div>
      </div>
    );
  }

  // Render structured value instead of raw JSON preview
  return (
    <div className={`rounded-md border px-3 py-2 ${tone}`}>
      <div className="flex items-center justify-between gap-2 mb-1">
        <span className="text-sm font-mono text-neutral-100">{label}</span>
        <span className="text-[10px] uppercase tracking-wider text-neutral-500">{props.role}</span>
      </div>
      <div className="text-[10px] font-mono text-neutral-500 mb-2 break-all">{props.subid}</div>
      <div className="text-[11px] font-mono max-h-60 overflow-auto">
        {plugin === undefined ? (
          <span className="text-neutral-500 italic">waiting for stream…</span>
        ) : (
          renderStructuredValue(value)
        )}
      </div>
    </div>
  );
}

export const SurfaceEl: El<"surface"> = ProjectionRole;
export const DisplayValueEl: El<"displayValue"> = ProjectionRole;
export const StateFlagEl: El<"stateFlag"> = ProjectionRole;
export const CollectionViewEl: El<"collectionView"> = ProjectionRole;
export const RecordViewEl: El<"recordView"> = ProjectionRole;
export const ValueListEl: El<"valueList"> = ProjectionRole;
export const BinaryControlEl: El<"binaryControl"> = ProjectionRole;
export const TextControlEl: El<"textControl"> = ProjectionRole;
export const NumericControlEl: El<"numericControl"> = ProjectionRole;
export const MultiChoiceEl: El<"multiChoice"> = ProjectionRole;
export const EditableCollectionEl: El<"editableCollection"> = ProjectionRole;
export const RecordEditorEl: El<"recordEditor"> = ProjectionRole;
export const StructuredControlEl: El<"structuredControl"> = ProjectionRole;
export const ValidationCarrierEl: El<"validationCarrier"> = ProjectionRole;
export const HydrationSourceEl: El<"hydrationSource"> = ProjectionRole;
export const TriggerBindingEl: El<"triggerBinding"> = ProjectionRole;
export const RepeatBindingEl: El<"repeatBinding"> = ProjectionRole;
