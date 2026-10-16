#!/bin/bash
# GUARDIANA SECURE · relé: el certificado para los móviles (16 oct 2026, iPhone y iPad).
#
# Un móvil solo abre wss:// hacia un nombre con certificado de una autoridad pública. Esto pide
# uno a Let's Encrypt para rele.guardianagroup.com (certbot, que lo renueva solo), lo copia
# donde el relé puede leerlo y reinicia el relé. Se ejecuta una vez, como root, en el servidor
# (consola de Hetzner), después de actualizar.sh:
#
#   bash /opt/guardiana-rele/guardiana/crates/rele/servidor-tls.sh
#
# El puerto 80 queda abierto solo para que Let's Encrypt compruebe el nombre al renovar; nadie
# escucha en él el resto del tiempo. No se guarda nada de nadie.
set -euo pipefail
NOMBRE=rele.guardianagroup.com
GRUPO="$(id -gn rele)"
apt-get install -y -q certbot >/dev/null
ufw allow 80/tcp >/dev/null
mkdir -p /etc/guardiana-rele/tls /etc/letsencrypt/renewal-hooks/deploy
GANCHO=/etc/letsencrypt/renewal-hooks/deploy/guardiana-rele.sh
cat > "$GANCHO" <<GANCHO
#!/bin/bash
set -e
install -m 0644 -o root -g $GRUPO /etc/letsencrypt/live/$NOMBRE/fullchain.pem /etc/guardiana-rele/tls/fullchain.pem
install -m 0640 -o root -g $GRUPO /etc/letsencrypt/live/$NOMBRE/privkey.pem /etc/guardiana-rele/tls/privkey.pem
systemctl restart guardiana-rele
GANCHO
chmod 0755 "$GANCHO"
certbot certonly --standalone -d "$NOMBRE" --non-interactive --agree-tos \
  --register-unsafely-without-email --keep-until-expiring
bash "$GANCHO"
sleep 2
systemctl --no-pager status guardiana-rele | head -5
echo
echo "Comprueba en el navegador: https://$NOMBRE/ debe decir «GUARDIANA SECURE relay»."
