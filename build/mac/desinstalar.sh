#!/bin/bash
# Removes the GUARDIANA daemon from a Mac and restores the DNS recorded by instalar.sh.
# Runs as root (called by lanzador.sh through the macOS administrator prompt). The data
# folder (ledger, token) stays; it is the person's record.
DATA="/Library/Application Support/Guardiana"
PLIST="/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"
# First the program's own undo (exact backup taken by the panel when it changed the DNS);
# then the safety copy from instalar.sh for anything left pointing at 127.0.0.1.
if [ -x /usr/local/guardiana/guardiana ]; then GUARDIANA_DATA="$DATA" /usr/local/guardiana/guardiana dns --restore >/dev/null 2>&1 || true; fi
if [ -s "$DATA/dns-anterior.txt" ]; then
  while IFS=$'\t' read -r svc servers; do
    [ -n "$svc" ] || continue
    if [ "$servers" = "empty" ]; then networksetup -setdnsservers "$svc" empty >/dev/null 2>&1; else networksetup -setdnsservers "$svc" $servers >/dev/null 2>&1; fi
  done < "$DATA/dns-anterior.txt"
else
  networksetup -listallnetworkservices | tail -n +2 | grep -v '^\*' | while IFS= read -r svc; do networksetup -setdnsservers "$svc" empty >/dev/null 2>&1; done
fi
dscacheutil -flushcache 2>/dev/null || true; killall -HUP mDNSResponder 2>/dev/null || true
launchctl bootout system "$PLIST" >/dev/null 2>&1 || true
rm -f "$PLIST"
rm -rf /usr/local/guardiana
mv "$DATA/dns-anterior.txt" "$DATA/dns-anterior.restaurado.txt" 2>/dev/null || true
echo "El extracto sigue en $DATA."
