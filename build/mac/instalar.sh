#!/bin/bash
# Installs the GUARDIANA daemon on a Mac (runs as root, called by lanzador.sh through
# the macOS administrator prompt). Internal tool for the development Mac (DECISIONES #55).
#   instalar.sh <path to the guardiana binary> <console user> [es|en|pt]
# The language is the one of the launcher's dialogs, which show this script's last line.
# Does: copies the binary to /usr/local/guardiana, creates the data folder owned by the
# console user (so the launcher can read the panel token), records the DNS servers in
# use per network service (a safety copy for desinstalar.sh) and starts a LaunchDaemon
# on 127.0.0.1:53 forwarding to those servers. It never touches the Mac's DNS (brief
# §11): the panel offers the change with its button, as on Windows, and undoes it.
set -e
BIN_SRC="$1"; USER_NAME="$2"; L="${3:-es}"
m() {  # m <es> <en> <pt>: the text in the launcher's language
  case "$L" in en) echo "$2" ;; pt) echo "$3" ;; *) echo "$1" ;; esac
}
BIN_DIR="/usr/local/guardiana"
DATA="/Library/Application Support/Guardiana"
PLIST="/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"
LABEL="com.guardianagroup.guardiana"
[ -f "$BIN_SRC" ] || { m "No encuentro el programa en $BIN_SRC." "I can't find the program at $BIN_SRC." "Não encontro o programa em $BIN_SRC."; exit 1; }
[ -n "$USER_NAME" ] || { m "Falta el usuario." "The user is missing." "Falta o usuário."; exit 1; }

# 1. The Mac must have a DNS upstream today, or installing would leave it without names.
#    It is only CHECKED here, not written into the daemon: until 1 Oct 2026 the servers found
#    at install time were passed as --upstream and frozen for ever, so a Mac installed at home
#    and opened elsewhere forwarded every name to the home router, which was not there, and the
#    owner's own Mac lost its internet. Now the daemon runs `service run`: it takes the servers
#    the panel saved when it changed the DNS and follows the DHCP lease when the network changes.
UPSTREAMS=""
if [ -s "$DATA/dns-anterior.txt" ]; then
  UPSTREAMS="$(awk -F'\t' '$2 != "empty" {print $2}' "$DATA/dns-anterior.txt" | tr ' ' '\n' | grep -v '^127\.' | sort -u | tr '\n' ' ')"
fi
if [ -z "$UPSTREAMS" ]; then
  UPSTREAMS="$(scutil --dns | awk '/nameserver\[/{print $3}' | grep -v '^127\.' | grep -v ':' | sort -u | tr '\n' ' ')"
fi
# Since 1.0.2 a Mac pointed at Guardiana has only 127.0.0.1 in scutil (no reserve behind it, so
# Chrome and Edge cannot go around it): the network's servers are in the DHCP lease, which is
# also where the daemon reads them.
if [ -z "$UPSTREAMS" ]; then
  UPSTREAMS="$(networksetup -listallhardwareports | awk '/^Device:/{print $2}' | while read -r dev; do
    ipconfig getpacket "$dev" 2>/dev/null | awk -F': ' '/^domain_name_server/{print $2}' | tr -d '{} ' | tr ',' '\n'
  done | grep -v '^127\.' | grep -v '^$' | sort -u | tr '\n' ' ')"
fi
# An update over an installed daemon goes on even offline: it changes no DNS, and the daemon
# follows the network when there is one. Only a first install insists on a DNS today.
if [ -z "$UPSTREAMS" ] && [ ! -f "$PLIST" ]; then
  m "No encuentro el DNS actual del Mac (scutil --dns); no cambio nada." "I can't find the Mac's current DNS (scutil --dns); nothing was changed." "Não encontro o DNS atual do Mac (scutil --dns); nada foi alterado."
  exit 1
fi

