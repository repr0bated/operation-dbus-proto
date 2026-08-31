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

function ProjectionRole({
  props,
  emit,
}: {
  props: ProjectionProps;
  emit: (event: string) => void;
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
      </div>
    );
  }

  const value = plugin === undefined ? undefined : fieldValue;
  const complex = value !== null && typeof value === "object";

  return (
    <div className={`rounded-md border px-3 py-2 ${tone}`}>
      <div className="flex items-center justify-between gap-2 mb-1">
        <span className="text-sm font-mono text-neutral-100">{label}</span>
        <span className="text-[10px] uppercase tracking-wider text-neutral-500">{props.role}</span>
      </div>
      <div className="text-[10px] font-mono text-neutral-500 mb-2 break-all">{props.subid}</div>
      <pre
        className={`text-[11px] font-mono whitespace-pre-wrap break-all max-h-40 overflow-auto ${
          complex ? "text-neutral-300" : "text-neutral-100"
        }`}
      >
        {plugin === undefined ? "waiting for stream…" : preview(value)}
      </pre>
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
