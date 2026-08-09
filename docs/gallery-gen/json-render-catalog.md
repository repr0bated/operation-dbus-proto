# json-render.dev Component Catalog

This document lists every legal component type in the json-render.dev DSL. A generated spec may only use these component names in the `type` field. Using an unknown component is a validation error.

## Spec Format

Every spec is a flat element tree:

```json
{
  "root": "<element-id>",
  "elements": {
    "<id>": {
      "type": "<component-name>",
      "props": { <component-specific properties> },
      "children": ["<child-id>", ...]
    }
  }
}
```

## Layout Components

### `stack`
Vertical or horizontal container.

**Props:**
- `dir`: `"v"` (default) or `"h"` — stacking direction
- `gap`: Optional spacing between children

**Children:** Any elements.

---

### `card`
Bordered, padded frame for grouping.

**Props:**
- `title`: Optional heading text
- `description`: Optional subtitle

**Children:** Any elements.

---

### `separator`
Visual divider between sections.

**Props:** None.

**Children:** None.

---

### `space`
Spacer for padding.

**Props:**
- `px`: Pixels of space (default: 8)

**Children:** None.

---

## Text Components

### `heading`
Sized bold text for section titles.

**Props:**
- `text`: The heading content
- `size`: Font size (default: 16)

**Children:** None.

---

### `label`
Plain text, optionally data-bound.

**Props:**
- `text`: Static text (mutually exclusive with `bind`)
- `bind`: JSON pointer to live value

**Children:** None.

---

### `muted`
Small dim text for hints and secondary info.

**Props:**
- `text`: The muted text content

**Children:** None.

---

## Status Components

### `status_pill`
Colored status indicator for health states.

**Props:**
- `bind`: JSON pointer to a status string

**Renders:**
- `"ok"`, `"up"`, `"healthy"` → green "OK"
- `"warn"`, `"degraded"` → yellow "WARN"
- `"err"`, `"down"`, `"failed"` → red "ERR"
- Other non-empty strings → muted, as-is

**Children:** None.

---

## Action Components

### `button`
Clickable action trigger.

**Props:**
- `label`: Button text
- `variant`: `"default"` | `"outline"` | `"destructive"`
- `on_click`: Action payload (when wired)

**Children:** None.

---

### `button_group`
Row of related action buttons.

**Props:**
- `buttons`: Array of `{ label, variant }` objects
- `selected`: Index of currently selected button

**Children:** None.

---

## Data Display Components

### `kv_pair`
Key-value pair display.

**Props:**
- `key`: Label text
- `value`: The value to display
- `mono`: Use monospace font (default: false)

**Children:** None.

---

### `table`
Tabular data display.

**Props:**
- `bind`: JSON pointer to an array
- `columns`: Array of `{ key, label, width? }` column definitions

**Children:** None.

---

### `log_stream`
Real-time log viewer.

**Props:**
- `lines`: Array of `{ timestamp, level, source, message }` objects
- `autoScroll`: Auto-scroll to bottom (default: true)
- `maxHeight`: CSS max-height (e.g., `"500px"`)

**Children:** None.

---

### `flow_table`
OpenFlow flow table display.

**Props:**
- `bind`: JSON pointer to flows array
- `flows`: Inline flows array (mutually exclusive with `bind`)

Each flow: `{ priority, match, actions, packetCount, byteCount }`

**Children:** None.

---

### `metric_card`
Single metric with optional thresholds.

**Props:**
- `label`: Metric name
- `value`: Current value
- `unit`: Optional unit suffix
- `warningThreshold`: Yellow threshold
- `criticalThreshold`: Red threshold

**Children:** None.

---

## Form Components

### `text_input`
Text input field.

**Props:**
- `label`: Field label
- `bind`: JSON pointer for value binding
- `placeholder`: Placeholder text
- `required`: Mark as required

**Children:** None.

---

### `number_input`
Numeric input with optional bounds.

**Props:**
- `label`: Field label
- `bind`: JSON pointer for value binding
- `min`: Minimum value
- `max`: Maximum value
- `step`: Step increment

**Children:** None.

---

### `select`
Dropdown selection.

**Props:**
- `label`: Field label
- `bind`: JSON pointer for value binding
- `options`: Array of `{ value, label }` objects

**Children:** None.

---

### `toggle`
On/off switch.

**Props:**
- `label`: Field label
- `bind`: JSON pointer for boolean value

**Children:** None.

---

## Repeat / Dynamic Components

### `repeat`
Iterate over an array, rendering a child template for each item.

**Props:**
- `bind`: JSON pointer to an array
- `child`: Element template (use relative `bind` paths like `/name` for each item)

**Children:** None (child is in props).

---

### `schema_form`
Auto-generated form from a JSON Schema.

**Props:**
- `schema`: The JSON Schema object
- `bind`: JSON pointer for the form value
- `title`: Optional form title

**Children:** None.

---

## Stable Core vs Novelty

~40 components are **StableCore** — they are protected from gallery rotation and always available. Prefer these for reliable rendering across environments.

**StableCore primitives:**
- Layout: `stack`, `card`, `separator`, `space`
- Text: `heading`, `label`, `muted`
- Status: `status_pill`
- Actions: `button`
- Data: `kv_pair`, `table`
- Forms: `text_input`, `number_input`, `select`, `toggle`
- Dynamic: `repeat`

**Novelty components** may be added by gemma but are not guaranteed. If you use a non-stable-core component, the spec may fail to render on older interpreters.

## Element Fields

Every element may have these fields:

| Field | Required | Purpose |
|-------|----------|---------|
| `type` | Yes | Component name from this catalog |
| `props` | No | Component-specific properties |
| `children` | No | Array of child element IDs |
| `visible` | No | Conditional visibility (boolean or bind path) |
| `on` | No | Event handlers (e.g., `on_click`) |
| `repeat` | No | Repeat configuration (alternative to `repeat` component) |
| `watch` | No | Live update bindings |
