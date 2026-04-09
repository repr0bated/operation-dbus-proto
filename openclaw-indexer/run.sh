#!/bin/bash
# Run openclaw-indexer with repo-aware chunking skips.
# Usage:
#   ./run.sh full                  # Index repos missing chunks
#   ./run.sh full --repo NAME      # Index one repo if not already chunked
#   ./run.sh full --force          # Reindex all repos
#   ./run.sh status                # Check index stats
#   ./run.sh search "query"        # Search indexed code
#   ./run.sh crawl-only            # Just list repos/files

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

PYTHON_BIN=".venv/bin/python"

# Source secrets if present.
if [[ -f "$HOME/.bash_secrets" ]]; then
    # shellcheck disable=SC1090
    source "$HOME/.bash_secrets"
fi

QDRANT_URL="${QDRANT_URL:-http://10.149.181.190:6333}"

ensure_qdrant() {
    if ! curl -s "$QDRANT_URL/collections" > /dev/null 2>&1; then
        echo "Starting Qdrant container..."
        incus start qdrant 2>/dev/null || true
        sleep 3
        if ! curl -s "$QDRANT_URL/collections" > /dev/null 2>&1; then
            echo "Qdrant not responding at $QDRANT_URL — run deploy/incus/qdrant.sh first"
            exit 1
        fi
    fi
}

repo_has_chunks() {
    local repo_name="$1"

    "$PYTHON_BIN" - "$repo_name" <<'PY'
import sys

from indexer.config import load_config
from indexer.qdrant_store import QdrantStore
from qdrant_client.models import FieldCondition, Filter, MatchValue

repo_name = sys.argv[1]
config = load_config()
store = QdrantStore(config)

try:
    store.client.get_collection(store.collection)
    result = store.client.count(
        collection_name=store.collection,
        count_filter=Filter(
            must=[FieldCondition(key="repo", match=MatchValue(value=repo_name))]
        ),
        exact=True,
    )
except Exception:
    sys.exit(1)

sys.exit(0 if result.count > 0 else 1)
PY
}

list_repo_states() {
    "$PYTHON_BIN" - <<'PY'
import sys

from rich.console import Console

import indexer.crawler as crawler
from indexer.config import load_config
from indexer.qdrant_store import QdrantStore
from qdrant_client.models import FieldCondition, Filter, MatchValue

crawler.console = Console(file=sys.stderr)
config = load_config()
repos = crawler.discover_repos(config)

try:
    store = QdrantStore(config)
    store.client.get_collection(store.collection)
except Exception:
    for repo in repos:
        print(f"{repo.name}\tmissing")
    sys.exit(0)

for repo in repos:
    result = store.client.count(
        collection_name=store.collection,
        count_filter=Filter(
            must=[FieldCondition(key="repo", match=MatchValue(value=repo.name))]
        ),
        exact=True,
    )
    state = "indexed" if result.count > 0 else "missing"
    print(f"{repo.name}\t{state}")
PY
}

ensure_qdrant

if [[ $# -gt 0 && "$1" == "full" ]]; then
    shift

    force_reindex=0
    repo_filter=""
    full_args=()

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --force)
                force_reindex=1
                shift
                ;;
            --repo)
                if [[ $# -lt 2 ]]; then
                    echo "Missing value for --repo" >&2
                    exit 1
                fi
                repo_filter="$2"
                full_args+=("$1" "$2")
                shift 2
                ;;
            --repo=*)
                repo_filter="${1#*=}"
                full_args+=("$1")
                shift
                ;;
            *)
                full_args+=("$1")
                shift
                ;;
        esac
    done

    if [[ -n "$repo_filter" ]]; then
        if [[ "$force_reindex" -eq 0 ]] && repo_has_chunks "$repo_filter"; then
            echo "Skipping $repo_filter: chunks already exist in Qdrant"
            exit 0
        fi

        exec "$PYTHON_BIN" scripts/run_index.py full "${full_args[@]}"
    fi

    mapfile -t repo_states < <(list_repo_states)

    pending_count=0
    indexed_count=0
    for repo_state in "${repo_states[@]}"; do
        IFS=$'\t' read -r repo_name repo_state_value <<< "$repo_state"

        if [[ "$force_reindex" -eq 0 && "$repo_state_value" == "indexed" ]]; then
            echo "Skipping $repo_name: chunks already exist in Qdrant"
            indexed_count=$((indexed_count + 1))
            continue
        fi

        echo "Indexing $repo_name..."
        "$PYTHON_BIN" scripts/run_index.py full "${full_args[@]}" --repo "$repo_name"
        pending_count=$((pending_count + 1))
    done

    if [[ "$pending_count" -eq 0 ]]; then
        if [[ "$indexed_count" -gt 0 ]]; then
            echo "All discovered repos already have chunks"
        else
            echo "No repos discovered"
        fi
    fi

    exit 0
fi

exec "$PYTHON_BIN" scripts/run_index.py "$@"
