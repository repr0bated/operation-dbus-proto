# JSON-Render GUI Integration Requirements

## Overview
Integrate `@json-render` with the Antigravity UI to enable AI-driven progressive UI generation. The server streams JSONL patches (SpecStream format) and the client compiles them into live UI elements using the component catalog.

## Goals
1. Enable LLM-driven UI generation via TchedRouter plugin
2. Stream incremental JSONL patches from server to client
3. Progressively render UI components as they arrive
4. Support both Standalone (full UI) and Inline (chat + UI) modes

## Functional Requirements

### REQ-01: Server-Side SpecStream Generation
- TchedRouter plugin generates JSONL patches via `pipeJsonRender`
- Each patch follows RFC 6902 JSON Patch format
- Patches target paths like `/root`, `/elements/{id}`, `/elements/{id}/props`
- Server uses `createUIMessageStream` for inline mode with chat

### REQ-02: Component Catalog Integration
- Define catalog using `defineCatalog` with all Antigravity UI components
- Catalog generates system prompt via `catalog.prompt()`
- Support custom rules for consistent styling/layout
- Map catalog components to existing Leptos/WASM registry

### REQ-03: Client-Side Progressive Rendering
- Use `useUIStream` hook for standalone generation
- Use `useJsonRenderMessage` for inline chat mode
- `Renderer` component updates as spec changes
- Support loading states during streaming
- Handle abort/cancel of in-progress streams

### REQ-04: BoundedChildren Integration
- `ChildSummary` type maps to json-render component instances
- `BoundedChildren.items` feeds `repeat` binding in templates
- Support cursor-based pagination via `next_cursor`
- Actions array maps to button handlers

### REQ-05: State Binding
- `$state` expressions in templates bind to plugin projection data
- Live updates when projection changes via D-Bus signals
- Support nested field access in data payloads

## Non-Functional Requirements

### NFR-01: Performance
- First meaningful paint within 100ms of stream start
- Incremental updates without full re-render
- Efficient patch application (O(1) per patch)

### NFR-02: Error Handling
- Graceful degradation on malformed patches
- Clear error states in UI
- Automatic retry with exponential backoff

### NFR-03: Compatibility
- Works with existing op-web Leptos frontend
- Bridges to WASM component registry
- Compatible with Vercel AI SDK patterns

## Architecture

### Server Flow
```
TchedRouter Plugin
    ↓ generates spec
catalog.prompt() → LLM → JSONL patches
    ↓ streams via
pipeJsonRender → createUIMessageStream
    ↓
HTTP/gRPC streaming response
```

### Client Flow
```
useUIStream / useJsonRenderMessage
    ↓ compiles patches
createSpecStreamCompiler
    ↓ updates state
Renderer component
    ↓ maps to
Component Registry (Leptos/WASM)
```

## Integration Points

### D-Bus
- `/org/odbus/plugins/tched_router` - plugin state projection
- Signals for state change notifications
- Method calls for action dispatch

### gRPC
- `TchedRouterService.StreamUI` - server-streaming RPC
- Carries JSONL patches as bytes
- Supports bidirectional for inline mode

### REST
- `POST /api/generate` - standalone mode
- `POST /api/chat` - inline mode with messages array
- SSE for streaming responses

## Component Catalog (Initial)

| Component | Props | Children |
|-----------|-------|----------|
| Card | title, subtitle, padding | Yes |
| Metric | label, value, trend, format | No |
| Chart | type, data, xAxis, yAxis | No |
| Table | columns, data, sortable | No |
| Row | gap, align, justify | Yes |
| Column | gap, align | Yes |
| Button | label, action, variant | No |
| Text | content, variant, color | No |
| Badge | label, color | No |
| Progress | value, max, label | No |

## References
- json-render docs: Streaming, AI SDK Integration
- RFC 6902: JSON Patch
- RFC 6901: JSON Pointer
- TchedRouter plugin: `crates/op-plugins/src/state_plugins/tched_router.rs`
- Child types: `crates/op-plugins/src/state_plugins/json_render.rs`
