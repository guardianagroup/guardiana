#!/bin/bash
# GUARDIANA.app, the Mac version that ships with 1.0. Double-click: opens the
# panel. First time: offers to install the daemon; macOS itself asks for the
# administrator password (we never see it).
# From the same icon the daemon can be removed and the previous DNS restored.
#
# The dialogs speak the Mac's language (the first of System Settings › General › Language &
# Region): Spanish, Portuguese, or English for any other, like the panel and the site. Until
# 1.0.1 they were Spanish only, so a Mac in English met "Instalar" and "Entendido".
RES="$(cd "$(dirname "$0")/../Resources" && pwd)"
DATA="/Library/Application Support/Guardiana"
PLIST="/Library/LaunchDaemons/com.guardianagroup.guardiana.plist"
ICON="$RES/guardiana.icns"
BIN=/usr/local/guardiana/guardiana

idioma() {
  local l
  l="$(defaults read -g AppleLanguages 2>/dev/null | sed -n 's/^[[:space:]]*"\{0,1\}\([A-Za-z][A-Za-z]\).*/\1/p' | head -n 1 | tr '[:upper:]' '[:lower:]')"
  case "$l" in es | pt) echo "$l" ;; "") echo es ;; *) echo en ;; esac
}
L="$(idioma)"

case "$L" in
en)
  B_OK="OK"; B_NO_AHORA="Not now"; B_ACTUALIZAR="Update"; B_QUITAR_MAC="Remove from this Mac"
  B_ABRIR="Open the panel"; B_ARRANCAR="Start it again"; B_CANCELAR="Cancel"; B_QUITAR="Remove"; B_INSTALAR="Install"
  M_SIN_CLAVE="I can't find the panel key in $DATA. Remove GUARDIANA from this icon and install it again."
  M_ACTUALIZO="This copy of GUARDIANA is newer than the one installed on this Mac. Shall I update it? The Mac will ask for your password; the DNS does not change and the ledger is kept."
  M_ACTUALIZADA="GUARDIANA was updated and is running again."
  M_ACTUALIZADA_SIN_PANEL="The new version was copied but the panel does not answer. Look at $DATA/guardiana.log."
  M_EN_MARCHA="GUARDIANA is running on this Mac.

The ledger is kept in $DATA."
  M_SIN_PANEL="GUARDIANA is installed but the panel does not answer. I can start it again (the Mac will ask for your password) or remove it."
  M_SIGUE_SIN="It still does not answer. Look at $DATA/guardiana.log."
  M_SEGURO="GUARDIANA is switched off, the Mac's DNS goes back to what it was before and the ledger stays in $DATA. The Mac will ask for your password."
  M_QUITADA="GUARDIANA was removed from this Mac and the previous DNS is back."
  M_INSTALO="GUARDIANA is not installed on this Mac.

If I install it, the Mac will ask for your password. It installs a guardian that listens on 127.0.0.1 and forwards to the DNS you already used; the Mac's DNS does not change until you press the button in the panel, and it is undone right there. It observes first; it cuts nothing without your decision. It is removed from this same icon."
  M_LISTO="Done. The panel opens in the browser: press the button there so this Mac goes through GUARDIANA. This same GUARDIANA icon opens the panel again whenever you want."
  M_NO_INSTALADA="It could not be installed. Details:"
  M_INTEL="This version of GUARDIANA needs a Mac with an Apple chip (M1 or later). This Mac has an Intel chip, so it cannot run here. Nothing was installed."
  ;;
pt)
  B_OK="Entendi"; B_NO_AHORA="Agora não"; B_ACTUALIZAR="Atualizar"; B_QUITAR_MAC="Remover deste Mac"
  B_ABRIR="Abrir o painel"; B_ARRANCAR="Iniciar de novo"; B_CANCELAR="Cancelar"; B_QUITAR="Remover"; B_INSTALAR="Instalar"
  M_SIN_CLAVE="Não encontro a chave do painel em $DATA. Remova a GUARDIANA por este ícone e instale de novo."
  M_ACTUALIZO="Esta cópia da GUARDIANA é mais nova que a instalada no Mac. Atualizo? O Mac vai pedir sua senha; o DNS não muda e o extrato é mantido."
  M_ACTUALIZADA="A GUARDIANA foi atualizada e já está funcionando de novo."
  M_ACTUALIZADA_SIN_PANEL="A versão nova foi copiada, mas o painel não responde. Veja $DATA/guardiana.log."
  M_EN_MARCHA="A GUARDIANA está funcionando neste Mac.

