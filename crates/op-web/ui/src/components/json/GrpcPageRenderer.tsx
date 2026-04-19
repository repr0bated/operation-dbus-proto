/**
 * ⚖️ GrpcPageRenderer — Schema-driven gRPC page element renderer.
 *
 * Accepts a live gRPC-Web stream of PluginSchema state and renders
 * each plugin's page elements through the SchemaRenderer generative UI.
 * This is the JSON renderer that bridges gRPC → page elements per the
 * Accountability Loop architecture.
 *
 * Requirements:
 *   - The Absolute Base: PluginSchema is the single source of truth.
 *     GUI is a direct JSON render of the schema — no legacy DB reads.
 *   - 1:1 Direct Read: State is projected from SchemaEngine shared
 *     memory via gRPC-Web stream, not polled or fetched from SQL.
 *   - The Accountability Loop: Ghostbridge headers (Trace ID, Footprint)
 *     are bound to every rendered element for auditability.
 *
 * Design:
 *   - Uses useGrpcPageStream hook to subscribe to PluginService.GetSchema
 *     (unary) + StateSync.Subscribe (server-stream).
 *   - Each plugin's schema+data pair is rendered via SchemaRenderer.
 *   - Mutations flow back through StateSync.Mutate for schema-validated
 *     writes that trigger the event chain.
 */

import { useCallback, useState } from "react";
import { SchemaRenderer, SchemaErrorBoundary } from "@/components/json/SchemaRenderer";
import { JsonRenderer } from "@/components/json/JsonRenderer";
import { StatusDot, Pill } from "@/components/shell/Primitives";
import { cn } from "@/lib/utils";
import { stateSync } from "@/grpc/client";
import { useGrpcPageStream, type UseGrpcPageStreamOptions, type PageElement } from "@/hooks/use-grpc-page-stream";
import type { OperationType } from "@/grpc/types/operation";
import {
  Loader2, AlertTriangle, Database, Activity, Hash,
  ChevronRight, LayoutGrid, List,
} from "lucide-react";

// ── Types ───────────────────────────────────────────────────────────────────

export interface GrpcPageRendererProps {
  /** Options passed to the gRPC page stream hook */
  streamOptions?: UseGrpcPageStreamOptions;
  /** Render mode: "schema" uses SchemaRenderer, "raw" uses JsonRenderer */
  defaultMode?: "schema" | "raw";
  /** Whether mutations are allowed (sends Mutate RPCs back) */
  readOnly?: boolean;
  /** Actor ID for mutation audit trail */
  actorId?: string;
  /** Additional CSS class */
  className?: string;
  /** Callback when a mutation is sent */
  onMutate?: (pluginId: string, member: string, value: unknown) => void;
}

type ViewMode = "schema" | "raw";

// ── Component ───────────────────────────────────────────────────────────────

