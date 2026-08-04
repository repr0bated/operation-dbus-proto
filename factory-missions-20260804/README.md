# OP-DBUS Factory mission handoff

These are two independent implementation missions for a Droid machine.

Run them in separate worktrees or sequentially on the same checkout:

1. `01-cognitive-memory.md`
2. `02-netmaker-xray-identity.md`

Required checkout: `/srv/git/odbus`, branch `main` on the receiving machine.
Do not deploy either mission automatically. Review the diff and tests first.

The missions must obey the repository `AGENTS.md` instructions. In
particular: runit is managed with `sudo sv`, application lifecycle uses D-Bus
through `busctl`, releases use the btrfs flow, and live Xray configuration is
only `/etc/xray/xray_config.json` inside the Xray container.

