#!/bin/bash
# Upload the Ghostbridge Live! knowledge corpus into the master NotebookLM notebook.
# Run this LAPTOP-SIDE (live browser session) or right after a fresh `nlm login` —
# the static-cookie sessions on the server die in minutes, which is why this is a script.
#
# Usage:  ./knowledge/upload-to-notebook.sh
# Prereq: nlm authenticated (nlm notebook list should work first)

set -u
MASTER="${1:-01b00e9c-033a-4bc9-965f-3257360ff4a8}"   # Ghostbridge Live!
cd "$(dirname "$0")/.." || exit 1

# fail fast if auth is dead
if ! nlm notebook list >/dev/null 2>&1; then
  echo "ERROR: nlm not authenticated. Run 'nlm login' (laptop) or re-import cookies, then retry."
  exit 1
fi

n=0
add() {  # add <file> <title>
  if nlm source add "$MASTER" --file "$1" --title "$2" >/dev/null 2>&1; then
    n=$((n+1)); echo "+[$n] $2"
  else
    echo "FAIL $2 (auth may have expired — re-login and re-run; dups are fine)"
  fi
}

echo "=== distilled knowledge ==="
add WISHLIST.md                       "Ghostbridge Live! — Task Board"
add SIGNALS.md                        "Ghostbridge Live! — Model Signals & Decisions"
add knowledge/notebooks.manifest.json "Knowledge Corpus Manifest"

echo "=== core memory ==="
for m in "$HOME"/.claude/projects/-home-jeremy-git-operation-dbus-proto/memory/*.md; do
  add "$m" "memory-$(basename "$m" .md)"
done

echo "=== full transcripts (the reasoning) ==="
for f in knowledge/transcripts/*.md; do
  add "$f" "transcript-$(basename "$f" .md)"
done

echo "=== done: $n sources uploaded to $MASTER ==="
echo "If it died partway, just re-run — NotebookLM dedups by you, dups are harmless."
