/**
 * SchemaRenderer - JSON Schema (draft-07) → Dynamic Form
 *
 * Renders a JSON Schema as a dynamic form with full recursive nesting.
 * Supports compact card preview and full editing modes with type-aware
 * form controls powered by shadcn/ui primitives.
 */
import { useState, useMemo } from 'react';
import { cn } from '@/lib/utils';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';
import { HelpCircle, Plus, Trash2, GripVertical, ChevronDown, ChevronRight } from 'lucide-react';

// ── Types ───────────────────────────────────────────────

interface JSONSchema {
  $ref?: string; $schema?: string; type?: string | string[]; title?: string;
  description?: string; default?: unknown; properties?: Record<string, JSONSchema>;
  required?: string[]; items?: JSONSchema | JSONSchema[]; enum?: unknown[];
  oneOf?: JSONSchema[]; anyOf?: JSONSchema[]; readOnly?: boolean; writeOnly?: boolean;
  minimum?: number; maximum?: number; minLength?: number; maxLength?: number;
  pattern?: string; format?: string; [key: string]: unknown;
}

export interface SchemaRendererProps {
  schema: JSONSchema;
  value?: Record<string, unknown>;
  data?: Record<string, unknown>; // alias for value — backwards compat
  onChange?: (value: Record<string, unknown>) => void;
  readOnly?: boolean;
  compact?: boolean;
  className?: string;
  label?: string;
  onDeploy?: () => void;
}

// ── $ref Resolution ──────────────────────────────────

