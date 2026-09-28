#!/bin/sh
# Guardiana tarball uninstaller: stops the service first (its watchdog would
# re-apply the DNS), restores the DNS exactly as it was, closes Home Mode and
# removes the binary. The extract stays unless --purge is given.
# It speaks the terminal's language (LANG): Spanish, Portuguese, or English for any other.
set -e
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in
    es* | "" | C | C.* | POSIX) L=es ;;
    pt*) L=pt ;;
    *) L=en ;;
esac
m() {  # m <es> <en> <pt>
    case "$L" in en) echo "$2" ;; pt) echo "$3" ;; *) echo "$1" ;; esac
}
if [ "$(id -u)" -ne 0 ]; then
    m "Ejecuta con sudo: sudo ./desinstalar.sh [--purge]" "Run it with sudo: sudo ./desinstalar.sh [--purge]" "Execute com sudo: sudo ./desinstalar.sh [--purge]"
    exit 1
fi
bin=/usr/local/bin/guardiana
if [ -x "$bin" ]; then
    "$bin" service stop >/dev/null 2>&1 || true
    "$bin" hogar off --desinstalando >/dev/null 2>&1 || true
    "$bin" dns --restore || true
    "$bin" service uninstall || true
    rm -f "$bin"
fi
rm -rf /usr/local/share/doc/guardiana
if [ "${1:-}" = "--purge" ]; then
    rm -rf /var/lib/guardiana
    m "Extracto borrado." "Ledger deleted." "Extrato apagado."
else
    m "El extracto sigue en /var/lib/guardiana (bórralo con --purge)." \
      "The ledger is still in /var/lib/guardiana (delete it with --purge)." \
      "O extrato continua em /var/lib/guardiana (apague com --purge)."
fi
m "Guardiana desinstalada. El DNS del sistema está como antes." \
  "Guardiana uninstalled. The system DNS is as it was before." \
  "Guardiana desinstalada. O DNS do sistema está como antes."
