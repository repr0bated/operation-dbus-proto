// JSON Schema type — used by SchemaRenderer for generative UI blocks
export interface JsonSchema {
  type?: string;
  title?: string;
  description?: string;
  properties?: Record<string, JsonSchema>;
  items?: JsonSchema;
  required?: string[];
  enum?: unknown[];
  default?: unknown;
  oneOf?: JsonSchema[];
  anyOf?: JsonSchema[];
  additionalProperties?: boolean | JsonSchema;
  format?: string;
  minimum?: number;
  maximum?: number;
  [key: string]: unknown;
}
