# JSON-Render GUI Integration Design

## Overview

This design specifies how TchedRouter integrates with `@json-render` to enable AI-driven progressive UI generation. The architecture bridges the Rust-based op-plugins ecosystem with the json-render streaming protocol, allowing LLMs to generate UI specifications that stream incrementally to the Antigravity frontend.

---

## Core Concepts

### SpecStream JSONL Format

The server streams UI specifications as newline-delimited JSON (JSONL). Each line is either a complete spec or an RFC 6902 JSON Patch operation:

```jsonl
{"op":"replace","path":"/root","value":"card_1"}
{"op":"add","path":"/elements/card_1","value":{"type":"Card","props":{"title":"Loading..."}}}
{"op":"replace","path":"/elements/card_1/props/title","value":"Network Status"}
{"op":"add","path":"/elements/card_1/children","value":["metric_1","metric_2"]}
```

**Patch Operations (RFC 6902):**
- `add` — Insert element or property
- `replace` — Update existing value
- `remove` — Delete element or property
- `move` — Relocate element in tree
- `copy` — Duplicate element

**Path Format (RFC 6901 JSON Pointer):**
- `/root` — Root element ID
- `/elements/{id}` — Element definition
- `/elements/{id}/props/{prop}` — Element property
- `/elements/{id}/children` — Child element array

### Component Catalog

The catalog defines available UI components and their schemas. It serves two purposes:
1. **LLM Guidance** — `catalog.prompt()` generates system prompt describing available components
2. **Runtime Validation** — Ensures streamed specs use valid component types and props

```typescript
import { defineCatalog } from '@json-render/core';

export const catalog = defineCatalog({
  Card: {
    props: {
      title: { type: 'string', required: true },
      subtitle: { type: 'string' },
      padding: { type: 'string', enum: ['none', 'sm', 'md', 'lg'] },
    },
    children: true,
  },
  Metric: {
    props: {
      label: { type: 'string', required: true },
      value: { type: 'string', required: true },
      trend: { type: 'string', enum: ['up', 'down', 'flat'] },
      format: { type: 'string', enum: ['number', 'percent', 'currency', 'bytes'] },
    },
    children: false,
  },
  // ... additional components
});
```

---

## Architecture

### Server-Side: pipeJsonRender

The `pipeJsonRender` function transforms LLM output into a validated SpecStream:

```
┌─────────────────────────────────────────────────────────────────┐
│                      TchedRouter Plugin                          │
├─────────────────────────────────────────────────────────────────┤
│  PluginProjection (BoundedChildren, ChildSummary[])             │
│           │                                                      │
│           ▼                                                      │
│  catalog.prompt() ──► LLM Context                               │
│           │                                                      │
│           ▼                                                      │
│  streamText() ──► Raw LLM tokens                                │
│           │                                                      │
│           ▼                                                      │
│  pipeJsonRender(catalog) ──► Validated JSONL patches            │
│           │                                                      │
│           ▼                                                      │
│  toDataStreamResponse() / gRPC StreamUI                         │
└─────────────────────────────────────────────────────────────────┘
```

**Server Implementation (Next.js API Route):**

```typescript
import { streamText } from 'ai';
import { pipeJsonRender, defineCatalog } from '@json-render/ai';
import { openai } from '@ai-sdk/openai';

export async function POST(req: Request) {
  const { prompt, context } = await req.json();
  
  const result = streamText({
    model: openai('gpt-4o'),
    system: catalog.prompt() + `\n\nContext:\n${JSON.stringify(context)}`,
    prompt,
  });

  // Transform LLM output to validated SpecStream
  return pipeJsonRender(result, catalog).toDataStreamResponse();
}
```

**gRPC Streaming (op-grpc-bridge):**

