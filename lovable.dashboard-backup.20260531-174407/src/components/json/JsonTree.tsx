import { useState } from "react";
import { cn } from "@/lib/utils";
import { ChevronRight, ChevronDown } from "lucide-react";

interface JsonTreeProps {
  data: unknown;
  depth?: number;
  maxDepth?: number;
  label?: string;
  className?: string;
}

function isObject(val: unknown): val is Record<string, unknown> {
  return typeof val === "object" && val !== null && !Array.isArray(val);
}

function isArray(val: unknown): val is unknown[] {
  return Array.isArray(val);
}

function getTypeColor(val: unknown): string {
  if (val === null) return "text-muted-foreground";
  if (typeof val === "boolean") return "text-amber-500";
  if (typeof val === "number") return "text-cyan-500";
  if (typeof val === "string") return "text-emerald-500";
  if (isArray(val)) return "text-blue-500";
  if (isObject(val)) return "text-purple-500";
  return "text-foreground";
}

function getPreview(val: unknown): string {
  if (val === null) return "null";
  if (typeof val === "boolean") return String(val);
  if (typeof val === "number") return String(val);
  if (typeof val === "string")
    return `"${val.slice(0, 40)}${val.length > 40 ? "..." : ""}"`;
  if (isArray(val)) return `[${val.length}]`;
  if (isObject(val)) return `{${Object.keys(val).length}}`;
  return String(val);
}

export function JsonTree({
  data,
  depth = 0,
  maxDepth = 8,
  label,
  className,
}: JsonTreeProps) {
  const [expanded, setExpanded] = useState(depth < 2);

  if (depth > maxDepth) {
    return <span className="text-muted-foreground">...</span>;
  }

  const indent = "ml-4 border-l border-border pl-2";

  if (isArray(data)) {
    if (data.length === 0) {
      return (
        <span className={cn("text-blue-500 font-mono text-xs", className)}>
          {label ? <span className="text-purple-400">{label}</span> : null} []
        </span>
      );
    }
    return (
      <div className={cn("font-mono text-xs", className)}>
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1 hover:opacity-70 transition-opacity"
        >
          {expanded ? (
            <ChevronDown className="h-3 w-3" />
          ) : (
            <ChevronRight className="h-3 w-3" />
          )}
          {label && <span className="text-purple-400">{label}</span>}
          <span className="text-blue-500">[{data.length}]</span>
        </button>
        {expanded && (
          <div className={indent}>
            {data.map((item, i) => (
              <div key={i} className="py-0.5">
                <span className="text-muted-foreground mr-2">{i}:</span>
                <JsonTree data={item} depth={depth + 1} maxDepth={maxDepth} />
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }

  if (isObject(data)) {
    const keys = Object.keys(data);
    if (keys.length === 0) {
      return (
        <span className={cn("text-purple-500 font-mono text-xs", className)}>
          {label ? <span className="text-purple-400">{label}</span> : null}{" "}
          {"{}"}
        </span>
      );
    }
    return (
      <div className={cn("font-mono text-xs", className)}>
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1 hover:opacity-70 transition-opacity"
        >
          {expanded ? (
            <ChevronDown className="h-3 w-3" />
          ) : (
            <ChevronRight className="h-3 w-3" />
          )}
          {label && <span className="text-purple-400">{label}</span>}
          <span className="text-purple-500">
            {"{"}
            {keys.length}
            {"}"}
          </span>
        </button>
        {expanded && (
          <div className={indent}>
            {keys.map((key) => (
              <div key={key} className="py-0.5">
                <JsonTree
                  data={data[key]}
                  depth={depth + 1}
                  maxDepth={maxDepth}
                  label={key}
                />
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <span className={cn("font-mono text-xs", getTypeColor(data), className)}>
      {label && <span className="text-purple-400 mr-2">{label}:</span>}
      {getPreview(data)}
    </span>
  );
}

export function tryParseJson(content: string): unknown | null {
  const trimmed = content.trim();
  if (!trimmed) return null;
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      return JSON.parse(trimmed);
    } catch {
      return null;
    }
  }
  return null;
}

export default JsonTree;