O extrato fica guardado em $DATA."
  M_SIN_PANEL="A GUARDIANA está instalada, mas o painel não responde. Posso iniciá-la de novo (o Mac vai pedir sua senha) ou removê-la."
  M_SIGUE_SIN="Continua sem responder. Veja $DATA/guardiana.log."
  M_SEGURO="A GUARDIANA é desligada, o DNS do Mac volta a ser o de antes e o extrato continua em $DATA. O Mac vai pedir sua senha."
  M_QUITADA="A GUARDIANA foi removida deste Mac e o DNS anterior está de volta."
  M_INSTALO="A GUARDIANA não está instalada neste Mac.

Se eu instalar, o Mac vai pedir sua senha. É instalado um guardião que escuta em 127.0.0.1 e encaminha para o DNS que você já usava; o DNS do Mac não muda até você apertar o botão do painel, e ali mesmo se desfaz. Primeiro observa; não corta nada sem a sua decisão. Remove-se por este mesmo ícone."
  M_LISTO="Pronto. O painel abre no navegador: aperte ali o botão para que este Mac passe pela GUARDIANA. Este mesmo ícone da GUARDIANA abre o painel de novo quando você quiser."
  M_NO_INSTALADA="Não foi possível instalar. Detalhe:"
  M_INTEL="Esta versão da GUARDIANA precisa de um Mac com chip Apple (M1 ou posterior). Este Mac tem chip Intel, então ela não funciona aqui. Nada foi instalado."
  ;;
*)
  B_OK="Entendido"; B_NO_AHORA="Ahora no"; B_ACTUALIZAR="Actualizar"; B_QUITAR_MAC="Quitar de este Mac"
  B_ABRIR="Abrir el panel"; B_ARRANCAR="Arrancar de nuevo"; B_CANCELAR="Cancelar"; B_QUITAR="Quitar"; B_INSTALAR="Instalar"
  M_SIN_CLAVE="No encuentro la clave del panel en $DATA. Quita GUARDIANA desde este icono y vuelve a instalarlo."
  M_ACTUALIZO="Esta copia de GUARDIANA es más nueva que la instalada en el Mac. ¿La actualizo? El Mac pedirá tu contraseña; el DNS no cambia y el extracto se conserva."
  M_ACTUALIZADA="GUARDIANA se actualizó y ya está de nuevo en marcha."
  M_ACTUALIZADA_SIN_PANEL="Se copió la versión nueva pero el panel no responde. Mira $DATA/guardiana.log."
  M_EN_MARCHA="GUARDIANA está en marcha en este Mac.

El extracto se guarda en $DATA."
  M_SIN_PANEL="GUARDIANA está instalado pero el panel no responde. Puedo arrancarlo de nuevo (el Mac pedirá tu contraseña) o quitarlo."
  M_SIGUE_SIN="Sigue sin responder. Mira $DATA/guardiana.log."
  M_SEGURO="Se apaga GUARDIANA, el DNS del Mac vuelve a ser el de antes y el extracto se conserva en $DATA. El Mac pedirá tu contraseña."
  M_QUITADA="GUARDIANA se quitó de este Mac y el DNS anterior está de vuelta."
  M_INSTALO="GUARDIANA no está instalado en este Mac.

Si lo instalo, el Mac pedirá tu contraseña. Se instala un guardián que escucha en 127.0.0.1 y reenvía al DNS que ya usabas; el DNS del Mac no cambia hasta que pulses el botón del panel, y ahí mismo se deshace. Primero observa; no corta nada sin tu decisión. Se quita desde este mismo icono."
  M_LISTO="Listo. Se abre el panel en el navegador: pulsa ahí el botón para que este Mac pase por GUARDIANA. Este mismo icono de GUARDIANA vuelve a abrir el panel cuando quieras."
  M_NO_INSTALADA="No se pudo instalar. Detalle:"
  M_INTEL="Esta versión de GUARDIANA necesita un Mac con chip de Apple (M1 o posterior). Este Mac tiene chip Intel, así que aquí no puede funcionar. No se ha instalado nada."
  ;;
esac