```rust
// crates/op-grpc-bridge/src/tched_router_service.rs
impl TchedRouterService for TchedRouterServiceImpl {
    type StreamUIStream = ReceiverStream<Result<UiPatch, Status>>;

    async fn stream_ui(
        &self,
        request: Request<UiGenerationRequest>,
    ) -> Result<Response<Self::StreamUIStream>, Status> {
        let (tx, rx) = mpsc::channel(32);
        let projection = self.get_projection().await?;
        
        tokio::spawn(async move {
            // Stream patches as LLM generates them
            let mut patch_stream = generate_ui_patches(projection).await;
            while let Some(patch) = patch_stream.next().await {
                if tx.send(Ok(patch)).await.is_err() {
                    break; // Client disconnected
                }
            }
        });
        
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
```

### Client-Side: useUIStream

The `useUIStream` hook manages SpecStream consumption and progressive rendering:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Antigravity Frontend                          │
├─────────────────────────────────────────────────────────────────┤
│  useUIStream(endpoint)                                          │
│           │                                                      │
│           ▼                                                      │
│  createSpecStreamCompiler() ──► Spec state machine              │
│           │                                                      │
│           ▼                                                      │
│  spec: Spec ──► Current compiled specification                  │
│           │                                                      │
│           ▼                                                      │
│  <Renderer spec={spec} components={registry} />                 │
│           │                                                      │
│           ▼                                                      │
│  Component Registry (Leptos/WASM)                               │
└─────────────────────────────────────────────────────────────────┘
```

**Standalone Mode (Full UI Generation):**

```tsx
import { useUIStream, Renderer } from '@json-render/react';
import { components } from './component-registry';

export function GeneratedUI({ endpoint }: { endpoint: string }) {
  const { spec, isLoading, error, abort } = useUIStream({
    api: endpoint,
    onPatch: (patch) => console.log('Received patch:', patch),
    onComplete: (finalSpec) => console.log('Generation complete:', finalSpec),
    onError: (err) => console.error('Stream error:', err),
  });

  if (error) return <ErrorState error={error} />;
  if (!spec && isLoading) return <LoadingState />;

  return (
    <div>
      {isLoading && <StreamingIndicator onAbort={abort} />}
      <Renderer spec={spec} components={components} />
    </div>
  );
}
```

**Inline Mode (Chat + UI):**

```tsx
import { useChat } from '@ai-sdk/react';
import { useJsonRenderMessage, Renderer } from '@json-render/react';
import { components } from './component-registry';

export function ChatWithUI() {
  const { messages, sendMessage, isLoading } = useChat({
    api: '/api/chat',
  });

  return (
    <div className="flex">
      {/* Chat panel */}
      <div className="w-1/2">
        {messages.map((m) => (
          <MessageBubble key={m.id} message={m} />
        ))}
      </div>
      
      {/* Generated UI panel */}
      <div className="w-1/2">
        {messages.filter(m => m.role === 'assistant').map((m) => {
          const { spec } = useJsonRenderMessage(m);
          return spec ? (
            <Renderer key={m.id} spec={spec} components={components} />
          ) : null;
        })}
      </div>
    </div>
  );
}
```

### SpecStream Compiler

The `createSpecStreamCompiler` function maintains spec state and applies patches incrementally:

```typescript
import { createSpecStreamCompiler } from '@json-render/core';

const compiler = createSpecStreamCompiler({
  onPatch: (patch, spec) => {
    // Called after each patch is applied
    console.log(`Applied ${patch.op} at ${patch.path}`);
  },
  onError: (error, patch) => {
    // Called when a patch fails validation
    console.error(`Invalid patch: ${error.message}`, patch);
  },
});

// Feed JSONL lines as they arrive
for await (const line of stream) {
  const patch = JSON.parse(line);
  compiler.apply(patch);
}