function resolveRef(ref: string, root: JSONSchema): JSONSchema {
  const parts = ref.replace(/^#\//, '').split('/');
  let cur: unknown = root;
  for (const p of parts) {
    if (!cur || typeof cur !== 'object') return {};
    cur = (cur as Record<string, unknown>)[p];
  }
  return (cur as JSONSchema) ?? {};
}

function resolveSchema(schema: JSONSchema, root: JSONSchema): JSONSchema {
  if (!schema.$ref) return schema;
  const resolved = resolveRef(schema.$ref, root);
  const { $ref, ...rest } = schema;
  return { ...resolved, ...rest };
}

// ── Helpers ─────────────────────────────────────────────

function getType(s: JSONSchema): string {
  if (s.oneOf || s.anyOf) return 'oneOf';
  if (typeof s.type === 'string') return s.type;
  if (Array.isArray(s.type) && s.type.length) return s.type[0];
  if (s.properties) return 'object';
  if (s.items) return 'array';
  if (s.enum) return 'enum';
  return 'string';
}

function fieldCount(s: JSONSchema): number {
  return s.properties ? Object.keys(s.properties).length : 0;
}

function coerceObj(v: unknown): Record<string, unknown> {
  if (v && typeof v === 'object' && !Array.isArray(v)) return v as Record<string, unknown>;
  return {};
}

function coerceArr(v: unknown): unknown[] {
  return Array.isArray(v) ? (v as unknown[]) : [];
}

function getItemSchema(s: JSONSchema): JSONSchema {
  if (!s.items || Array.isArray(s.items)) return { type: 'string' };
  return s.items as JSONSchema;
}

// ── Mini Components ─────────────────────────────────────

const ReqStar = ({ on }: { on: boolean }) => on ? <span className="text-red-500">*</span> : null;

const HelpTip = ({ text }: { text?: string }) => text ? (
  <Tooltip>
    <TooltipTrigger asChild><HelpCircle className="h-3 w-3 text-muted-foreground/50 cursor-help" /></TooltipTrigger>
    <TooltipContent side="right" className="text-[11px] max-w-[240px]">{text}</TooltipContent>
  </Tooltip>
) : null;

function FieldWrapper({ name, desc, required, children }: {
  name?: string; desc?: string; required?: boolean; children: React.ReactNode;
}) {
  return (
    <div className="space-y-1">
      {name && (
        <Label className="text-[11px] font-mono font-medium text-muted-foreground uppercase tracking-wider flex items-center gap-1">
          {name}<ReqStar on={!!required} /><HelpTip text={desc} />
        </Label>
      )}
      {children}
    </div>
  );
}

// ── Compact Card ────────────────────────────────────────

function CompactCard({ schema, onDeploy }: { schema: JSONSchema; onDeploy?: () => void }) {
  const [hovered, setHovered] = useState(false);
  const count = fieldCount(schema);
  const preview = schema.properties ? Object.keys(schema.properties).slice(0, 3) : [];

  return (
    <Card className="relative overflow-hidden transition-shadow duration-200 hover:shadow-md"
      onMouseEnter={() => setHovered(true)} onMouseLeave={() => setHovered(false)}>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between gap-2">
          <CardTitle className="text-sm font-medium truncate">{schema.title ?? 'Untitled Schema'}</CardTitle>
          <Badge variant="secondary" className="text-[10px] shrink-0">{count} {count === 1 ? 'field' : 'fields'}</Badge>
        </div>
        {schema.description && <CardDescription className="text-[11px] line-clamp-2">{schema.description}</CardDescription>}
      </CardHeader>
      <CardContent className="pb-3 pt-0">
        {preview.map((name) => (
          <div key={name} className="flex items-center gap-2 text-[11px]">
            <span className="text-muted-foreground font-mono">{name}</span>
            <span className="text-muted-foreground/50 text-[10px]">{schema.properties![name].type ?? 'string'}</span>
          </div>
        ))}
        {count > 3 && <span className="text-[10px] text-muted-foreground/50 italic">+{count - 3} more field{count - 3 !== 1 ? 's' : ''}…</span>}
        {count === 0 && <span className="text-[11px] text-muted-foreground italic">No fields defined</span>}
      </CardContent>
      {onDeploy && hovered && (
        <div className="absolute inset-0 bg-background/85 flex items-center justify-center backdrop-blur-[1px]">
          <Button size="sm" onClick={onDeploy} className="text-xs">Deploy</Button>
        </div>
      )}
    </Card>
  );
}

// ── PropertyRenderer (core recursive) ──────────────────

interface PRProps {
  name: string; schema: JSONSchema; value: unknown; onChange: (v: unknown) => void;
  readOnly: boolean; required: boolean; rootSchema: JSONSchema; depth: number;
}

function PropertyRenderer({ name, schema, value, onChange, readOnly, required, rootSchema, depth }: PRProps) {
  const resolved = useMemo(() => resolveSchema(schema, rootSchema), [schema, rootSchema]);
  const type = useMemo(() => getType(resolved), [resolved]);
  const ro = readOnly || resolved.readOnly === true;

  // ── oneOf / anyOf → Tabs ──
  if (type === 'oneOf' && (resolved.oneOf || resolved.anyOf)) {
    const options = resolved.oneOf ?? resolved.anyOf ?? [];
    const label = resolved.oneOf ? 'oneOf' : 'anyOf';
    return (
      <div className="space-y-2">
        <Label className="text-[11px] font-mono font-medium text-muted-foreground uppercase tracking-wider flex items-center gap-1">
          {name}<ReqStar on={required} />
          <Badge variant="outline" className="text-[9px] px-1 py-0 h-4 ml-1">{label}</Badge>
          <HelpTip text={resolved.description} />
        </Label>
        <Tabs defaultValue="0" className="w-full">
          <TabsList className="h-7">
            {options.map((opt, i) => (
              <TabsTrigger key={i} value={String(i)} className="text-[10px] px-2 h-5">
                {opt.title ?? `Option ${i + 1}`}
              </TabsTrigger>
            ))}
          </TabsList>
          {options.map((opt, i) => (
            <TabsContent key={i} value={String(i)} className="pt-2 mt-0">
              <PropertyRenderer name="" schema={opt} value={value} onChange={onChange}
                readOnly={ro} required={false} rootSchema={rootSchema} depth={depth + 1} />
            </TabsContent>
          ))}
        </Tabs>
      </div>
    );
  }

  // ── object → collapsible section ──
  if (type === 'object' && resolved.properties) {
    const [collapsed, setCollapsed] = useState(depth > 1);
    const obj = coerceObj(value);
    const reqF = resolved.required ?? [];
    const keys = Object.keys(resolved.properties);
    return (
      <div className="space-y-1">
        {name && (
          <button type="button" onClick={() => setCollapsed(!collapsed)}
            className="flex items-center gap-1 hover:text-foreground text-muted-foreground transition-colors cursor-pointer">
            {collapsed ? <ChevronRight className="h-3 w-3 shrink-0" /> : <ChevronDown className="h-3 w-3 shrink-0" />}
            <span className="text-[11px] font-mono font-medium uppercase tracking-wider">{name}</span>
            <ReqStar on={required} />
            {resolved.title && <span className="text-[10px] text-muted-foreground truncate max-w-[120px]">{resolved.title}</span>}
            <Badge variant="secondary" className="text-[9px] px-1 py-0 h-4">{keys.length}</Badge>
            <HelpTip text={resolved.description} />
          </button>
        )}
        {!collapsed && (
          <div className={cn('ml-3 pl-3 border-l border-border/50 space-y-3 pt-1', !name && 'ml-0 pl-0 border-l-0')}>
            {keys.map((k) => (
              <PropertyRenderer key={k} name={k} schema={resolved.properties![k]} value={obj[k]}
                onChange={(v) => onChange({ ...obj, [k]: v })}
                readOnly={ro} required={reqF.includes(k)} rootSchema={rootSchema} depth={depth + 1} />
            ))}
            {keys.length === 0 && <span className="text-[10px] text-muted-foreground italic">No properties</span>}
          </div>
        )}
      </div>
    );
  }

  // ── array → repeatable items ──
  if (type === 'array') {
    const arr = coerceArr(value);
    const itemSchema = getItemSchema(resolved);
    return (
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Label className="text-[11px] font-mono font-medium text-muted-foreground uppercase tracking-wider flex items-center gap-1">
            {name}<ReqStar on={required} />
          </Label>
          <Badge variant="secondary" className="text-[9px] px-1 py-0 h-4">{arr.length}</Badge>
          <HelpTip text={resolved.description} />
          {!ro && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon" className="h-5 w-5" onClick={() => onChange([...arr, undefined])}>
                  <Plus className="h-3 w-3" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Add item</TooltipContent>
            </Tooltip>
          )}
        </div>
        <div className="space-y-2 ml-3 pl-3 border-l border-border/50">
          {arr.length === 0 ? (
            <span className="text-[10px] text-muted-foreground italic">No items</span>
          ) : (
            arr.map((item, i) => (
              <div key={i} className="flex items-start gap-1 group">
                <GripVertical className="h-3 w-3 text-muted-foreground/30 mt-1.5 shrink-0 cursor-grab" />
                <div className="flex-1 min-w-0">
                  <PropertyRenderer name={String(i)} schema={itemSchema} value={item}
                    onChange={(v) => { const next = [...arr]; next[i] = v; onChange(next); }}
                    readOnly={ro} required={false} rootSchema={rootSchema} depth={depth + 1} />
                </div>
                {!ro && (
                  <Button variant="ghost" size="icon"
                    className="h-5 w-5 text-muted-foreground/50 hover:text-red-500 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                    onClick={() => onChange(arr.filter((_, idx) => idx !== i))}>
                    <Trash2 className="h-3 w-3" />
                  </Button>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    );
  }

  // ── boolean → Switch ──
  if (type === 'boolean') {
    return (
      <FieldWrapper name={name} desc={resolved.description} required={required}>
        <div className="flex items-center gap-2">
          <Switch checked={Boolean(value)} disabled={ro} onCheckedChange={onChange} />
          <Badge variant={value ? 'default' : 'secondary'} className="text-[10px] px-1.5 font-mono">
            {value ? 'true' : 'false'}
          </Badge>
        </div>
      </FieldWrapper>
    );
  }

  // ── enum → Select ──
  if (type === 'enum' || resolved.enum) {
    const options = (resolved.enum as unknown[]) ?? [];
    return (
      <FieldWrapper name={name} desc={resolved.description} required={required}>
        <Select value={value !== undefined && value !== null && value !== '' ? String(value) : '__none__'}
          disabled={ro} onValueChange={onChange}>
          <SelectTrigger className="h-8 text-xs font-mono"><SelectValue placeholder="Select…" /></SelectTrigger>
          <SelectContent>
            {!required && <SelectItem value="__none__" className="text-xs font-mono text-muted-foreground" disabled>Select…</SelectItem>}
            {options.map((v) => (
              <SelectItem key={String(v)} value={String(v)} className="text-xs font-mono">{String(v)}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </FieldWrapper>
    );
  }

  // ── number / integer → number input ──
  if (type === 'number' || type === 'integer') {
    return (
      <FieldWrapper name={name} desc={resolved.description} required={required}>
        <Input type="number" value={value !== undefined && value !== null ? Number(value) : ''} readOnly={ro}
          min={resolved.minimum} max={resolved.maximum} step={type === 'integer' ? 1 : undefined}
          placeholder={resolved.default !== undefined ? String(resolved.default) : ''}
          onChange={(e) => {
            const raw = e.target.value;
            if (raw === '') { onChange(undefined); return; }
            const v = type === 'integer' ? parseInt(raw, 10) : parseFloat(raw);
            onChange(isNaN(v) ? (type === 'integer' ? 0 : 0) : v);
          }}
          className="h-8 text-xs font-mono" />
      </FieldWrapper>
    );
  }

  // ── string → Input or Textarea ──
  if (type === 'string') {
    const isLong = (resolved.maxLength && resolved.maxLength > 200) || resolved.format === 'textarea';
    const ph = resolved.default !== undefined ? String(resolved.default) : '';
    if (isLong) {
      return (
        <FieldWrapper name={name} desc={resolved.description} required={required}>
          <Textarea value={String(value ?? '')} readOnly={ro} placeholder={ph}
            maxLength={resolved.maxLength} minLength={resolved.minLength}
            onChange={(e) => onChange(e.target.value)} className="h-20 text-xs font-mono resize-y" />
        </FieldWrapper>
      );
    }
    return (
      <FieldWrapper name={name} desc={resolved.description} required={required}>
        <Input type={resolved.format === 'password' ? 'password' : 'text'} value={String(value ?? '')} readOnly={ro}
          placeholder={ph} maxLength={resolved.maxLength} minLength={resolved.minLength}
          onChange={(e) => onChange(e.target.value)} className="h-8 text-xs font-mono" />
      </FieldWrapper>
    );
  }

  // ── fallback: raw JSON ──
  return (
    <FieldWrapper name={name} desc={resolved.description} required={required}>
      <pre className="text-[10px] text-muted-foreground bg-muted/30 rounded-md px-2 py-1 overflow-auto max-h-24 whitespace-pre-wrap break-all font-mono">
        {value !== undefined ? JSON.stringify(value, null, 2) : 'null'}
      </pre>
    </FieldWrapper>
  );
}

// ── SchemaRenderer (Main Export) ────────────────────────

export function SchemaRenderer({ schema, value, data, onChange, readOnly = false, compact = false, onDeploy }: SchemaRendererProps) {
  value = value ?? data;
  if (compact) return <CompactCard schema={schema} onDeploy={onDeploy} />;

  const obj = coerceObj(value);
  const props = schema.properties ?? {};
  const reqF = schema.required ?? [];
  const keys = Object.keys(props);

  return (
    <div className="space-y-4">
      {(schema.title || schema.description) && (
        <div>
          {schema.title && <h3 className="text-sm font-semibold text-foreground">{schema.title}</h3>}
          {schema.description && <p className="text-[11px] text-muted-foreground mt-0.5">{schema.description}</p>}
        </div>
      )}
      <ScrollArea className="max-h-[70vh]">
        <div className="space-y-3 pr-3">
          {keys.map((k) => (
            <PropertyRenderer key={k} name={k} schema={props[k]} value={obj[k]}
              onChange={(v) => onChange?.({ ...obj, [k]: v })} readOnly={readOnly}
              required={reqF.includes(k)} rootSchema={schema} depth={0} />
          ))}
          {keys.length === 0 && (
            <p className="text-[11px] text-muted-foreground italic py-4 text-center">No properties defined</p>
          )}
        </div>
      </ScrollArea>
      {schema.$schema && (
        <>
          <Separator />
          <p className="text-[9px] text-muted-foreground/50 font-mono">{schema.$schema}</p>
        </>
      )}
    </div>
  );
}

export default SchemaRenderer;