# 2. Binary and data folder.
# rm before cp: copying over the program that is running rewrites the same file in place, and
# macOS kills a signed program whose pages no longer match its signature ("Code Signature
# Invalid"). A new file gets a new inode; the running daemon keeps the old one until it restarts
# below (review of 5 Oct 2026, serious 6).
mkdir -p "$BIN_DIR"; rm -f "$BIN_DIR/guardiana"; cp "$BIN_SRC" "$BIN_DIR/guardiana"; chown root:wheel "$BIN_DIR/guardiana"; chmod 755 "$BIN_DIR/guardiana"
# The folder belongs to the account that installed. An update pressed from another account
# leaves it there: taking it over silently left the first account with "reinstall" as the only
# advice; the launcher offers the handover instead (review of 5 Oct 2026, second pass).
if [ ! -d "$DATA" ]; then mkdir -p "$DATA"; chown "$USER_NAME" "$DATA"; fi
chmod 750 "$DATA"
OWNER="$(stat -f %Su "$DATA" 2>/dev/null || true)"; [ -n "$OWNER" ] || OWNER="$USER_NAME"
if ! grep -qE '^[0-9a-f]{64}$' "$DATA/panel.token" 2>/dev/null; then
  sudo -u "$OWNER" sh -c "umask 077; openssl rand -hex 32 > '$DATA/panel.token'"
fi

# 3. Record the DNS per enabled service before touching it (once; a reinstall keeps the original).
if [ ! -s "$DATA/dns-anterior.txt" ]; then
  networksetup -listallnetworkservices | tail -n +2 | grep -v '^\*' | while IFS= read -r svc; do
    cur="$(networksetup -getdnsservers "$svc" 2>/dev/null | tr '\n' ' ' | sed 's/ *$//')"
    case "$cur" in *"aren't any"*|"") cur="empty" ;; esac
    printf '%s\t%s\n' "$svc" "$cur"
  done > "$DATA/dns-anterior.txt"
  chown "$OWNER" "$DATA/dns-anterior.txt"
fi

# 4. The daemon: root, port 53 on loopback, panel on 7443, restarted by launchd if it dies.
#    `service run` is the quiet service body (the same one systemd runs): no line per query, so
#    the log stops growing by the megabyte, and the upstream follows the network (see step 1).
#    The old log is started afresh: on the owner's Mac it had reached 29 MB of every name asked.
#    No GUARDIANA_DATA in the environment: the data folder is already this one by default on a
#    Mac, and the variable also moves the trial mark INTO the data folder (it exists for tests),
#    so in 1.0.1 deleting that folder gave the seven days back, and the mark was not where the
#    install page says, /etc/guardiana/prueba-empezada. Found by the customer test, 1 Oct 2026.
: > "$DATA/guardiana.log" 2>/dev/null || true
cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>$LABEL</string>
<key>ProgramArguments</key><array><string>$BIN_DIR/guardiana</string><string>service</string><string>run</string></array>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><true/>
<key>StandardOutPath</key><string>$DATA/guardiana.log</string>
<key>StandardErrorPath</key><string>$DATA/guardiana.log</string>
</dict></plist>
PL
chown root:wheel "$PLIST"; chmod 644 "$PLIST"
launchctl bootout system "$PLIST" >/dev/null 2>&1 || true
# bootout can return before the old daemon has gone, and bootstrap then fails (error 5 or 36)
# and ends this script with the daemon unloaded: wait for it, as the uninstaller does.
n=0; while pgrep -qf "$BIN_DIR/guardiana service" && [ "$n" -lt 20 ]; do sleep 0.5; n=$((n + 1)); done
launchctl bootstrap system "$PLIST"

# 5. Wait until it answers on port 53 and on the panel.
ok=0
for i in $(seq 1 20); do
  if dig +short +time=2 +tries=1 @127.0.0.1 example.com 2>/dev/null | grep -q . && [ "$(curl -s -m 2 -o /dev/null -w '%{http_code}' http://127.0.0.1:7443/)" = "200" ]; then ok=1; break; fi
  sleep 0.5
done
[ "$ok" = 1 ] || { m "El guardián no responde. Mira $DATA/guardiana.log." "The guardian does not answer. Look at $DATA/guardiana.log." "O guardião não responde. Veja $DATA/guardiana.log."; exit 1; }
if [ -n "$UPSTREAMS" ]; then
  m "Reenvía hoy a: $UPSTREAMS (y sigue a la red cuando cambie)" "Forwards today to: $UPSTREAMS (and follows the network when it changes)" "Encaminha hoje para: $UPSTREAMS (e segue a rede quando ela mudar)"
else
  m "Reenvía a los DNS que dé la red en cuanto haya una." "Forwards to the DNS the network gives as soon as there is one." "Encaminha para o DNS que a rede der assim que houver uma."
fi
