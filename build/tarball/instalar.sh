#!/bin/sh
# Guardiana tarball installer (brief §11): copies the binary, registers the
# systemd service and starts it. It does NOT touch the system DNS; that is
# done from the panel with consent. Readable on purpose: read it before running.
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
    m "Ejecuta con sudo: sudo ./instalar.sh" "Run it with sudo: sudo ./instalar.sh" "Execute com sudo: sudo ./instalar.sh"
    exit 1
fi
here=$(cd "$(dirname "$0")" && pwd)
install -d -m 755 /usr/local/bin
install -m 755 "$here/guardiana" /usr/local/bin/guardiana
install -d -m 755 /usr/local/share/doc/guardiana
for f in LEEME.txt README.txt LEIAME.txt WHAT_IT_DOES_NOT_DO.md VERIFY.md HOGAR.md LISTS.md; do
    [ -f "$here/$f" ] && install -m 644 "$here/$f" /usr/local/share/doc/guardiana/
done
/usr/local/bin/guardiana service install
echo
m "Guardiana instalada y en marcha. Abre el panel con: guardiana panel" \
  "Guardiana is installed and running. Open the panel with: guardiana panel" \
  "A Guardiana está instalada e funcionando. Abra o painel com: guardiana panel"
m "El DNS del sistema no ha cambiado. Se cambia desde el panel, con tu permiso." \
  "The system DNS has not changed. It is changed from the panel, with your permission." \
  "O DNS do sistema não mudou. Ele é mudado pelo painel, com a sua permissão."
