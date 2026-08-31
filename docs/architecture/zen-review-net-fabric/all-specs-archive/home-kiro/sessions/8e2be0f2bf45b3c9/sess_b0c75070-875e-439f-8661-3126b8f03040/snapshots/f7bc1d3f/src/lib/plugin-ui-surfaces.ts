import type { Spec } from "@json-render/core";
import { schemaToSpec } from "@/components/PluginPage";
import { resolveApiBase } from "@/lib/api";

export interface PluginUiSurface {
  name: string;
  path: string;
  schema: string;
}

export interface PluginSchemaEnvelope {
  plugin: string;
  schema_hash?: string;
  schema: unknown;
}

export interface SealedPluginGalleryEntry {
  id: string;
  plugin: string;
  spec: Spec;
  schema_hash?: string;
}

export interface AntigravityGallerySurface {
  id: string;
  name: string;
  path: string;
  schema: string;
  spec: Spec;
}

export interface AntigravityGalleryBundle {
  schema_hash?: string;
  surfaces: AntigravityGallerySurface[];
}

interface PluginSchemaEnvelopeShape {
  schema?: {
    fields?: {
      ui_surfaces?: {
        default?: unknown;
      };
    };
  };
}

interface SchemaField {
  field_type?: string | Record<string, unknown>;
  description?: string | null;
  required?: boolean;
  read_only?: boolean;
  default?: unknown;
}

interface SchemaMethod {
  name?: string;
  side_effect?: string;
  subid?: string;
  required_capability?: string | null;
  idempotent?: boolean;
}

interface PluginSchemaShape {
  name?: string;
  display_name?: string | null;
  version?: string;
  category?: string;
  description?: string;
  tags?: string[];
  dependencies?: string[];
  fields?: Record<string, SchemaField> | SchemaField[];
  methods?: Record<string, SchemaMethod> | SchemaMethod[];
  guarantees?: Record<string, boolean>;
}

/** Read ui_surfaces.default from a sealed plugin schema envelope. */
export function parsePluginUiSurfaces(payload: unknown): PluginUiSurface[] {
  const raw = (payload as PluginSchemaEnvelopeShape | null)?.schema?.fields?.ui_surfaces?.default;
  if (!Array.isArray(raw)) return [];

  return raw.flatMap((value) => {
    if (!value || typeof value !== "object") return [];
    const route = value as Record<string, unknown>;
    if (
      typeof route.name !== "string" ||
      typeof route.path !== "string" ||
      typeof route.schema !== "string"
    ) {
      return [];
    }
    return [{ name: route.name, path: route.path, schema: route.schema }];
  });
}

export async function fetchPluginSchema(plugin: string): Promise<PluginSchemaEnvelope> {
  const response = await fetch(
    `${resolveApiBase()}/ui-model/plugin-schema/${encodeURIComponent(plugin)}`,
  );
  if (!response.ok) throw new Error(`${plugin} schema HTTP ${response.status}`);
  return response.json() as Promise<PluginSchemaEnvelope>;
}

export async function fetchPluginUiSurfaces(plugin: string): Promise<PluginUiSurface[]> {
  const envelope = await fetchPluginSchema(plugin);
  return parsePluginUiSurfaces(envelope);
}

/** Render the sealed PluginSchema blob through the shared schema → spec builder. */
export function sealedPluginSpec(plugin: string, envelope: PluginSchemaEnvelope): Spec {
  return schemaToSpec(envelope.schema as PluginSchemaShape, plugin);
}

/** Plugins that exist as sealed blobs but declare no json-render UI surfaces. */
export const PLUGINS_WITHOUT_UI_SURFACES = new Set(["antigravity_chat"]);

/** Map gallery catalog ids like `antigravity-blob-render` → sealed plugin id. */
export function pluginIdFromBlobRenderCatalogId(catalogId: string): string | null {
  if (catalogId === "antigravity-blob-render") return "antigravity";
  if (catalogId === "antigravity_chat-blob-render" || catalogId === "antigravity-chat-blob-render") {
    return "antigravity_chat";
  }
  const match = catalogId.match(/^(.+)-blob-render$/);
  if (!match) return null;
  return match[1].replace(/-/g, "_");
}

export function isRenderableGalleryPlugin(pluginId: string): boolean {
  return !PLUGINS_WITHOUT_UI_SURFACES.has(pluginId);
}

function entriesOf<T>(v: Record<string, T> | T[] | undefined): Array<[string, T]> {
  if (!v) return [];
  if (Array.isArray(v)) {
    return v.map((item, i) => [((item as { name?: string }).name ?? String(i)), item]);
  }
  return Object.entries(v);
}

const ANTIGRAVITY_SURFACE_FIELDS: Record<string, string[] | null> = {
  antigravity: null,
  "antigravity.chat": [], // ChatWidget mount — no schema field dump
  "antigravity.safety": ["safety_settings"],
  "antigravity.usage": ["usage"],
  "antigravity.auth": ["auth"],
};

