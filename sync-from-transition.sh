#!/bin/bash
# sync-from-transition.sh - Copy missing files from transition to operation-dbus
set -euo pipefail

SRC="/home/jeremy/Videos/transition"
DST="/home/jeremy/Videos/operation-dbus"

# Sync crates - only copy if missing in destination
for crate in "$SRC"/crates/op-*; do
    name=$(basename "$crate")
    if [[ -d "$DST/crates/$name" ]]; then
        # Crate exists, check for missing files
        find "$crate" -type f | while read -r file; do
            rel="${file#$crate/}"
            dst_file="$DST/crates/$name/$rel"
            if [[ ! -f "$dst_file" ]]; then
                mkdir -p "$(dirname "$dst_file")"
                cp "$file" "$dst_file"
                echo "Copied: crates/$name/$rel"
            fi
        done
    else
        # Crate missing entirely, copy it
        cp -r "$crate" "$DST/crates/"
        echo "Copied crate: $name"
    fi
done

# Sync src directory
if [[ -d "$SRC/src" ]]; then
    find "$SRC/src" -type f | while read -r file; do
        rel="${file#$SRC/}"
        dst_file="$DST/$rel"
        if [[ ! -f "$dst_file" ]]; then
            mkdir -p "$(dirname "$dst_file")"
            cp "$file" "$dst_file"
            echo "Copied: $rel"
        fi
    done
fi

echo "Done."
