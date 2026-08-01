# SUPERSEDED

This spec has been superseded by `.kiro/specs/remove-projection-static-tree/`.

The op-dbus-mirror crate was deleted entirely. The event-driven goals of this
spec are achieved by the `Updated` signal on `org.opdbus.v1.PluginV1` + direct
shm reads — a simpler architecture that does not require a mirror daemon.

Do NOT implement the tasks in this directory.