const ANTIGRAVITY_SURFACE_METHODS: Record<string, string[]> = {
  antigravity: ["get_auth_status", "get_usage_report", "configure_safety"],
  "antigravity.chat": [],
  "antigravity.safety": ["configure_safety"],
  "antigravity.usage": ["get_usage_report"],
  "antigravity.auth": ["get_auth_status"],
};

const ANTIGRAVITY_CHAT_SURFACE: PluginUiSurface = {
  name: "Antigravity Chat",
  path: "/antigravity/chat",
  schema: "antigravity.chat",
};

/** True when this antigravity ui_surface should mount ChatWidget (not SpecRenderer). */
export function isAntigravityChatSurface(surface: {
  path?: string;
  schema?: string;
  id?: string;
}): boolean {
  return (
    surface.schema === "antigravity.chat" ||
    surface.path === "/antigravity/chat" ||
    surface.id === "antigravity/antigravity/chat"
  );
}

/** Render one Antigravity ui_surface from the sealed blob, not route metadata. */
export function antigravityUiSurfaceSpec(
  envelope: PluginSchemaEnvelope,
  surface: PluginUiSurface,
): Spec {
  const full = envelope.schema as PluginSchemaShape;
  const fieldKeys = ANTIGRAVITY_SURFACE_FIELDS[surface.schema];
  const methodKeys = ANTIGRAVITY_SURFACE_METHODS[surface.schema] ?? [];

  if (fieldKeys === null) {
    return schemaToSpec(full, surface.schema);
  }

  // ChatWidget mount — empty field slice, no schema dump.
  if (Array.isArray(fieldKeys) && fieldKeys.length === 0) {
    return schemaToSpec(
      {
        name: surface.schema,
        display_name: surface.name,
        description: `Antigravity UI surface ${surface.path} · ChatWidget`,
        fields: {},
        methods: {},
      },
      surface.schema,
    );
  }

  // Unknown schema key — fall back to full schema with surface display name.
  if (fieldKeys === undefined) {
    return schemaToSpec(
      {
        ...full,
        name: surface.schema,
        display_name: surface.name,
      },
      surface.schema,
    );
  }

  const allFields = entriesOf<SchemaField>(full.fields);
  const fields = Object.fromEntries(
    allFields.filter(([name]) => fieldKeys.includes(name)),
  );

  const allMethods = entriesOf<SchemaMethod>(full.methods);
  const methods = Object.fromEntries(
    allMethods.filter(([name]) => methodKeys.includes(name)),
  );

  return schemaToSpec(
    {
      name: surface.schema,
      display_name: surface.name,
      description: `Antigravity UI surface ${surface.path} · SpecRenderer`,
      fields,
      methods,
    },
    surface.schema,
  );
}

/** Antigravity gallery entries from the plugin's own ui_surfaces projection. */
export async function fetchAntigravityGalleryBundle(): Promise<AntigravityGalleryBundle> {
  const envelope = await fetchPluginSchema("antigravity");
  let routes = parsePluginUiSurfaces(envelope);
  if (routes.length === 0) {
    throw new Error("antigravity schema declares no ui_surfaces");
  }

  // Chat is an antigravity ui_surface (not zeroclaw, not antigravity_chat).
  // Inject until the sealed blob is resealed with antigravity.chat.
  if (!routes.some((r) => isAntigravityChatSurface(r))) {
    routes = [...routes, ANTIGRAVITY_CHAT_SURFACE];
  }

  return {
    schema_hash: envelope.schema_hash,
    surfaces: routes.map((surface) => ({
      id: `antigravity${surface.path}`,
      name: surface.name,
      path: surface.path,
      schema: surface.schema,
      spec: antigravityUiSurfaceSpec(envelope, surface),
    })),
  };
}

export interface ChatbotModelGallerySurface {
  id: string;
  name: string;
  path: string;
  schema: string;
  spec: Spec;
}

export interface ChatbotModelGalleryBundle {
  schema_hash?: string;
  provider?: string;
  model?: string;
  surfaces: ChatbotModelGallerySurface[];
}

/** zeroclaw owns chatbot model routing; surface field slices for Gallery. */
const ZEROCLAW_SURFACE_FIELDS: Record<string, string[] | null> = {
  zeroclaw: null,
  "zeroclaw.providers": [
    "selected_provider",
    "selected_model",
    "providers",
    "model_routes",
    "model_assignments",
    "router",
    "status",
    "transport",
  ],
};

const ZEROCLAW_SURFACE_METHODS: Record<string, string[]> = {
  zeroclaw: [
    "GetModelRoutes",
    "GetProviderCatalog",
    "ListModels",
    "ListProviders",
    "SetModel",
    "SetProvider",
    "ResolveRoute",
    "GetRouter",
    "GetState",
  ],
  "zeroclaw.providers": [
    "GetModelRoutes",
    "GetProviderCatalog",
    "ListModels",
    "ListProviders",
    "SetModel",
    "SetProvider",
    "ResolveRoute",
    "GetRouter",
    "GetModelAssignments",
  ],
};

