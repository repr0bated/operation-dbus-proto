# TypeScript Conventions

## General
- Use `interface` for object types, `type` for unions/intersections.
- Avoid `any` - use `unknown` with type guards instead.
- Export types alongside their implementations.

## React Components
- Use functional components with TypeScript FC type.
- Props interfaces should be named `{ComponentName}Props`.
- Use `React.ReactNode` for children, not `React.ReactChild`.

## Imports
- Group imports: React, external libs, internal modules, types.
- Use absolute imports from `@/` prefix.
- Avoid barrel files (index.ts re-exports) for performance.
