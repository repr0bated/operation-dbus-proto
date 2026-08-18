# Why "3tched" became "tched" in the plugin rebrand

The **subject** section of the subid — which maps directly to the plugin name in the D-Bus object path `/org/opdbus/v1/plugins/<name>` — cannot begin with a number.

Per the D-Bus specification, object path elements must not begin with a digit (only `[A-Za-z_]` as the first character, then `[A-Za-z0-9_]` after). The project name "3tched" starts with `3`, so `3tched_router` as a plugin name would produce an invalid D-Bus path:

```
/org/opdbus/v1/plugins/3tched_router   ← INVALID: element starts with a digit
/org/opdbus/v1/plugins/tched_router     ← valid
```

That's why the plugin rebrand dropped the `3` — `3tched` → `tched_router`. The subid `subject` inherits the same constraint since it's used as the D-Bus path element and as the OSCAL prop value that resolves to that path.
