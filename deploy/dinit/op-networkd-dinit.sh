#!/bin/sh
set -eu

write_resolvconf() {
  tmp_file="$(mktemp /run/resolvconf/resolv.conf.XXXXXX)"
  found_dns=0

  for network_file in /etc/systemd/network/*.network; do
    [ -f "$network_file" ] || continue

    while IFS= read -r line; do
      case "$line" in
        DNS=*)
          printf 'nameserver %s\n' "${line#DNS=}" >> "$tmp_file"
          found_dns=1
          ;;
      esac
    done < "$network_file"
  done

  if [ "$found_dns" -eq 1 ]; then
    mv "$tmp_file" /run/resolvconf/resolv.conf
  else
    rm -f "$tmp_file"
  fi
}

mkdir -p /run/resolvconf
write_resolvconf

exec /usr/lib/systemd/systemd-networkd
