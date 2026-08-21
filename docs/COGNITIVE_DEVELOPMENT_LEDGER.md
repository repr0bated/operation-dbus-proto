# Cognitive development ledger

The Cognitive development ledger is the structured record of capability work.
It lives in the bridge-owned Cozo store; it is not a second memory or model
controlled namespace.

## Operations

The `cognitive_development` tool supports:

- `upsert`: register or update a capability and its category, schema surface,
  dependencies, tests, owner, and deployment metadata.
- `list`: list capabilities, optionally filtered by `status` and `category`.
- `summary`: return category/status counts for dashboards.
- `history`: return the append-only evidence trail for one capability.
- `record_verification`: record live verification evidence. Any failed check
  forces the capability to `blocked`.
- `categories`: return the canonical category taxonomy.

Models and clients submit evidence; they do not execute arbitrary commands
through the ledger. The repository verification entry point is:

```sh
scripts/verify-cognitive-development.sh
```

That check covers the ledger, the sealed plugin contract, and canonical bridge
compilation.

The relation is created during Cozo startup. Releases that change its columns
must use the normal database migration/release path; do not hand-edit the live
database or create a parallel ledger.
