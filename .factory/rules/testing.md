# Testing Conventions

## File Organization
- Test files live next to source: `Component.tsx` → `Component.test.tsx`.
- Integration tests go in `__tests__/integration/` or `crates/tests/`.
- Rust workspace unit/integration tests must adhere to behavior-focused naming: `should_handle_<scenario>`.

## Test Structure
- Use descriptive test names: "should [action] when [condition]".
- One assertion per test when possible.
- Use `beforeEach` for common setup, not `beforeAll`.

## Mocking
- Mock at the boundary (API calls, not internal functions).
- Reset mocks in `afterEach`.