export function GrpcPageRenderer({
  streamOptions,
  defaultMode = "schema",
  readOnly = false,
  actorId = "ui:accountability",
  className,
  onMutate,
}: GrpcPageRendererProps) {
  const stream = useGrpcPageStream(streamOptions);
  const [viewMode, setViewMode] = useState<ViewMode>(defaultMode);
  const [collapsedPlugins, setCollapsedPlugins] = useState<Set<string>>(new Set());

  // ── Mutation handler: sends changes back via StateSync.Mutate ──
  const handleMutation = useCallback(
    async (pluginId: string, memberName: string, value: unknown) => {
      if (readOnly) return;

      try {
        await stateSync.mutate({
          pluginId,
          objectPath: "",
          operation: 1 as OperationType, // SET_PROPERTY
          memberName,
          value,
          actorId,
          capabilityId: "",
          idempotencyKey: `${pluginId}-${memberName}-${Date.now()}`,
        });
        onMutate?.(pluginId, memberName, value);
      } catch (err) {
        console.error("[GrpcPageRenderer] Mutation failed:", err);
      }
    },
    [readOnly, actorId, onMutate],
  );

  const togglePlugin = (pluginId: string) => {
    setCollapsedPlugins((prev) => {
      const next = new Set(prev);
      next.has(pluginId) ? next.delete(pluginId) : next.add(pluginId);
      return next;
    });
  };

  // ── Loading state ──
  if (stream.loading) {
    return (
      <div className={cn("flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground", className)}>
        <Loader2 className="h-4 w-4 animate-spin" />
        Fetching PluginSchema via gRPC…
      </div>
    );
  }

  // ── Error state ──
  if (stream.error && !stream.streamConnected && Object.keys(stream.elements).length === 0) {
    return (
      <div className={cn("flex flex-col items-center justify-center gap-3 py-12", className)}>
        <AlertTriangle className="h-5 w-5 text-danger" />
        <span className="text-sm text-danger font-medium">{stream.error}</span>
        <span className="text-xs text-muted-foreground">
          Verify op-grpc-bridge is running on port 50051.
        </span>
      </div>
    );
  }

  // ── Empty state ──
  if (stream.elementOrder.length === 0 && !stream.loading) {
    return (
      <div className={cn("flex flex-col items-center justify-center gap-3 py-12 text-muted-foreground", className)}>
        <Database className="h-5 w-5" />
        <span className="text-sm">No page elements received from gRPC stream.</span>
        <span className="text-xs">Waiting for PluginSchema state from SchemaEngine…</span>
      </div>
    );
  }

  return (
    <div className={cn("flex flex-col gap-0", className)}>
      {/* Stream status bar */}
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-border bg-[hsl(var(--bg-elevated))]">
        <div className="flex items-center gap-3 text-xs">
          <div className="flex items-center gap-1.5">
            <StatusDot status={stream.streamConnected ? "ok" : "error"} />
            <span className="text-muted-foreground">
              {stream.streamConnected ? "Stream live" : "Disconnected"}
            </span>
          </div>
          <div className="flex items-center gap-1.5 text-muted-foreground">
            <Activity className="h-3 w-3" />
            <span className="font-mono">{stream.mutationCount}</span> mutations
          </div>
          {stream.traceId && (
            <div className="flex items-center gap-1.5 text-muted-foreground">
              <Hash className="h-3 w-3" />
              <span className="font-mono truncate max-w-[140px]">{stream.traceId}</span>
            </div>
          )}
        </div>

        {/* View mode toggle */}
        <div className="flex rounded-md border border-border bg-secondary p-0.5">
          {([
            { mode: "schema" as const, icon: LayoutGrid, label: "Schema" },
            { mode: "raw" as const, icon: List, label: "Raw" },
          ]).map(({ mode, icon: Icon, label }) => (
            <button
              key={mode}
              onClick={() => setViewMode(mode)}
              className={cn(
                "px-2 py-0.5 text-[11px] rounded-sm transition-colors flex items-center gap-1",
                viewMode === mode
                  ? "bg-primary/15 text-primary font-medium"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Icon className="h-3 w-3" />
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Error banner (non-fatal) */}
      {stream.error && (
        <div className="flex items-center gap-2 px-3 py-1.5 bg-danger/5 border-b border-danger/20 text-xs text-danger">
          <AlertTriangle className="h-3 w-3 shrink-0" />
          {stream.error}
        </div>
      )}

      {/* Plugin element cards */}
      <div className="flex-1 overflow-auto">
        {stream.elementOrder.map((pluginId) => {
          const element = stream.elements[pluginId];
          if (!element) return null;

          const isCollapsed = collapsedPlugins.has(pluginId);
          const propCount = Object.keys(element.data).length;

          return (
            <div key={pluginId} className="border-b border-border">
              {/* Plugin header */}
              <button
                onClick={() => togglePlugin(pluginId)}
                className="w-full flex items-center justify-between px-3 py-2 hover:bg-muted/20 transition-colors"
              >
                <div className="flex items-center gap-2 text-xs">
                  <ChevronRight
                    className={cn(
                      "h-3.5 w-3.5 text-muted-foreground transition-transform",
                      !isCollapsed && "rotate-90",
                    )}
                  />
                  <span className="font-semibold text-foreground">{pluginId}</span>
                  <span className="text-muted-foreground">
                    {propCount} {propCount === 1 ? "property" : "properties"}
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  {element.effectiveHash && (
                    <span className="text-[10px] font-mono text-muted-foreground truncate max-w-[120px]">
                      {element.effectiveHash}
                    </span>
                  )}
                  <Pill variant="default">
                    <span className="text-[10px] font-mono">evt#{element.lastEventId}</span>
                  </Pill>
                </div>
              </button>

              {/* Plugin content */}
              {!isCollapsed && (
                <div className="px-3 pb-3">
                  <SchemaErrorBoundary>
                    {viewMode === "schema" ? (
                      <SchemaRenderer
                        schema={element.schema}
                        data={element.data}
                        readOnly={readOnly}
                        onChange={
                          readOnly
                            ? undefined
                            : (updated) => {
                                // Diff and send individual property mutations
                                if (updated && typeof updated === "object" && !Array.isArray(updated)) {
                                  const obj = updated as Record<string, unknown>;
                                  for (const [key, val] of Object.entries(obj)) {
                                    if (element.data[key] !== val) {
                                      handleMutation(pluginId, key, val);
                                    }
                                  }
                                }
                              }
                        }
                      />
                    ) : (
                      <JsonRenderer data={element.data} />
                    )}
                  </SchemaErrorBoundary>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
