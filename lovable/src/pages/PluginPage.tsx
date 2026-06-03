import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { SchemaRenderer } from "@/components/json/SchemaRenderer";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ArrowLeft, Puzzle } from "lucide-react";

interface FieldDef {
  name: string;
  type: string | Record<string, unknown>;
  required?: boolean;
  description?: string;
  example?: unknown;
  readOnly?: boolean;
}

interface PluginEntry {
  name: string;
  version: string;
  category?: string;
  fields: FieldDef[];
}

function pluginEntryToSchema(entry: PluginEntry, pluginId: string) {
  const properties: Record<string, unknown> = {};
  const required: string[] = [];
  for (const field of entry.fields) {
    const prop: Record<string, unknown> = {};
    if (typeof field.type === "string") {
      prop.type = field.type;
    } else {
      prop.type = "object";
      prop.properties = field.type;
    }
    if (field.description) prop.description = field.description;
    if (field.example !== undefined && field.example !== null) prop.default = field.example;
    if (field.readOnly) prop.readOnly = true;
    properties[field.name] = prop;
    if (field.required) required.push(field.name);
  }
  return {
    $schema: "http://json-schema.org/draft-07/schema#",
    title: entry.name.replace(/_/g, " ").replace(/\b\w/g, (c: string) => c.toUpperCase()),
    description: `${entry.fields.length} fields — ${entry.category || "uncategorized"}`,
    type: "object",
    properties,
    required: required.length > 0 ? required : undefined,
  };
}

export default function PluginPage() {
  const { pluginId } = useParams<{ pluginId: string }>();
  const navigate = useNavigate();
  const [entry, setEntry] = useState<PluginEntry | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!pluginId) return;
    fetch("/api/schema")
      .then((r) => r.json())
      .then((catalog) => {
        const entries = catalog[pluginId];
        if (entries && entries.length > 0) {
          setEntry(entries[0]);
        } else {
          setError(`Plugin "${pluginId}" not found in schema catalog`);
        }
        setLoading(false);
      })
      .catch(() => {
        setError("Failed to load schema catalog");
        setLoading(false);
      });
  }, [pluginId]);

  if (loading) {
    return (
      <div className="container mx-auto p-6 space-y-4">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-96" />
      </div>
    );
  }

  if (error || !entry) {
    return (
      <div className="container mx-auto p-6 text-center py-12">
        <Puzzle className="w-12 h-12 mx-auto mb-4 text-muted-foreground" />
        <p className="text-muted-foreground">{error || "Plugin not found"}</p>
        <Button variant="outline" className="mt-4" onClick={() => navigate("/schema-gallery")}>
          <ArrowLeft className="w-4 h-4 mr-2" /> Back to Gallery
        </Button>
      </div>
    );
  }

  const schema = pluginEntryToSchema(entry, pluginId || "");

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate("/schema-gallery")}>
          <ArrowLeft className="w-4 h-4" />
        </Button>
        <div>
          <h1 className="text-2xl font-bold">
            {entry.name.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase())}
          </h1>
          <div className="flex gap-2 mt-1">
            <Badge variant="outline">{pluginId}</Badge>
            <Badge variant="secondary">{entry.version}</Badge>
            {entry.category && <Badge>{entry.category}</Badge>}
          </div>
        </div>
      </div>
      <Card>
        <CardContent className="pt-6">
          <SchemaRenderer schema={schema as any} />
        </CardContent>
      </Card>
    </div>
  );
}
