#!/bin/bash
# deploy/run_foreground.sh
# Run all services in foreground with local environment

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="$PROJECT_ROOT/deploy/data"
mkdir -p "$DATA_DIR/cache"

# Initialize local D-Bus session if not present
if [ -z "$DBUS_SESSION_BUS_ADDRESS" ]; then
    export $(dbus-daemon --session --fork --print-address | sed 's/^/DBUS_SESSION_BUS_ADDRESS=/')
    echo "Started local D-Bus session: $DBUS_SESSION_BUS_ADDRESS"
fi

# Environment Overrides
export OP_DBUS_DATABASE_URL="sqlite://$DATA_DIR/state.db"
export OP_DBUS_CACHE_DIR="$DATA_DIR/cache"
export OP_DBUS_WEB_PORT=8081
export OP_DBUS_SESSION_BUS=1
export OP_CHAT_LISTEN="0.0.0.0:50052"

echo "Starting Stack..."

# Trap to kill background processes on exit
trap 'kill $(jobs -p)' EXIT

# Run services
"$PROJECT_ROOT/deploy/bin/op-services" > "$PROJECT_ROOT/deploy/services.log" 2>&1 &
"$PROJECT_ROOT/deploy/bin/op-chat" > "$PROJECT_ROOT/deploy/chat.log" 2>&1 &

# Run op-dbus in main foreground
"$PROJECT_ROOT/deploy/bin/op-dbus"
