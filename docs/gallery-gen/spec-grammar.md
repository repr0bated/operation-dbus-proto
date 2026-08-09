# json-render.dev Spec Grammar

This document defines the formal grammar for json-render.dev specifications. Every generated spec must conform to this grammar to be admitted to the gallery.

## Top-Level Structure

```json
{
  "root": "<element-id>",
  "elements": {
    "<id>": { "<element-def>" }
  }
}
```

### Constraints

- `root` MUST reference an element ID defined in `elements`
- `elements` MUST be a non-empty object
- All element IDs MUST be unique strings
- No forward references (all referenced IDs must exist)
- No cycles in the element tree

## Element Definition

```json
{
  "type": "<component-name>",
  "props": { <component-props> },
  "children": ["<id>", ...],
  "visible": true | false | "<bind-path>",
  "repeat": {
    "bind": "<array-path>",
    "as": "<item-var>"
  },
  "watch": ["<bind-path>", ...]
}
```

### Required Fields

| Field | Type | Constraint |
|-------|------|------------|
| `type` | string | MUST be a legal component from the catalog |

### Optional Fields

| Field | Type | Constraint |
|-------|------|------------|
| `props` | object | MUST match component's prop schema |
| `children` | array | Array of element IDs |
| `visible` | boolean or string | If string, MUST be a bind path |
| `repeat` | object | See repeat spec below |
| `watch` | array | Array of bind paths |

## Component Names (Stable Core)

These are the stable-core component types guaranteed to be available:

**Layout:**
- `stack`, `card`, `separator`, `space`

**Text:**
- `heading`, `label`, `muted`

**Status:**
- `status_pill`

**Action:**
- `button`, `button_group`

**Data Display:**
- `kv_pair`, `table`, `log_stream`, `flow_table`, `metric_card`

**Form:**
- `text_input`, `number_input`, `select`, `toggle`

**Dynamic:**
- `repeat`, `schema_form`

## Prop Validation Rules

### Common Prop Patterns

#### Bind Path
A bind path is a JSON pointer string starting with `/`:

```json
{ "bind": "/status/health" }
```

Valid patterns:
- `/field` — top-level field
- `/nested/field` — nested field
- `/array/0/item` — array index
- `../../relative` — relative path (inside repeat context)

#### Text Props
Components that display text accept either:
- `text`: static string
- `bind`: live data path

These are mutually exclusive. A validation error occurs if both or neither are present.

#### Array Binding
Components that iterate over arrays (`table`, `repeat`, `flow_table`) require:
- `bind`: path to an array field in plugin state

The interpreter validates the bound value is an array at render time.

### Component-Specific Prop Schemas

#### stack
```json
{
  "dir": { "enum": ["v", "h"], "default": "v" },
  "gap": { "type": "number", "minimum": 0 }
}
```

#### card
```json
{
  "title": { "type": "string" },
  "description": { "type": "string" }
}
```

#### heading
```json
{
  "text": { "type": "string", "required": true },
  "size": { "type": "number", "minimum": 10, "maximum": 48, "default": 16 }
}
```

#### label
```json
{
  "text": { "type": "string" },
  "bind": { "type": "string", "pattern": "^/" }
}
```
One of `text` or `bind` required.

#### status_pill
```json
{
  "bind": { "type": "string", "pattern": "^/", "required": true }
}
```

#### button
```json
{
  "label": { "type": "string", "required": true },
  "variant": { "enum": ["default", "outline", "destructive"], "default": "default" },
  "on_click": { "type": "object" }
}
```

#### table
```json
{
  "bind": { "type": "string", "pattern": "^/", "required": true },
  "columns": {
    "type": "array",
    "items": {
      "type": "object",
      "properties": {
        "key": { "type": "string", "required": true },
        "label": { "type": "string", "required": true },
        "width": { "type": "number" }
      }
    },
    "required": true
  }
}
```

#### repeat
```json
{
  "bind": { "type": "string", "pattern": "^/", "required": true },
  "child": { "$ref": "#/definitions/element", "required": true }
}
```

## Children Validation

- `children` MUST be an array of strings
- Each string MUST reference an existing element ID
- Components that don't accept children (`label`, `button`, etc.) MUST NOT have a `children` field
- Components that require children (`stack`, `card`) MUST have a non-empty `children` field

## Visibility Rules

The `visible` field controls conditional rendering:

```json
{ "visible": true }           // always visible
{ "visible": false }          // never visible
{ "visible": "/status/ok" }   // visible when status.ok is truthy
```

## Repeat Semantics

The `repeat` field creates a repeated template:

```json
{
  "type": "stack",
  "repeat": {
    "bind": "/items",
    "as": "item"
  },
  "children": ["item-card"]
}
```

Inside the repeated context:
- Relative bind paths resolve against each array item
- The `as` variable name can be used in nested expressions

## Watch Bindings

The `watch` array declares which paths the element depends on:

```json
{
  "type": "label",
  "props": { "bind": "/counter" },
  "watch": ["/counter"]
}
```

When any watched path changes, the element re-renders.

## Validation Algorithm

When a spec is submitted for gallery admission:

1. **Structure check**: Verify top-level `root` and `elements` fields exist
2. **Reference check**: Ensure `root` exists in `elements`, all children IDs exist
3. **Type check**: Verify every `type` is a known component
4. **Prop schema check**: Validate each element's `props` against component schema
5. **Children check**: Verify children are allowed and valid for each component
6. **Bind path check**: Ensure all bind paths start with `/`
7. **Cycle check**: Traverse element tree, reject if cycles detected
8. **Signature check**: Dedupe against existing gallery signatures

If all checks pass, the spec is eligible for admission to the gallery.

## Error Messages

| Error Code | Message |
|------------|---------|
| `E_UNKNOWN_TYPE` | Unknown component type: `<type>` |
| `E_MISSING_ROOT` | Root element ID not found in elements |
| `E_DANGLING_REF` | Element `<id>` references non-existent child `<child-id>` |
| `E_CYCLE` | Cycle detected in element tree: `<path>` |
| `E_PROP_SCHEMA` | Invalid props for `<type>`: `<reason>` |
| `E_CHILDREN_NOT_ALLOWED` | Component `<type>` does not accept children |
| `E_CHILDREN_REQUIRED` | Component `<type>` requires children |
| `E_BIND_PATH` | Invalid bind path: `<path>` |
| `E_DUPLICATE_SIGNATURE` | Spec signature already exists in gallery |
