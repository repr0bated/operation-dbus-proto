import { useState, useMemo } from "react";
import { PageHeader, Card } from "@/components/shell/Primitives";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { EventTape } from "@/components/json/EventTape";
import { useEventStore } from "@/stores/event-store";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ChevronRight, FolderTree, Play, FileCode2 } from "lucide-react";
import { cn } from "@/lib/utils";

interface DbusObject {
  path: string;
  interfaces: DbusInterface[];
}

interface DbusInterface {
  name: string;
  methods: DbusMethod[];
  properties: Record<string, { type: string; access: "read" | "readwrite"; value: unknown }>;
  signals: string[];
}

interface DbusMethod {
  name: string;
  args: { name: string; type: string; direction: "in" | "out" }[];
}

const DBUS_SIG_LABELS: Record<string, string> = { s: "string", b: "boolean", i: "int32", u: "uint32", d: "double", a: "array", o: "object_path" };

function methodSchema(method: DbusMethod): Record<string, unknown> {
  const inArgs = method.args.filter((a) => a.direction === "in");
  const props: Record<string, unknown> = {};
  for (const arg of inArgs) {
    const baseType = arg.type.replace(/^a/, "");
    if (baseType === "b") props[arg.name] = { type: "boolean", title: `${arg.name} (${DBUS_SIG_LABELS[baseType] ?? arg.type})` };
    else if (["i", "u", "d"].includes(baseType)) props[arg.name] = { type: "number", title: `${arg.name} (${DBUS_SIG_LABELS[baseType] ?? arg.type})` };
    else props[arg.name] = { type: "string", title: `${arg.name} (${DBUS_SIG_LABELS[baseType] ?? arg.type})` };
  }
  return { type: "object", properties: props };
}

function propertySchema(properties: DbusInterface["properties"]): { schema: Record<string, unknown>; data: Record<string, unknown>; mutableKeys: Set<string> } {
  const props: Record<string, unknown> = {};
  const data: Record<string, unknown> = {};
  const mutableKeys = new Set<string>();
  for (const [k, v] of Object.entries(properties)) {
    data[k] = v.value;
    if (v.access === "readwrite") mutableKeys.add(k);
    if (v.type === "b") props[k] = { type: "boolean", title: k, readOnly: v.access === "read" };
    else if (["i", "u", "d"].includes(v.type)) props[k] = { type: "number", title: k, readOnly: v.access === "read" };
    else props[k] = { type: "string", title: k, readOnly: v.access === "read" };
  }
  return { schema: { type: "object", properties: props }, data, mutableKeys };
}

