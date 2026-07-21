# Agent Integration for Deployment Workflow

## Overview

The deployment script (`deploy.sh`) integrates with MCP agents to provide:
- **Memory Agent**: Stores deployment history, recalls previous issues, tracks patterns
- **Backend Architect Agent**: Analyzes deployment architecture and provides feedback
- **Rust Pro Agent**: Code analysis and recommendations (future)

## Workflow

### 1. Pre-Deployment Phase

```bash
# Recall previous deployment issues
agent_recall "deploy:op-web:error" 5
# Returns: Previous errors, build failures, service start issues
```

**Memory Agent** recalls:
- Previous build failures for this component
- Service start issues
- Common error patterns
- Deployment timing data

### 2. Build Phase

```bash
# Store pre-build context
agent_store "pre-build" "op-web" '{"package":"op-web",...}' "deployment" "pre-build"

# After build success
agent_store "post-build" "op-web" '{"build_time":45,"status":"success",...}' "deployment" "success"

# Get architectural feedback
agent_analyze '{"build_time":45,...}' "op-web"
# Returns: Recommendations on build optimization, dependency analysis
```

**Backend Architect Agent** provides:
- Build time analysis and optimization suggestions
- Dependency conflict warnings
- Architecture recommendations
- Performance insights

### 3. Code Quality Phase (After Build, Before Deploy)

```bash
# Run clippy to check for warnings
cargo clippy -p op-web -- -D warnings

# If warnings found, consult Rust Pro agent
agent_fix_rust "op-web" "/tmp/deploy-clippy-op-web.log"
# Returns: Suggested fixes for warnings and linting issues

# Apply automatic fixes
agent_apply_rust_fixes "op-web" "$rust_fixes"
# Attempts: cargo fix, then rebuilds and re-checks
```

**Rust Pro Agent** provides:
- Analysis of clippy warnings
- Suggested code fixes
- Automatic application of safe fixes
- Rebuild and verification

### 4. Deployment Phase

```bash
# After successful deployment
agent_store "post-deploy" "op-web" '{"status":"running",...}' "deployment" "success"
agent_report true "op-web" '{"status":"running",...}'

# Get deployment recommendations
agent_analyze '{"status":"running",...}' "op-web"
# Returns: Service configuration suggestions, monitoring recommendations
```

### 4. Error Handling

```bash
# On build failure
agent_store "error" "op-web" '{"error":"...","build_time":30}' "deployment" "error" "build-failure"
agent_report false "op-web" '{"error":"..."}' "Build failed: ..."

# Backend architect analyzes failure
agent_analyze '{"error":"...","component":"op-web"}' "op-web"
# Returns: Root cause analysis, prevention strategies, recovery steps
```

## Integration Points in deploy.sh

### Current Integration

1. **Pre-deployment check**: Recalls previous issues before building
2. **Post-build storage**: Stores build context and gets feedback
3. **Backend architect feedback**: Analyzes build and provides recommendations
4. **Rust Pro code quality**: Runs clippy, gets fixes from Rust Pro agent, applies them
5. **Post-deploy reporting**: Reports success/failure and gets recommendations
6. **Error analysis**: On failure, gets architectural feedback on root causes

### Example Output

```
[INFO] 🤖 Agent integration enabled (memory + backend architect)
[INFO] 🚀 Deploying op-web...
[WARN] Previous deployment issues found for op-web:
  - Build failed: missing dependency quick-xml
  - Service start timeout: 30s
[INFO] Building op-web...
[INFO] Build successful (45s).
[INFO] 📋 Architectural feedback:
  - Consider caching dependencies to reduce build time
  - Binary size increased 15% - review dependencies
[INFO] 🔍 Running clippy and Rust Pro analysis for op-web...
[WARN] Found 12 clippy warnings, consulting Rust Pro agent...
[INFO] 🦀 Rust Pro agent suggestions:
  - Remove unused imports: debug, warn
  - Use if let instead of match for single pattern
  - Consider using .is_empty() instead of .len() == 0
[INFO] ✓ Applied Rust Pro fixes, rebuilding...
[INFO] ✓ Rebuild successful after fixes
[INFO] ✅ All warnings fixed!
[INFO] ✅ op-web.service is running.
[INFO] 💡 Deployment recommendations:
  - Monitor memory usage (recent increase detected)
  - Consider health check endpoint
```

## Implementation Status

### ✅ Completed
- Agent integration functions in `deploy/lib/agent-integration.sh`
- Integration points in `deploy.sh`
- Memory agent storage/recall workflow
- Backend architect analysis workflow
- Rust Pro agent integration for code quality fixes
- Automatic clippy checking and fix application

### 🚧 In Progress
- `deploy-agent` CLI binary (requires op-agents crate)
- Agent execution via TraitAgentExecutor

### 📋 Future Enhancements
- Rust Pro agent integration for code analysis
- Deployment pattern learning
- Automated optimization suggestions
- Integration with CI/CD pipelines

## Usage

The integration is **automatic** when `deploy-agent` binary is available:

```bash
# Normal deployment (agents auto-integrated)
sudo ./deploy/deploy.sh

# Agents provide feedback automatically:
# - Pre-deployment: Previous issues recalled
# - Post-build: Architectural feedback
# - Post-deploy: Recommendations
# - On error: Failure analysis
```

## Manual Agent Calls

You can also call agents manually:

```bash
# Store deployment context
./target/release/deploy-agent store "pre-build" --component "op-web" \
  --context '{"package":"op-web"}' --tag deployment --tag pre-build

# Recall previous issues
./target/release/deploy-agent recall "deploy:op-web:error" --limit 5

# Get architectural analysis
./target/release/deploy-agent analyze '{"build_time":45}' --component "op-web"

# Report deployment result
./target/release/deploy-agent report --success true --component "op-web" \
  --details '{"status":"running"}'
```

## Feedback Loop to Development

1. **Memory Agent** stores all deployment events
2. **Backend Architect** analyzes patterns and provides recommendations
3. **Feedback** is displayed during deployment and stored for review
4. **Development** can query memory for patterns:
   ```bash
   deploy-agent recall "deploy:error" 20  # All recent errors
   deploy-agent recall "deploy:op-web:build-time" 10  # Build time trends
   ```

## Next Steps

1. Complete `deploy-agent` binary (requires op-agents dependency)
2. Add Rust Pro agent for code-level analysis
3. Create deployment dashboard using stored memory
4. Integrate with development workflow (pre-commit hooks, PR feedback)