# The texts travel to AppleScript as arguments, never pasted into its source: a quote or a
# backslash in a message (or in the output of the install script) cannot break the dialog.
say() {
  osascript -e 'on run argv' \
    -e 'display dialog (item 1 of argv) buttons {item 2 of argv} default button 1 with title "GUARDIANA" with icon (POSIX file (item 3 of argv))' \
    -e 'end run' "$1" "$B_OK" "$ICON" >/dev/null 2>&1
}
# ask <message> <first button> <second button, the default>: prints the button pressed.
ask() {
  osascript -e 'on run argv' \
    -e 'button returned of (display dialog (item 1 of argv) buttons {item 2 of argv, item 3 of argv} default button (item 3 of argv) with title "GUARDIANA" with icon (POSIX file (item 4 of argv)))' \
    -e 'end run' "$1" "$2" "$3" "$ICON" 2>/dev/null
}
# as_admin <shell command>: macOS asks for the administrator password, not us.
as_admin() {
  osascript -e 'on run argv' -e 'do shell script (item 1 of argv) with administrator privileges' -e 'end run' "$1" 2>&1
}
# q <text>: the text between single quotes, safe inside the command given to as_admin.
q() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
panel_up() { [ "$(curl -s -m 2 -o /dev/null -w '%{http_code}' http://127.0.0.1:7443/)" = "200" ]; }
open_panel() {
  local token
  token="$(tr -d '[:space:]' < "$DATA/panel.token" 2>/dev/null)"
  if [ -z "$token" ]; then say "$M_SIN_CLAVE"; return 1; fi
  open "http://127.0.0.1:7443/?t=$token"
}
wait_panel() { local i; for i in $(seq 1 20); do panel_up && return 0; sleep 0.5; done; return 1; }
# The version a guardiana binary reports ("guardiana 1.0.1" → 1.0.1); empty if it can't say.
version_of() { "$1" --version 2>/dev/null | awk 'NR == 1 {print $2}'; }
# newer A B: true when version A is higher than version B (1.0.10 > 1.0.9).
newer() {
  local IFS=. i x y
  local -a a b
  read -r -a a <<< "$1"
  read -r -a b <<< "$2"
  for i in 0 1 2; do
    x="${a[i]:-0}"; y="${b[i]:-0}"
    case "$x$y" in *[!0-9]*) return 1 ;; esac
    [ "$x" -gt "$y" ] && return 0
    [ "$x" -lt "$y" ] && return 1
  done
  return 1
}

# The program inside is built for Apple chips only (the site says so). On an Intel Mac it would
# install and then never answer; say it plainly instead.
if [ "$(sysctl -n hw.optional.arm64 2>/dev/null)" != "1" ]; then
  say "$M_INTEL"
  exit 0
fi

if [ -f "$PLIST" ]; then
  # A newer build inside the app than the one installed: offer the update (root copies it and
  # restarts the daemon). Only newer: opening an old download must not offer to go back.
  if ! cmp -s "$RES/guardiana" "$BIN"; then
    installed="$(version_of "$BIN")"
    if [ -z "$installed" ] || newer "$(version_of "$RES/guardiana")" "$installed"; then
      up="$(ask "$M_ACTUALIZO" "$B_NO_AHORA" "$B_ACTUALIZAR")"
      if [ "$up" = "$B_ACTUALIZAR" ]; then
        as_admin "cp $(q "$RES/guardiana") $BIN && chown root:wheel $BIN && chmod 755 $BIN && launchctl kickstart -k system/com.guardianagroup.guardiana" >/dev/null
        if wait_panel; then open_panel; say "$M_ACTUALIZADA"; else say "$M_ACTUALIZADA_SIN_PANEL"; fi
        exit 0
      fi
    fi
  fi
  if panel_up; then
    choice="$(ask "$M_EN_MARCHA" "$B_QUITAR_MAC" "$B_ABRIR")"
  else
    choice="$(ask "$M_SIN_PANEL" "$B_QUITAR_MAC" "$B_ARRANCAR")"
    if [ "$choice" = "$B_ARRANCAR" ]; then
      as_admin "launchctl bootout system $(q "$PLIST") >/dev/null 2>&1; launchctl bootstrap system $(q "$PLIST")" >/dev/null
      if wait_panel; then open_panel; else say "$M_SIGUE_SIN"; fi
      exit 0
    fi
  fi
  case "$choice" in
    "$B_ABRIR") open_panel ;;
    "$B_QUITAR_MAC")
      sure="$(ask "$M_SEGURO" "$B_CANCELAR" "$B_QUITAR")"
      if [ "$sure" = "$B_QUITAR" ]; then
        out="$(as_admin "$(q "$RES/desinstalar.sh") $L")"
        say "$M_QUITADA

$out"
      fi ;;
  esac
  exit 0
fi

choice="$(ask "$M_INSTALO" "$B_CANCELAR" "$B_INSTALAR")"
[ "$choice" = "$B_INSTALAR" ] || exit 0
out="$(as_admin "$(q "$RES/instalar.sh") $(q "$RES/guardiana") $(q "$USER") $L")"
if [ -f "$PLIST" ] && wait_panel; then
  open_panel
  say "$M_LISTO

$out"
else
  say "$M_NO_INSTALADA

$out"
fi