// Get final compiled spec
const spec = compiler.getSpec();
```

---

## Data Binding

### BoundedChildren → repeat Binding

The `BoundedChildren` type from json_render.rs maps to json-render's `repeat` binding:

```rust
// crates/op-plugins/src/state_plugins/json_render.rs
pub struct BoundedChildren {
    pub items: Vec<ChildSummary>,
    pub total_count: u32,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

pub struct ChildSummary {
    pub id: String,
    pub name: String,
    pub status: ChildStatus,
    pub data: serde_json::Value,  // Flexible payload
    pub actions: Vec<ChildAction>,
}
```

**json-render Spec with repeat:**

```json
{
  "root": "container",
  "elements": {
    "container": {
      "type": "Column",
      "children": ["child_template"]
    },
    "child_template": {
      "type": "Card",
      "repeat": {
        "bind": "$state.children.items",
        "as": "item",
        "key": "item.id"
      },
      "props": {
        "title": { "$bind": "item.name" }
      },
      "children": ["status_badge", "action_buttons"]
    },
    "status_badge": {
      "type": "Badge",
      "props": {
        "label": { "$bind": "item.status" },
        "color": {
          "$switch": {
            "value": { "$bind": "item.status" },
            "cases": {
              "active": "green",
              "pending": "yellow",
              "error": "red"
            },
            "default": "gray"
          }
        }
      }
    },
    "action_buttons": {
      "type": "Row",
      "children": ["action_template"]
    },
    "action_template": {
      "type": "Button",
      "repeat": {
        "bind": "item.actions",
        "as": "action"
      },
      "props": {
        "label": { "$bind": "action.label" },
        "variant": { "$bind": "action.variant" }
      },
      "on": {
        "click": {
          "action": "dispatch",
          "payload": {
            "type": { "$bind": "action.action_type" },
            "target": { "$bind": "item.id" }
          }
        }
      }
    }
  }
}
```

### State Binding Expressions

`$state` expressions bind to TchedRouter's plugin projection:

| Expression | Source |
|------------|--------|
| `$state.children` | `BoundedChildren` struct |
| `$state.children.items` | `Vec<ChildSummary>` |
| `$state.children.total_count` | Pagination total |
| `$state.children.has_more` | More pages available |
| `$state.selected` | Currently selected item ID |
| `$state.loading` | Async operation in progress |

**D-Bus Signal Integration:**

```rust
// When projection changes, emit D-Bus signal
impl TchedRouter {
    fn emit_projection_changed(&self, projection: &PluginProjection) {
        let signal = ProjectionChangedSignal {
            plugin_id: self.id.clone(),
            projection: serde_json::to_value(projection).unwrap(),
        };
        self.dbus_connection.emit_signal(signal);
    }
}
```

**Client Subscription:**

```typescript
// Subscribe to projection changes via WebSocket/SSE
useEffect(() => {
  const unsubscribe = subscribeToProjection(pluginId, (newProjection) => {
    setProjection(newProjection);
  });
  return unsubscribe;
}, [pluginId]);
```

---

## Component Registry Bridge

### Leptos/WASM Component Mapping

The Antigravity frontend uses Leptos (Rust WASM). json-render components map to the existing registry:

```rust
// crates/op-web/src/components/json_render_bridge.rs
use leptos::*;
use json_render_core::{Spec, Element};

#[component]
pub fn JsonRenderBridge(spec: Spec) -> impl IntoView {
    let root_id = spec.root.clone();
    let elements = spec.elements.clone();
    
    view! {
        <RenderElement 
            element_id=root_id 
            elements=elements 
        />
    }
}

#[component]
fn RenderElement(element_id: String, elements: HashMap<String, Element>) -> impl IntoView {
    let element = elements.get(&element_id)?;
    
    match element.type_.as_str() {
        "Card" => view! { <Card props=element.props.clone() /> },
        "Metric" => view! { <Metric props=element.props.clone() /> },
        "Chart" => view! { <Chart props=element.props.clone() /> },
        "Table" => view! { <Table props=element.props.clone() /> },
        "Row" => view! { <Row props=element.props.clone() /> },
        "Column" => view! { <Column props=element.props.clone() /> },
        "Button" => view! { <Button props=element.props.clone() /> },
        "Text" => view! { <Text props=element.props.clone() /> },
        "Badge" => view! { <Badge props=element.props.clone() /> },
        "Progress" => view! { <Progress props=element.props.clone() /> },
        _ => view! { <UnknownComponent type_=element.type_.clone() /> },
    }
}
```

### Action Dispatch

Actions from json-render specs route through D-Bus:

```rust
// crates/op-web/src/actions/dispatcher.rs
pub async fn dispatch_action(action: JsonRenderAction) -> Result<(), ActionError> {
    match action.action_type.as_str() {
        "dbus.call" => {
            let method = action.payload.get("method").unwrap();
            let args = action.payload.get("args").unwrap();
            dbus_call(method, args).await
        }
        "grpc.call" => {
            let service = action.payload.get("service").unwrap();
            let method = action.payload.get("method").unwrap();
            grpc_call(service, method, &action.payload).await
        }
        "navigate" => {
            let path = action.payload.get("path").unwrap();
            router_navigate(path)
        }
        _ => Err(ActionError::UnknownType(action.action_type))
    }
}
```

---

## Error Handling

### Malformed Patch Recovery

```typescript
const compiler = createSpecStreamCompiler({
  onError: (error, patch) => {
    // Log but don't crash - skip invalid patch
    console.warn(`Skipping invalid patch: ${error.message}`);
    
    // Optionally notify user
    if (error.type === 'UNKNOWN_COMPONENT') {
      showToast(`Unknown component type: ${patch.value?.type}`);
    }
  },
  strict: false, // Continue on errors
});
```

### Stream Interruption

```typescript
const { spec, error, retry } = useUIStream({
  api: '/api/generate',
  retryConfig: {
    maxRetries: 3,
    backoff: 'exponential',
    initialDelay: 1000,
  },
  onRetry: (attempt, error) => {
    console.log(`Retry ${attempt} after error: ${error.message}`);
  },
});

if (error) {
  return (
    <div>
      <p>Generation failed: {error.message}</p>
      <button onClick={retry}>Retry</button>
    </div>
  );
}
```

---

## Implementation Tasks

### Phase 1: Core Infrastructure
1. [ ] Define TypeScript catalog matching Leptos component library
2. [ ] Implement `pipeJsonRender` integration in op-grpc-bridge
3. [ ] Create Leptos `<JsonRenderBridge>` component
4. [ ] Wire D-Bus projection signals to frontend state

### Phase 2: Streaming Integration
5. [ ] Implement `useUIStream` hook for standalone mode
6. [ ] Implement `useJsonRenderMessage` for inline chat mode
7. [ ] Add SpecStream compiler with error recovery
8. [ ] Integrate with existing Antigravity chat UI

### Phase 3: Data Binding
9. [ ] Map `BoundedChildren` to `repeat` binding
10. [ ] Implement `$state` expression evaluation
11. [ ] Add cursor-based pagination controls
12. [ ] Wire action dispatch through D-Bus

### Phase 4: Polish
13. [ ] Loading states and streaming indicators
14. [ ] Error boundaries and graceful degradation
15. [ ] Performance optimization (incremental updates)
16. [ ] Integration tests with TchedRouter plugin

---

## References

- json-render Streaming Docs: useUIStream, pipeJsonRender, SpecStream JSONL
- RFC 6902: JSON Patch - https://datatracker.ietf.org/doc/html/rfc6902
- RFC 6901: JSON Pointer - https://datatracker.ietf.org/doc/html/rfc6901
- AI SDK UI Skill: `/srv/git/odbus/.factory/skills/ai-sdk-ui/SKILL.md`
- Gallery UI Spec: `/srv/git/odbus/.kiro/specs/gallery-ui-generation/requirements.md`
- NetMaker Custom UI: `/srv/git/odbus/.kiro/specs/netmaker-custom-json-render-ui/requirements.md`
- TchedRouter Plugin: `crates/op-plugins/src/state_plugins/tched_router.rs`
- Child Types: `crates/op-plugins/src/state_plugins/json_render.rs` (lines 481-620)
