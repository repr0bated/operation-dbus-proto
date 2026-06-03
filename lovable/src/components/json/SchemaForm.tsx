import { useState, useCallback } from "react";
import { cn } from "@/lib/utils";

interface SchemaFormProps {
  schema: {
    type?: string;
    properties?: Record<string, SchemaProperty>;
    required?: string[];
    title?: string;
  } | null;
  value: Record<string, unknown>;
  onChange: (value: Record<string, unknown>) => void;
  onSubmit: () => void;
  className?: string;
  disabled?: boolean;
}

interface SchemaProperty {
  type?: string;
  enum?: string[];
  oneOf?: Array<{ const?: string; title?: string }>;
  description?: string;
  minLength?: number;
  maxLength?: number;
  default?: unknown;
}

export function SchemaForm({
  schema,
  value,
  onChange,
  onSubmit,
  className,
  disabled = false,
}: SchemaFormProps) {
  const [errors, setErrors] = useState<Record<string, string>>({});

  const validateField = useCallback((key: string, prop: SchemaProperty, val: unknown): string | null => {
    if (prop.minLength && typeof val === "string" && val.length < prop.minLength) {
      return `Minimum length is ${prop.minLength}`;
    }
    if (schema?.required?.includes(key) && (!val || (typeof val === "string" && val.trim() === ""))) {
      return "This field is required";
    }
    return null;
  }, [schema?.required]);

  const handleFieldChange = (key: string, prop: SchemaProperty, newValue: unknown) => {
    const error = validateField(key, prop, newValue);
    setErrors((prev) => ({ ...prev, [key]: error || "" }));
    onChange({ ...value, [key]: newValue });
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      onSubmit();
    }
  };

  if (!schema || schema.type !== "object" || !schema.properties) {
    return (
      <div className={cn("text-sm text-muted-foreground", className)}>
        No schema available
      </div>
    );
  }

  const properties = schema.properties;

  return (
    <div className={cn("space-y-4", className)}>
      {Object.entries(properties).map(([key, prop]) => (
        <SchemaField
          key={key}
          name={key}
          property={prop}
          value={value[key]}
          error={errors[key]}
          required={schema.required?.includes(key)}
          disabled={disabled}
          onChange={(newValue) => handleFieldChange(key, prop, newValue)}
          onKeyDown={handleKeyDown}
        />
      ))}
    </div>
  );
}

interface SchemaFieldProps {
  name: string;
  property: SchemaProperty;
  value: unknown;
  error?: string;
  required?: boolean;
  disabled?: boolean;
  onChange: (value: unknown) => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
}

function SchemaField({
  name,
  property,
  value,
  error,
  required,
  disabled,
  onChange,
  onKeyDown,
}: SchemaFieldProps) {
  const label = property.description || name;

  // Enum select (provider dropdown)
  if (property.enum && property.enum.length > 0) {
    return (
      <div className="space-y-1">
        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          {label}
          {required && <span className="text-danger ml-1">*</span>}
        </label>
        <select
          value={(value as string) || ""}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
          className={cn(
            "w-full px-3 py-2 rounded-md border bg-card text-sm font-mono focus:border-ring outline-none transition-colors",
            error ? "border-danger" : "border-input",
            disabled && "opacity-50 cursor-not-allowed"
          )}
        >
          {!required && <option value="">-- Select {label} --</option>}
          {property.enum.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
        {error && <span className="text-xs text-danger">{error}</span>}
      </div>
    );
  }

  // oneOf select with const/title (model dropdown)
  if (property.oneOf && property.oneOf.length > 0) {
    return (
      <div className="space-y-1">
        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          {label}
          {required && <span className="text-danger ml-1">*</span>}
        </label>
        <select
          value={(value as string) || ""}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
          className={cn(
            "w-full px-3 py-2 rounded-md border bg-card text-sm font-mono focus:border-ring outline-none transition-colors",
            error ? "border-danger" : "border-input",
            disabled && "opacity-50 cursor-not-allowed"
          )}
        >
          {!required && <option value="">-- Select {label} --</option>}
          {property.oneOf.map((opt) => (
            <option key={`${opt.const ?? ""}:${opt.title ?? ""}`} value={opt.const}>
              {opt.title || opt.const}
            </option>
          ))}
        </select>
        {error && <span className="text-xs text-danger">{error}</span>}
      </div>
    );
  }

  // Textarea for string fields (message input)
  if (property.type === "string") {
    return (
      <div className="space-y-1">
        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          {label}
          {required && <span className="text-danger ml-1">*</span>}
        </label>
        <textarea
          value={(value as string) || ""}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          disabled={disabled}
          placeholder={`Enter ${label.toLowerCase()}...`}
          rows={4}
          className={cn(
            "w-full px-3 py-2.5 rounded-lg border bg-card text-sm resize-none min-h-[80px] max-h-40 focus:border-ring focus:ring-1 focus:ring-ring outline-none transition-colors font-sans",
            error ? "border-danger" : "border-input",
            disabled && "opacity-50 cursor-not-allowed"
          )}
        />
        {error && <span className="text-xs text-danger">{error}</span>}
      </div>
    );
  }

  // Fallback for unsupported types
  return (
    <div className="space-y-1">
      <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
        {label}
      </label>
      <div className="text-xs text-muted-foreground">
        Unsupported field type: {property.type}
      </div>
    </div>
  );
}