function readSchemaDefaultString(
  envelope: PluginSchemaEnvelope,
  field: string,
): string | undefined {
  const fields = (envelope.schema as PluginSchemaShape | undefined)?.fields;
  if (!fields || Array.isArray(fields)) return undefined;
  const value = fields[field]?.default;
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

export interface ZeroclawChatSelection {
  provider: string;
  model: string;
}

/**
 * Active chatbot inference route from zeroclaw (OD-28).
 * Prefer live `/llm/status`; fall back to sealed schema defaults.
 * Empty strings mean "use zeroclaw selected_*" on the bridge.
 */
export async function resolveZeroclawChatSelection(): Promise<ZeroclawChatSelection> {
  let provider = "";
  let model = "";
  try {
    const statusResp = await fetch(`${resolveApiBase()}/llm/status`);
    if (statusResp.ok) {
      const status = (await statusResp.json()) as { provider?: string; model?: string };
      if (
        typeof status.provider === "string" &&
        status.provider.length > 0 &&
        status.provider !== "zeroclaw-unavailable"
      ) {
        provider = status.provider;
      }
      if (typeof status.model === "string" && status.model.length > 0) {
        model = status.model;
      }
    }
  } catch {
    // Fall through to schema defaults.
  }
  if (provider && model) {
    return { provider, model };
  }
  try {
    const envelope = await fetchPluginSchema("tched_router");
    provider = provider || readSchemaDefaultString(envelope, "selected_provider") || "";
    model = model || readSchemaDefaultString(envelope, "selected_model") || "";
  } catch {
    // Bridge accepts empty provider/model → uses live selected_*.
  }
  return { provider, model };
}

/** One zeroclaw ui_surface from the sealed blob (main chatbot model router). */
export function zeroclawUiSurfaceSpec(
  envelope: PluginSchemaEnvelope,
  surface: PluginUiSurface,
): Spec {
  const full = envelope.schema as PluginSchemaShape;
  const fieldKeys = ZEROCLAW_SURFACE_FIELDS[surface.schema];
  const methodKeys = ZEROCLAW_SURFACE_METHODS[surface.schema] ?? [];

  // Full schema when fieldKeys is null (zeroclaw main).
  if (fieldKeys === null) {
    return schemaToSpec(full, surface.schema);
  }

  // Unknown schema — fall back to full schema with surface display_name.
  if (fieldKeys === undefined) {
    return schemaToSpec(
      {
        ...full,
        name: surface.schema,
        display_name: surface.name,
      },
      surface.schema,
    );
  }

  const allFields = entriesOf<SchemaField>(full.fields);
  const fields = Object.fromEntries(
    allFields.filter(([name]) => fieldKeys.includes(name)),
  );

  const allMethods = entriesOf<SchemaMethod>(full.methods);
  const methods = Object.fromEntries(
    allMethods.filter(([name]) => methodKeys.includes(name)),
  );

  return schemaToSpec(
    {
      name: surface.schema,
      display_name: surface.name,
      description: `Chatbot model surface ${surface.path} · SpecRenderer`,
      fields,
      methods,
    },
    surface.schema,
  );
}

/**
 * Gallery inference source: sealed zeroclaw projection (active chatbot model
 * router), not gemma_brain.GetUiSpec.
 */
export async function fetchChatbotModelGalleryBundle(): Promise<ChatbotModelGalleryBundle> {
  const envelope = await fetchPluginSchema("tched_router");
  // Drop stale /chat ownership — chat belongs to antigravity ui_surfaces.
  const routes = parsePluginUiSurfaces(envelope).filter((r) => r.path !== "/chat");
  if (routes.length === 0) {
    throw new Error("tched_router schema declares no ui_surfaces");
  }

  const selection = await resolveZeroclawChatSelection();
  const provider =
    selection.provider || readSchemaDefaultString(envelope, "selected_provider");
  const model = selection.model || readSchemaDefaultString(envelope, "selected_model");

  return {
    schema_hash: envelope.schema_hash,
    provider,
    model,
    surfaces: routes.map((surface) => ({
      id: `zeroclaw${surface.path}`,
      name: surface.name,
      path: surface.path,
      schema: surface.schema,
      spec: zeroclawUiSurfaceSpec(envelope, surface),
    })),
  };
}

/** Gallery entries backed by sealed blob catalog content, not route descriptors. */
export async function fetchSealedPluginGalleryEntries(
  plugins: string[],
): Promise<SealedPluginGalleryEntry[]> {
  const settled = await Promise.allSettled(plugins.map((plugin) => fetchPluginSchema(plugin)));
  const entries: SealedPluginGalleryEntry[] = [];

  settled.forEach((result, index) => {
    if (result.status !== "fulfilled") return;
    const plugin = plugins[index];
    entries.push({
      id: `${plugin}/sealed-blob`,
      plugin,
      spec: sealedPluginSpec(plugin, result.value),
      schema_hash: result.value.schema_hash,
    });
  });

  if (entries.length === 0 && settled.some((result) => result.status === "rejected")) {
    const first = settled.find((result) => result.status === "rejected") as PromiseRejectedResult;
    throw first.reason;
  }

  return entries;
}
