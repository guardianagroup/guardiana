#!/bin/sh
# Guardiana tarball uninstaller: stops the service first (its watchdog would
# re-apply the DNS), restores the DNS exactly as it was, closes Home Mode and
# removes the binary. The extract stays unless --purge is given.
set -e
if [ "$(id -u)" -ne 0 ]; then
    echo "Ejecuta con sudo: sudo ./desinstalar.sh [--purge]"
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
if [ "$1" = "--purge" ]; then
    rm -rf /var/lib/guardiana
    echo "Extracto borrado."
else
    echo "El extracto sigue en /var/lib/guardiana (bórralo con --purge)."
fi
echo "Guardiana desinstalada. El DNS del sistema está como antes."
