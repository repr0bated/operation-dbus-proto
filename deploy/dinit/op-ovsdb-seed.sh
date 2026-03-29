#!/bin/sh
set -eu

BRIDGE="ovsbr0"
UPLINK="ens3"
SOCKET_PATH="/var/run/openvswitch/db.sock"
if [ -S /run/openvswitch/db.sock ]; then
  SOCKET_PATH="/run/openvswitch/db.sock"
fi
SERVER="unix:$SOCKET_PATH"

wait_for_socket() {
  i=0
  while [ "$i" -lt 60 ]; do
    if [ -S "$SOCKET_PATH" ]; then
      return 0
    fi
    i=$((i + 1))
    sleep 1
  done
  return 1
}

json_compact() {
  tr -d '[:space:]'
}

query_json() {
  ovsdb-client transact "$SERVER" "$1"
}

select_rows() {
  table="$1"
  column="$2"
  value="$3"
  query_json "[\"Open_vSwitch\",{\"op\":\"select\",\"table\":\"$table\",\"where\":[[\"$column\",\"==\",\"$value\"]]}]"
}

rows_empty() {
  printf '%s' "$1" | json_compact | grep -F '"rows":[]' >/dev/null 2>&1
}

if ! wait_for_socket; then
  echo "op-ovsdb-seed: OVSDB socket $SOCKET_PATH unavailable after timeout" >&2
  exit 1
fi

bridge_rows="$(select_rows Bridge name "$BRIDGE")"
if ! rows_empty "$bridge_rows"; then
  echo "op-ovsdb-seed: bridge $BRIDGE already present in OVSDB"
  exit 0
fi

for spec in \
  "Port name $UPLINK" \
  "Port name $BRIDGE" \
  "Interface name $UPLINK" \
  "Interface name $BRIDGE"
do
  set -- $spec
  existing_rows="$(select_rows "$1" "$2" "$3")"
  if ! rows_empty "$existing_rows"; then
    echo "op-ovsdb-seed: found pre-existing $1 row for $3 without bridge $BRIDGE; refusing automatic seed" >&2
    exit 1
  fi
done

if [ ! -r "/sys/class/net/$UPLINK/address" ]; then
  echo "op-ovsdb-seed: uplink MAC for $UPLINK is unavailable" >&2
  exit 1
fi
UPLINK_MAC="$(tr 'A-Z' 'a-z' < "/sys/class/net/$UPLINK/address")"

transaction=$(cat <<EOF
["Open_vSwitch",
 {"op":"insert","table":"Interface","row":{"name":"$UPLINK"},"uuid-name":"uplink_iface"},
 {"op":"insert","table":"Port","row":{"name":"$UPLINK","interfaces":["set",[["named-uuid","uplink_iface"]]]},"uuid-name":"uplink_port"},
 {"op":"insert","table":"Interface","row":{"name":"$BRIDGE","type":"internal"},"uuid-name":"bridge_iface"},
 {"op":"insert","table":"Port","row":{"name":"$BRIDGE","interfaces":["set",[["named-uuid","bridge_iface"]]]},"uuid-name":"bridge_port"},
 {"op":"insert","table":"Bridge","row":{"name":"$BRIDGE","fail_mode":"standalone","other_config":["map",[["hwaddr","$UPLINK_MAC"]]],"ports":["set",[["named-uuid","uplink_port"],["named-uuid","bridge_port"]]]},"uuid-name":"bridge"},
 {"op":"mutate","table":"Open_vSwitch","where":[],"mutations":[["bridges","insert",["set",[["named-uuid","bridge"]]]]]}
]
EOF
)

query_json "$transaction" >/dev/null

bridge_rows="$(select_rows Bridge name "$BRIDGE")"
if rows_empty "$bridge_rows"; then
  echo "op-ovsdb-seed: bridge $BRIDGE still missing after seed transaction" >&2
  exit 1
fi

echo "op-ovsdb-seed: seeded bridge $BRIDGE with uplink $UPLINK into OVSDB"
