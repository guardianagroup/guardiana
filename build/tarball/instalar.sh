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
# An update keeps whatever the person decided about the DNS: on start the service points the
# machine at the guardian again if it was pointed there before. Only a first install can say
# "the DNS has not changed". An update is told by the program already being here: the data
# folder outlives an uninstall without --purge, and that uninstall gave the DNS back, so a
# reinstall after it is a first install as far as the DNS goes (review of branch 3, 5 Oct 2026).
if [ -x /usr/local/bin/guardiana ]; then primera=0; else primera=1; fi
install -d -m 755 /usr/local/bin
install -m 755 "$here/guardiana" /usr/local/bin/guardiana
install -d -m 755 /usr/local/share/doc/guardiana
for f in LEEME.txt README.txt LEIAME.txt WHAT_IT_DOES_NOT_DO.md VERIFY.md HOGAR.md LISTS.md; do
    [ -f "$here/$f" ] && install -m 644 "$here/$f" /usr/local/share/doc/guardiana/
done
# Writes the unit (the same one the .deb ships) and restarts the service, so an update over a
# running copy picks up the new program and the new unit, not only the next boot.
/usr/local/bin/guardiana service install
echo
# "Installed and running" used to be printed without looking (review of 1 Oct 2026, entry 8):
# a service can be registered and still fall over right after starting (port 53 taken, for
# one). Give it a moment and ask systemd before saying so.
sleep 2
if systemctl is-active --quiet guardiana; then
    m "Guardiana instalada y en marcha. Para abrir el panel: sudo guardiana panel" \
      "Guardiana is installed and running. To open the panel: sudo guardiana panel" \
      "A Guardiana está instalada e funcionando. Para abrir o painel: sudo guardiana panel"
else
    m "Guardiana está instalada, pero el servicio no está en marcha. Mira por qué con: journalctl -u guardiana -n 20" \
      "Guardiana is installed, but the service is not running. See why with: journalctl -u guardiana -n 20" \
      "A Guardiana está instalada, mas o serviço não está em andamento. Veja o motivo com: journalctl -u guardiana -n 20"
fi
if [ "$primera" = 1 ]; then
    m "El DNS del sistema no ha cambiado. Se cambia desde el panel, con tu permiso." \
      "The system DNS has not changed. It is changed from the panel, with your permission." \
      "O DNS do sistema não mudou. Ele é mudado pelo painel, com a sua permissão."
else
    m "Era una actualización: el DNS del sistema queda como lo tenías. Compruébalo con: guardiana verify" \
      "This was an update: the system DNS stays as you had it. Check it with: guardiana verify" \
      "Foi uma atualização: o DNS do sistema fica como você tinha. Confira com: guardiana verify"
fi
