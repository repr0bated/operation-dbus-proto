import { useStateValue } from "@json-render/react";
import { uiStore } from "@/store/ui-store";
import { toCatalogName } from "../stream-plugins";
import type { El } from "./types";

function preview(value: unknown): string {
  if (value === null || value === undefined) return "waiting…";
  if (typeof value !== "object") return String(value);
  try {
    const text = JSON.stringify(value);
    return text.length > 280 ? `${text.slice(0, 280)}…` : text;
  } catch {
    return String(value);
  }
}

function StreamObjectCard({
  pluginId,
  member,
  className,
  emit,
}: {
  pluginId: string;
  member?: string | null;
  className?: string | null;
  emit: (event: string) => void;
}) {
  const state = useStateValue(`/plugins/${pluginId}`) as Record<string, unknown> | undefined;
  const schemaHash = useStateValue(`/schemaHashes/${pluginId}`) as string | undefined;
  const selected = useStateValue("/shell/selectedPlugin") as string | null | undefined;
  const value = member && state ? state[member] : state;
  const members = state ? Object.keys(state) : [];
  const active = selected === pluginId;

  return (
    <button
      type="button"
      onClick={() => {
        uiStore.set("/shell/selectedPlugin", pluginId);
        emit("press");
      }}
      className={`w-full text-left rounded-lg border p-3 ${
        active
          ? "border-sky-700 bg-sky-950/40"
          : "border-neutral-800 bg-neutral-900/50 hover:border-neutral-600"
      } ${className ?? ""}`}
    >
      <div className="flex items-center justify-between gap-2 mb-2">
        <span className="text-sm font-medium text-neutral-100 font-mono">{pluginId}</span>
        <span className="text-[10px] text-neutral-500 font-mono">{toCatalogName(pluginId)}</span>
      </div>
      <div className="flex flex-wrap gap-1 mb-2">
        {schemaHash ? (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-neutral-800 text-neutral-400 font-mono">
            {schemaHash.slice(0, 12)}
          </span>
        ) : (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-neutral-800 text-neutral-500">
            waiting for contract
          </span>
        )}
        <span className="text-[10px] px-1.5 py-0.5 rounded bg-neutral-800 text-neutral-400">
          {members.length} fields
        </span>
      </div>
      <pre className="text-[11px] text-neutral-400 font-mono whitespace-pre-wrap break-all max-h-28 overflow-hidden">
        {preview(value)}
      </pre>
    </button>
  );
}

export const StreamObjectEl: El<"streamObject"> = ({ props, emit }) => (
  <StreamObjectCard
    pluginId={props.pluginId}
    member={props.member}
    className={props.className}
    emit={emit}
  />
);

export const StreamGridEl: El<"streamGrid"> = ({ props, emit }) => {
  const index = (useStateValue("/pluginIndex") as Array<{ id: string }> | undefined) ?? [];
  console.log(`[StreamGrid] renderering ${index.length} plugins:`, index.map(e => e.id));
  return (
    <div
      className={`grid gap-3 ${props.className ?? ""}`}
      style={{ gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))" }}
    >
      {index.length === 0 ? (
        <p className="text-sm text-neutral-500 italic">waiting for StateSync objects…</p>
      ) : (
        index.map((entry) => (
          <StreamObjectCard
            key={entry.id}
            pluginId={entry.id}
            emit={(event) => {
              if (event === "press") emit("select");
              else emit(event);
            }}
          />
        ))
      )}
    </div>
  );
};

/** Named catalog entry for one sealed plugin (fixed pluginId). */
export function makeStreamPluginEl(pluginId: string) {
  return ({
    props,
    emit,
  }: {
    props: { member?: string | null; className?: string | null };
    emit: (event: string) => void;
  }) => (
    <StreamObjectCard
      pluginId={pluginId}
      member={props.member}
      className={props.className}
      emit={emit}
    />
  );
}