export default function InspectorPage() {
  const { events, latestState } = useEventStore();
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [selectedIface, setSelectedIface] = useState<string | null>(null);
  const [methodArgs, setMethodArgs] = useState<Record<string, Record<string, unknown>>>({});
  const [callResults, setCallResults] = useState<Record<string, unknown>>({});
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  const tree = useMemo(() => {
    const live = latestState["dbus.tree"] ?? latestState["inspector:tree"] ?? latestState["dbus:tree"];
    if (Array.isArray(live)) return live as DbusObject[];
    return [] as DbusObject[];
  }, [latestState]);

  const selectedObj = tree.find((o) => o.path === selectedPath);
  const activeIface = selectedObj?.interfaces.find((i) => i.name === selectedIface) ?? selectedObj?.interfaces[0];

  const togglePath = (path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    });
  };

  const handleExecute = (method: DbusMethod) => {
    const args = methodArgs[method.name] ?? {};
    setCallResults((p) => ({ ...p, [method.name]: { status: "ok", result: `Simulated call: ${method.name}(${JSON.stringify(args)})`, ts: Date.now() } }));
  };

  return (
    <>
      <PageHeader title="D-Bus Inspector" subtitle="Hierarchical object browser with method execution." />

      <div className="grid grid-cols-1 lg:grid-cols-[320px_1fr] gap-4" style={{ minHeight: "calc(100vh - 200px)" }}>
        {/* Left: Tree */}
        <div className="rounded-lg border border-border bg-card overflow-hidden flex flex-col">
          <div className="px-3 py-2 border-b border-border">
            <span className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">Object Tree</span>
          </div>
          <ScrollArea className="flex-1">
            <div className="p-2 space-y-0.5">
              {tree.length === 0 && (
                <div className="text-xs text-muted-foreground text-center py-8">No D-Bus objects. Waiting for live data…</div>
              )}
              {tree.map((obj) => (
                <div key={obj.path}>
                  <button
                    onClick={() => { togglePath(obj.path); setSelectedPath(obj.path); setSelectedIface(obj.interfaces[0]?.name ?? null); }}
                    className={cn(
                      "w-full flex items-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-mono transition-colors",
                      selectedPath === obj.path ? "bg-primary/10 text-foreground" : "text-muted-foreground hover:bg-muted/30 hover:text-foreground"
                    )}
                  >
                    <ChevronRight className={cn("h-3 w-3 shrink-0 transition-transform", expandedPaths.has(obj.path) && "rotate-90")} />
                    <FolderTree className="h-3.5 w-3.5 shrink-0 text-primary" />
                    <span className="truncate">{obj.path}</span>
                  </button>
                  {expandedPaths.has(obj.path) && (
                    <div className="ml-6 space-y-0.5 mt-0.5">
                      {obj.interfaces.map((iface) => (
                        <button
                          key={iface.name}
                          onClick={() => { setSelectedPath(obj.path); setSelectedIface(iface.name); }}
                          className={cn(
                            "w-full flex items-center gap-1.5 px-2 py-1 rounded text-[11px] font-mono transition-colors",
                            selectedIface === iface.name && selectedPath === obj.path ? "bg-primary/10 text-primary" : "text-muted-foreground hover:text-foreground hover:bg-muted/20"
                          )}
                        >
                          <FileCode2 className="h-3 w-3 shrink-0" />
                          <span className="truncate">{iface.name.split(".").pop()}</span>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </ScrollArea>
        </div>

        {/* Right: Detail */}
        <div className="rounded-lg border border-border bg-card overflow-hidden flex flex-col">
          {activeIface ? (
            <ScrollArea className="flex-1">
              <div className="p-4 space-y-6">
                <div>
                  <h3 className="text-sm font-semibold text-foreground mb-1 font-mono">{activeIface.name}</h3>
                  <span className="text-xs text-muted-foreground">{selectedPath}</span>
                </div>

                {Object.keys(activeIface.properties).length > 0 && (() => {
                  const { schema, data } = propertySchema(activeIface.properties);
                  return (
                    <div>
                      <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-3">Properties</h4>
                      <SchemaRenderer schema={schema as any} data={data} onChange={() => {}} />
                    </div>
                  );
                })()}

                {activeIface.methods.length > 0 && (
                  <div>
                    <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-3">Methods</h4>
                    <div className="space-y-3">
                      {activeIface.methods.map((method) => {
                        const inArgs = method.args.filter((a) => a.direction === "in");
                        const outArgs = method.args.filter((a) => a.direction === "out");
                        const schema = methodSchema(method);
                        const result = callResults[method.name];
                        return (
                          <div key={method.name} className="rounded-md border border-border p-3 space-y-2">
                            <div className="flex items-center justify-between">
                              <div className="flex items-center gap-2">
                                <span className="font-mono text-sm font-semibold text-foreground">{method.name}</span>
                                {outArgs.map((a, i) => (
                                  <Badge key={i} variant="outline" className="text-[10px] font-mono">→ {a.type}</Badge>
                                ))}
                              </div>
                              <button
                                onClick={() => handleExecute(method)}
                                className="flex items-center gap-1 px-3 py-1.5 rounded-md bg-primary text-primary-foreground text-xs font-medium hover:bg-primary/90 transition-colors"
                              >
                                <Play className="h-3 w-3" /> Execute
                              </button>
                            </div>
                            {inArgs.length > 0 && (
                              <SchemaRenderer
                                schema={schema as any}
                                data={methodArgs[method.name] ?? {}}
                                onChange={(v) => setMethodArgs((p) => ({ ...p, [method.name]: v as Record<string, unknown> }))}
                              />
                            )}
                            {result && (
                              <pre className="font-mono text-xs bg-muted/30 rounded-md p-3 overflow-auto max-h-32 whitespace-pre-wrap text-muted-foreground">
                                {JSON.stringify(result, null, 2)}
                              </pre>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}

                {activeIface.signals.length > 0 && (
                  <div>
                    <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">Signals</h4>
                    <div className="flex flex-wrap gap-1.5">
                      {activeIface.signals.map((s) => (
                        <Badge key={s} variant="outline" className="text-[10px] font-mono">{s}</Badge>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </ScrollArea>
          ) : (
            <div className="flex-1 flex items-center justify-center text-sm text-muted-foreground">
              {tree.length === 0 ? "Waiting for D-Bus data…" : "Select an object from the tree"}
            </div>
          )}
        </div>
      </div>

      <div className="mt-6">
        <EventTape events={events} />
      </div>
    </>
  );
}
