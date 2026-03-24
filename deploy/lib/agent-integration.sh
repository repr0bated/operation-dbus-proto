#!/bin/bash
# deploy/lib/agent-integration.sh
# Agent integration helpers for deploy.sh
# Provides memory and backend architect feedback during deployment

# Agent integration functions
# Get PROJECT_ROOT - this file is in deploy/lib/, so go up two levels to get project root
# deploy.sh sets PROJECT_ROOT before sourcing this file, but we calculate it as fallback
if [[ -z "$PROJECT_ROOT" ]]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
    export PROJECT_ROOT
fi
DEPLOY_AGENT="${PROJECT_ROOT}/target/release/deploy-agent"

# Check if deploy-agent is available
check_deploy_agent() {
    if [[ -f "$DEPLOY_AGENT" ]]; then
        return 0
    else
        # Try to build it
        if cargo build --release --bin deploy-agent 2>/dev/null; then
            return 0
        else
            return 1
        fi
    fi
}

# Store deployment context in memory agent
agent_store() {
    local stage=$1
    local component=$2
    local context=$3
    shift 3
    local tags=("$@")
    
    if check_deploy_agent; then
        local tag_args=""
        for tag in "${tags[@]}"; do
            [[ -n "$tag" ]] && tag_args="$tag_args --tag $tag"
        done
        "$DEPLOY_AGENT" store "$stage" --component "$component" --context "$context" $tag_args 2>/dev/null || true
    fi
}

# Recall deployment context from memory agent
agent_recall() {
    local query=$1
    local limit=${2:-10}
    
    if check_deploy_agent; then
        "$DEPLOY_AGENT" recall "$query" --limit "$limit" 2>/dev/null || echo "{}"
    else
        echo "{}"
    fi
}

# Get architectural feedback
agent_analyze() {
    local summary=$1
    local component=${2:-""}
    
    if check_deploy_agent; then
        # Ensure PROJECT_ROOT is set
        if [[ -z "$PROJECT_ROOT" ]]; then
            SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
            PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
            DEPLOY_AGENT="${PROJECT_ROOT}/target/release/deploy-agent"
        fi
        
        if [[ -n "$component" ]]; then
            "$DEPLOY_AGENT" analyze "$summary" "$component" 2>&1 || echo "{}"
        else
            "$DEPLOY_AGENT" analyze "$summary" 2>&1 || echo "{}"
        fi
    else
        echo "{}"
    fi
}

# Report deployment result
agent_report() {
    local success=$1
    local component=$2
    local details=$3
    local error=${4:-""}
    
    if check_deploy_agent; then
        if [[ "$success" == "true" ]]; then
            "$DEPLOY_AGENT" report --success true --component "$component" --details "$details" 2>/dev/null || true
        else
            "$DEPLOY_AGENT" report --success false --component "$component" --details "$details" --error "$error" 2>/dev/null || true
        fi
    fi
}

# Get deployment summary for feedback
get_deployment_summary() {
    local component=$1
    local status=$2
    local details=$3
    
    cat <<EOF
{
  "component": "$component",
  "status": "$status",
  "timestamp": "$(date -Iseconds)",
  "details": $details
}
EOF
}

# Run Rust Pro agent to fix warnings and clippy issues
agent_fix_rust() {
    local package=$1
    local warnings_log=$2
    
    if check_deploy_agent; then
        # Get clippy output
        local clippy_output=""
        if [[ -f "$warnings_log" ]]; then
            clippy_output=$(cat "$warnings_log" | jq -Rs . 2>/dev/null || echo '{}')
        fi
        
        # Run cargo clippy to get current warnings
        local clippy_result=$(cargo clippy -p "$package" --message-format=json 2>&1 | jq -s . 2>/dev/null || echo '[]')
        
        local rust_analysis=$(cat <<EOF
{
  "package": "$package",
  "warnings_log": $clippy_output,
  "clippy_results": $clippy_result,
  "task": "fix_warnings_and_linting",
  "context": {
    "type": "rust_code_quality",
    "focus": ["warnings", "clippy_lints", "code_style", "best_practices"]
  }
}
EOF
        )
        
        # Call Rust Pro agent directly (not backend_architect)
        # Note: deploy-agent analyze uses backend_architect, we need a rust_pro command
        # For now, use analyze but the agent should route to rust_pro based on context
        "$DEPLOY_AGENT" analyze "$rust_analysis" --component "$package" 2>/dev/null || echo "{}"
    else
        echo "{}"
    fi
}

# Apply Rust Pro fixes
agent_apply_rust_fixes() {
    local package=$1
    local fixes=$2
    
    if check_deploy_agent && [[ "$fixes" != "{}" ]] && [[ "$fixes" != "null" ]]; then
        # Extract suggested fixes from rust pro output
        local suggested_fixes=$(echo "$fixes" | jq -r '.output.fixes[]? // .data.fixes[]? // empty' 2>/dev/null)
        
        if [[ -n "$suggested_fixes" ]]; then
            log_info "🔧 Rust Pro suggested fixes for $package:"
            echo "$suggested_fixes" | head -10
            
            # Try to apply automatic fixes
            if cargo fix --allow-dirty --package "$package" 2>/dev/null; then
                log_info "✓ Applied automatic fixes"
                return 0
            fi
        fi
    fi
    return 1
}
