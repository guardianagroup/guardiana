#!/bin/bash
# Removes the GUARDIANA daemon from a Mac and puts the DNS back. Runs as root (called by
# lanzador.sh through the macOS administrator prompt). The data folder (ledger, token) stays;
# it is the person's record, as on Windows and Linux.
#
# The order matters and is the same one the .deb and the MSI use:
#   1. stop the daemon FIRST. While it runs, its watchdog re-applies the DNS change
#      (crates/cli/src/engine.rs), so restoring before stopping can be undone in seconds.
#   2. close Home Mode, which also writes the change into the ledger signed as an uninstall.
#   3. the program's own undo, from the exact backup the panel took when it changed the DNS.
#   4. only then the safety net, and only for services still pointing at this machine.
DATA="/Library/Application Support/Guardiana"
PLIST="/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"
BIN=/usr/local/guardiana/guardiana

# 1. The daemon first.
launchctl bootout system "$PLIST" >/dev/null 2>&1 || true
n=0; while pgrep -qf "$BIN" && [ "$n" -lt 20 ]; do sleep 0.5; n=$((n + 1)); done

# 2 and 3. Home Mode off and the program's own undo, both recorded in the ledger.
if [ -x "$BIN" ]; then
  GUARDIANA_DATA="$DATA" "$BIN" hogar off --desinstalando >/dev/null 2>&1 || true
  GUARDIANA_DATA="$DATA" "$BIN" dns --restore >/dev/null 2>&1 || true
fi

# 4. Safety net for anything still pointing at this machine. Only those: a service the person
#    set themselves after installing must not be overwritten with what it held months ago, and
#    a missing record must never mean "wipe everyone's DNS back to automatic".
apunta_aqui() {
  networksetup -getdnsservers "$1" 2>/dev/null | grep -qE '^(127\.|::1$)'
}
if [ -s "$DATA/dns-anterior.txt" ]; then
  while IFS=$'\t' read -r svc servers; do
    [ -n "$svc" ] || continue
    apunta_aqui "$svc" || continue
    if [ "$servers" = "empty" ]; then
      networksetup -setdnsservers "$svc" empty >/dev/null 2>&1
    else
      networksetup -setdnsservers "$svc" $servers >/dev/null 2>&1
    fi
  done < "$DATA/dns-anterior.txt"
else
  # No record of what was there before. Leaving a service pointing at a guardian that no longer
  # exists would leave the Mac without names, so those go back to automatic; the rest are left
  # exactly as the person has them.
  networksetup -listallnetworkservices | tail -n +2 | grep -v '^\*' | while IFS= read -r svc; do
    apunta_aqui "$svc" && networksetup -setdnsservers "$svc" empty >/dev/null 2>&1
  done
fi
dscacheutil -flushcache 2>/dev/null || true
killall -HUP mDNSResponder 2>/dev/null || true

# 5. The files. The plist is already unloaded.
rm -f "$PLIST"
rm -rf /usr/local/guardiana
mv "$DATA/dns-anterior.txt" "$DATA/dns-anterior.restaurado.txt" 2>/dev/null || true
echo "El extracto sigue en $DATA."
