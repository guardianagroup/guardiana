#!/bin/bash
# Installs the GUARDIANA daemon on a Mac (runs as root, called by lanzador.sh through
# the macOS administrator prompt). Internal tool for the development Mac (DECISIONES #55).
#   instalar.sh <path to the guardiana binary> <console user>
# Does: copies the binary to /usr/local/guardiana, creates the data folder owned by the
# console user (so the launcher can read the panel token), records the DNS servers in
# use per network service (a safety copy for desinstalar.sh) and starts a LaunchDaemon
# on 127.0.0.1:53 forwarding to those servers. It never touches the Mac's DNS (brief
# §11): the panel offers the change with its button, as on Windows, and undoes it.
set -e
BIN_SRC="$1"; USER_NAME="$2"
BIN_DIR="/usr/local/guardiana"
DATA="/Library/Application Support/Guardiana"
PLIST="/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"
LABEL="com.guardianagroup.guardiana"
[ -f "$BIN_SRC" ] || { echo "no encuentro el binario $BIN_SRC"; exit 1; }
[ -n "$USER_NAME" ] || { echo "falta el usuario"; exit 1; }

# 1. The DNS servers the Mac uses today become the upstream. Refuse to continue
#    without one: switching the DNS to a resolver with no upstream leaves the Mac offline.
UPSTREAMS=""
if [ -s "$DATA/dns-anterior.txt" ]; then
  UPSTREAMS="$(awk -F'\t' '$2 != "empty" {print $2}' "$DATA/dns-anterior.txt" | tr ' ' '\n' | grep -v '^127\.' | sort -u | tr '\n' ' ')"
fi
if [ -z "$UPSTREAMS" ]; then
  UPSTREAMS="$(scutil --dns | awk '/nameserver\[/{print $3}' | grep -v '^127\.' | grep -v ':' | sort -u | tr '\n' ' ')"
fi
[ -n "$UPSTREAMS" ] || { echo "no encuentro el DNS actual del Mac (scutil --dns); no cambio nada"; exit 1; }

# 2. Binary and data folder.
mkdir -p "$BIN_DIR"; cp "$BIN_SRC" "$BIN_DIR/guardiana"; chown root:wheel "$BIN_DIR/guardiana"; chmod 755 "$BIN_DIR/guardiana"
mkdir -p "$DATA"; chown "$USER_NAME" "$DATA"; chmod 750 "$DATA"
if ! grep -qE '^[0-9a-f]{64}$' "$DATA/panel.token" 2>/dev/null; then
  sudo -u "$USER_NAME" sh -c "umask 077; openssl rand -hex 32 > '$DATA/panel.token'"
fi

# 3. Record the DNS per enabled service before touching it (once; a reinstall keeps the original).
if [ ! -s "$DATA/dns-anterior.txt" ]; then
  networksetup -listallnetworkservices | tail -n +2 | grep -v '^\*' | while IFS= read -r svc; do
    cur="$(networksetup -getdnsservers "$svc" 2>/dev/null | tr '\n' ' ' | sed 's/ *$//')"
    case "$cur" in *"aren't any"*|"") cur="empty" ;; esac
    printf '%s\t%s\n' "$svc" "$cur"
  done > "$DATA/dns-anterior.txt"
  chown "$USER_NAME" "$DATA/dns-anterior.txt"
fi

# 4. The daemon: root, port 53 on loopback, panel on 7443, restarted by launchd if it dies.
ARGS=""
for u in $UPSTREAMS; do ARGS="$ARGS<string>--upstream</string><string>$u</string>"; done
cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>$LABEL</string>
<key>ProgramArguments</key><array><string>$BIN_DIR/guardiana</string><string>observe</string><string>--listen</string><string>127.0.0.1:53</string>$ARGS<string>--panel-listen</string><string>127.0.0.1:7443</string></array>
<key>EnvironmentVariables</key><dict><key>GUARDIANA_DATA</key><string>$DATA</string></dict>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><true/>
<key>StandardOutPath</key><string>$DATA/guardiana.log</string>
<key>StandardErrorPath</key><string>$DATA/guardiana.log</string>
</dict></plist>
PL
chown root:wheel "$PLIST"; chmod 644 "$PLIST"
launchctl bootout system "$PLIST" >/dev/null 2>&1 || true
launchctl bootstrap system "$PLIST"

# 5. Wait until it answers on port 53 and on the panel.
ok=0
for i in $(seq 1 20); do
  if dig +short +time=2 +tries=1 @127.0.0.1 example.com 2>/dev/null | grep -q . && [ "$(curl -s -m 2 -o /dev/null -w '%{http_code}' http://127.0.0.1:7443/)" = "200" ]; then ok=1; break; fi
  sleep 0.5
done
[ "$ok" = 1 ] || { echo "el daemon no responde. Mira $DATA/guardiana.log"; exit 1; }
echo "Reenvía a: $UPSTREAMS"
