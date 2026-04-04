import { JsonTree } from "@/components/json/JsonTree";
import { Badge } from "@/components/ui/badge";

interface SchemaPanelProps {
  schema: Record<string, unknown>;
  className?: string;
}

export function SchemaPanel({ schema, className }: SchemaPanelProps) {
  const properties = (schema.properties as Record<string, Record<string, unknown>>) || {};
  const required = (schema.required as string[]) || [];

  return (
    <div className={className}>
      {schema.description && (
        <p className="text-xs text-muted-foreground mb-2">{schema.description as string}</p>
      )}
      {Object.keys(properties).length > 0 ? (
        <div className="space-y-1">
          {Object.entries(properties).map(([key, prop]) => (
            <div key={key} className="rounded border p-2 space-y-0.5">
              <div className="flex items-center gap-2">
                <span className="font-mono text-xs font-medium">{key}</span>
                {required.includes(key) && (
                  <Badge variant="outline" className="text-[9px] h-4">required</Badge>
                )}
                {prop.type && (
                  <span className="text-[10px] text-info font-mono">{String(prop.type)}</span>
                )}
              </div>
              {prop.description && (
                <p className="text-[11px] text-muted-foreground">{String(prop.description)}</p>
              )}
              {prop.enum && (
                <div className="flex gap-1 flex-wrap">
                  {(prop.enum as string[]).map((v) => (
                    <Badge key={v} variant="secondary" className="text-[9px]">{String(v)}</Badge>
                  ))}
                </div>
              )}
              {prop.default !== undefined && (
                <span className="text-[10px] text-muted-foreground">default: <span className="font-mono">{JSON.stringify(prop.default)}</span></span>
              )}
            </div>
          ))}
        </div>
      ) : (
        <JsonTree data={schema} defaultExpanded={false} />
      )}
    </div>
  );
}
