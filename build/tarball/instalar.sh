#!/bin/sh
# Guardiana tarball installer (brief §11): copies the binary, registers the
# systemd service and starts it. It does NOT touch the system DNS; that is
# done from the panel with consent. Readable on purpose: read it before running.
set -e
if [ "$(id -u)" -ne 0 ]; then
    echo "Ejecuta con sudo: sudo ./instalar.sh"
    exit 1
fi
here=$(cd "$(dirname "$0")" && pwd)
install -d -m 755 /usr/local/bin
install -m 755 "$here/guardiana" /usr/local/bin/guardiana
install -d -m 755 /usr/local/share/doc/guardiana
for f in LEEME.txt WHAT_IT_DOES_NOT_DO.md VERIFY.md HOGAR.md LISTS.md; do
    [ -f "$here/$f" ] && install -m 644 "$here/$f" /usr/local/share/doc/guardiana/
done
/usr/local/bin/guardiana service install
echo
echo "Guardiana instalada y en marcha. Abre el panel con: guardiana panel"
echo "El DNS del sistema no ha cambiado. Se cambia desde el panel, con tu permiso."
