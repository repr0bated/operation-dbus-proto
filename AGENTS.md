# Agent host-service policy

Agents must manage s6 services exclusively through `sudo service6 ...`.

Do not invoke raw `s6`, `s6-*`, `s6d`, their absolute paths, renamed copies,
or a shell/interpreter used to bypass this policy. Do not edit the live s6-rc
database or `/run/service` directly. Native s6 commands are reserved for boot,
supervisors, and explicit human console recovery.

Container and application deployment must use D-Bus through `busctl` for
service-manager operations. Do not deploy service lifecycle calls through
`systemctl` or other service-manager CLIs. Host s6 services remain governed by
the `sudo service6 ...` rule above.

# Xray configuration policy — mandatory

**XRAY'S LIVE CONFIGURATION MUST EXIST ONLY AT
`/etc/xray/xray_config.json` inside the container.** Never point Xray at `/dev/shm/xray_config.json`,
`/usr/local/etc/xray/config.json`, or another disk-backed live path.

Until model-generated dynamic tag routing is implemented, the static bootstrap
configuration is correct and must be materialized into the container path during
boot. Later, the validated model/control-plane generator replaces that same file
atomically and reloads Xray through D-Bus. Models must not write or reload
Xray directly.
