#!/bin/bash
# GUARDIANA.app, the Mac version that ships with 1.0. Double-click: opens the
# panel. First time: offers to install the daemon; macOS itself asks for the
# administrator password (we never see it).
# From the same icon the daemon can be removed and the previous DNS restored.
RES="$(cd "$(dirname "$0")/../Resources" && pwd)"
DATA="/Library/Application Support/Guardiana"
PLIST="/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"
ICON="$RES/guardiana.icns"

say() { osascript -e "display dialog \"$1\" buttons {\"Entendido\"} default button 1 with title \"GUARDIANA\" with icon POSIX file \"$ICON\"" >/dev/null 2>&1; }
ask() { osascript -e "button returned of (display dialog \"$1\" buttons {$2} default button \"$3\" with title \"GUARDIANA\" with icon POSIX file \"$ICON\")" 2>/dev/null; }
as_admin() { osascript -e "do shell script \"$1\" with administrator privileges" 2>&1; }
panel_up() { [ "$(curl -s -m 2 -o /dev/null -w '%{http_code}' http://127.0.0.1:7443/)" = "200" ]; }
open_panel() {
  local token
  token="$(tr -d '[:space:]' < "$DATA/panel.token" 2>/dev/null)"
  if [ -z "$token" ]; then say "No encuentro la clave del panel en $DATA. Quita GUARDIANA desde este icono y vuelve a instalarlo."; return 1; fi
  open "http://127.0.0.1:7443/?t=$token"
}
wait_panel() { local i; for i in $(seq 1 20); do panel_up && return 0; sleep 0.5; done; return 1; }

if [ -f "$PLIST" ]; then
  # A newer build inside the app than the one installed: offer the update (root copies it and restarts the daemon).
  if ! cmp -s "$RES/guardiana" /usr/local/guardiana/guardiana; then
    up="$(ask "Esta copia de GUARDIANA es más nueva que la instalada en el Mac. ¿La actualizo? El Mac pedirá tu contraseña; el DNS no cambia y el extracto se conserva." '"Ahora no", "Actualizar"' "Actualizar")"
    if [ "$up" = "Actualizar" ]; then
      as_admin "cp '$RES/guardiana' /usr/local/guardiana/guardiana && chown root:wheel /usr/local/guardiana/guardiana && chmod 755 /usr/local/guardiana/guardiana && launchctl kickstart -k system/com.guardianagroup.guardiana" >/dev/null
      if wait_panel; then open_panel; say "GUARDIANA se actualizó y ya está de nuevo en marcha."; else say "Se copió la versión nueva pero el panel no responde. Mira $DATA/guardiana.log."; fi
      exit 0
    fi
  fi
  if panel_up; then
    choice="$(ask "GUARDIANA está en marcha en este Mac.\\n\\nEl extracto se guarda en $DATA." '"Quitar de este Mac", "Abrir el panel"' "Abrir el panel")"
  else
    choice="$(ask "GUARDIANA está instalado pero el panel no responde. Puedo arrancarlo de nuevo (el Mac pedirá tu contraseña) o quitarlo." '"Quitar de este Mac", "Arrancar de nuevo"' "Arrancar de nuevo")"
    if [ "$choice" = "Arrancar de nuevo" ]; then
      as_admin "launchctl bootout system '$PLIST' >/dev/null 2>&1; launchctl bootstrap system '$PLIST'" >/dev/null
      if wait_panel; then open_panel; else say "Sigue sin responder. Mira $DATA/guardiana.log."; fi
      exit 0
    fi
  fi
  case "$choice" in
    "Abrir el panel") open_panel ;;
    "Quitar de este Mac")
      sure="$(ask "Se apaga GUARDIANA, el DNS del Mac vuelve a ser el de antes y el extracto se conserva en $DATA. El Mac pedirá tu contraseña." '"Cancelar", "Quitar"' "Quitar")"
      if [ "$sure" = "Quitar" ]; then
        out="$(as_admin "'$RES/desinstalar.sh'")"
        say "GUARDIANA se quitó de este Mac y el DNS anterior está de vuelta.\\n\\n$out"
      fi ;;
  esac
  exit 0
fi

choice="$(ask "GUARDIANA no está instalado en este Mac.\\n\\nSi lo instalo, el Mac pedirá tu contraseña. Se instala un guardián que escucha en 127.0.0.1 y reenvía al DNS que ya usabas; el DNS del Mac no cambia hasta que pulses el botón del panel, y ahí mismo se deshace. Primero observa; no corta nada sin tu decisión. Se quita desde este mismo icono." '"Cancelar", "Instalar"' "Instalar")"
[ "$choice" = "Instalar" ] || exit 0
out="$(as_admin "'$RES/instalar.sh' '$RES/guardiana' '$USER'")"
if [ -f "$PLIST" ] && wait_panel; then
  open_panel
  say "Listo. Se abre el panel en el navegador: pulsa ahí el botón para que este Mac pase por GUARDIANA. El icono del escritorio vuelve a abrir el panel cuando quieras.\\n\\n$out"
else
  say "No se pudo instalar. Detalle:\\n\\n$out"
fi
