#!/bin/bash
# GUARDIANA SECURE · relé: el servicio de llamadas (TURN, 17 oct 2026).
#
# Las llamadas pasan a WebRTC (UDP) y van siempre por este servidor: ninguno de los dos lados
# ve nunca la dirección del otro. Esto instala coturn, crea una sola vez el secreto que
# comparte con el relé (/etc/guardiana-rele/turn.secret), escribe /etc/turnserver.conf sin
# ningún registro de direcciones, abre los puertos de las llamadas y reinicia el relé para que
# lea el secreto. El relé solo da credenciales (válidas 12 horas) para una sala que existe en
# ese momento; coturn las comprueba con el secreto y no guarda lista de usuarios. Lo que pasa
# por él va cifrado de extremo a extremo (DTLS-SRTP): tampoco puede leerlo.
#
# Se ejecuta como root en el servidor, después de actualizar.sh. Se puede repetir sin peligro
# (no cambia el secreto si ya existe):
#
#   bash /opt/guardiana-rele/guardiana/crates/rele/turn.sh
#
# El de América pasa su nombre: bash turn.sh rele-am.guardianagroup.com
set -euo pipefail
NOMBRE="${1:-rele.guardianagroup.com}"
GRUPO="$(id -gn rele)"
SECRETO_ARCHIVO=/etc/guardiana-rele/turn.secret

# Espera hasta 5 min si otro apt (p. ej. el del certificado) tiene el candado.
apt-get -o DPkg::Lock::Timeout=300 install -y -q coturn openssl >/dev/null

mkdir -p /etc/guardiana-rele
if [ ! -s "$SECRETO_ARCHIVO" ]; then
  (umask 077 && openssl rand -hex 32 > "$SECRETO_ARCHIVO")
fi
chown root:"$GRUPO" "$SECRETO_ARCHIVO"
chmod 0640 "$SECRETO_ARCHIVO"
SECRETO="$(tr -d '[:space:]' < "$SECRETO_ARCHIVO")"

CONF=/etc/turnserver.conf
(umask 077 && cat > "$CONF" <<CONF
# GUARDIANA SECURE · TURN para las llamadas (lo escribe turn.sh; no editar a mano).
listening-port=3478
fingerprint
use-auth-secret
static-auth-secret=$SECRETO
realm=$NOMBRE
server-name=$NOMBRE
min-port=49160
max-port=49400
no-tls
no-dtls
no-cli
no-multicast-peers
no-loopback-peers
# Nunca hacia redes privadas o reservadas: solo entre dos clientes de este mismo servidor.
denied-peer-ip=0.0.0.0-0.255.255.255
denied-peer-ip=10.0.0.0-10.255.255.255
denied-peer-ip=100.64.0.0-100.127.255.255
denied-peer-ip=127.0.0.0-127.255.255.255
denied-peer-ip=169.254.0.0-169.254.255.255
denied-peer-ip=172.16.0.0-172.31.255.255
denied-peer-ip=192.0.0.0-192.0.0.255
denied-peer-ip=192.0.2.0-192.0.2.255
denied-peer-ip=192.88.99.0-192.88.99.255
denied-peer-ip=192.168.0.0-192.168.255.255
denied-peer-ip=198.18.0.0-198.19.255.255
denied-peer-ip=198.51.100.0-198.51.100.255
denied-peer-ip=203.0.113.0-203.0.113.255
denied-peer-ip=224.0.0.0-239.255.255.255
denied-peer-ip=240.0.0.0-255.255.255.255
denied-peer-ip=::1
denied-peer-ip=::ffff:0.0.0.0-::ffff:255.255.255.255
denied-peer-ip=64:ff9b::-64:ff9b::ffff:ffff
denied-peer-ip=100::-100::ffff:ffff:ffff:ffff
denied-peer-ip=2001:db8::-2001:db8:ffff:ffff:ffff:ffff:ffff:ffff
denied-peer-ip=fc00::-fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff
denied-peer-ip=fe80::-febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff
total-quota=200
user-quota=12
max-bps=3000000
stale-nonce=600
# Ningún registro: ni direcciones ni nada.
no-stdout-log
log-file=/dev/null
simple-log
CONF
)
# coturn corre como su propio usuario: que pueda leer su configuración (lleva el secreto).
if getent group turnserver >/dev/null; then
  chown root:turnserver "$CONF"
  chmod 0640 "$CONF"
else
  chown root:root "$CONF"
  chmod 0600 "$CONF"
fi

if [ -f /etc/default/coturn ]; then
  if grep -qE '^#?[[:space:]]*TURNSERVER_ENABLED=' /etc/default/coturn; then
    sed -i -E 's/^#?[[:space:]]*TURNSERVER_ENABLED=.*/TURNSERVER_ENABLED=1/' /etc/default/coturn
  else
    echo 'TURNSERVER_ENABLED=1' >> /etc/default/coturn
  fi
fi

ufw allow 3478/udp >/dev/null
ufw allow 3478/tcp >/dev/null
ufw allow 49160:49400/udp >/dev/null

systemctl enable --now coturn
systemctl restart coturn
# El relé lee el secreto al arrancar.
systemctl restart guardiana-rele
sleep 2
systemctl --no-pager status guardiana-rele | head -5
echo
echo "TURN listo en $NOMBRE. Compruébalo con: systemctl status coturn (debe decir «active (running)»)."
