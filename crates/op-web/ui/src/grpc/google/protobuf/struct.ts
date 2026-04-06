/**
 * google.protobuf.Value / Struct helpers.
 * @protobuf-ts uses JsonObject for Struct and JsonValue for Value natively.
 */
export type ProtobufStruct = Record<string, unknown>;
export type ProtobufValue = unknown;

/** Convert a protobuf Struct (JSON-encoded) to a plain JS object */
export function structToObject(s: ProtobufStruct | undefined): Record<string, unknown> {
  if (!s) return {};
  return s as Record<string, unknown>;
}

/** Convert a protobuf Value to a plain JS value */
export function valueToJs(v: ProtobufValue): unknown {
  return v;
}

/** Convert a JS object to a protobuf Struct */
export function objectToStruct(obj: Record<string, unknown>): ProtobufStruct {
  return obj;
}
