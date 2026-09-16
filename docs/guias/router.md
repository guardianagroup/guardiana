# Guía: poner Guardiana como DNS de toda la casa (router)

Un solo cambio en el router y todos los dispositivos de la Wi‑Fi pasan por Guardiana. La pantalla
exacta cambia con cada modelo; aquí van el camino general y los nombres que usan los routers más
comunes en Colombia y en España. Si tu router no aparece, busca "DNS" dentro de los ajustes de "LAN" o "DHCP".

## Antes de empezar

1. En el panel de Guardiana, página "Modo Hogar", enciéndelo y apunta la **IP del computador**
   (por ejemplo `192.168.1.39`).
2. Esa IP tiene que ser fija. Si Guardiana avisa de que la asigna el router, haz primero la
   **reserva DHCP** (abajo).
3. Decide el DNS secundario. Si el computador se apaga o se duerme, los teléfonos que solo apunten
   a él se quedan sin DNS. Dos opciones, y las dos son honestas:
   - **Solo Guardiana:** todo pasa por Guardiana; si el computador no está, no hay internet en
     los teléfonos hasta que vuelva.
   - **Guardiana + el router como secundario:** los teléfonos nunca se quedan sin internet; a
     cambio, cuando Guardiana no esté, esas consultas no se verán. Algunos teléfonos usan el
     secundario a ratos aunque el primario funcione: esas consultas tampoco se ven.

## El camino general

1. Abre el navegador y entra en la dirección del router: normalmente `192.168.1.1` (a veces
   `192.168.0.1`). Usuario y contraseña: están en la pegatina del router.
2. Busca **LAN**, **Red local** o **DHCP**.
3. Encontrarás "Servidor DNS", "DNS primario / secundario" o "Servidores DNS del DHCP".
4. Primario: la IP del computador. Secundario: vacío, o la IP del propio router.
5. Guarda. Algunos routers reinician.
6. En cada teléfono, apaga y enciende la Wi‑Fi para que coja el nuevo DNS.
7. Comprueba desde un teléfono: `http://comprobar.guardiana.hogar`.

## Reserva DHCP (IP fija para el computador)

En el mismo apartado de LAN/DHCP suele haber "Reserva de direcciones", "DHCP estático" o
"Asignación fija". Se añade la **MAC del computador** (Guardiana la muestra en "Mi dispositivo";
en Windows: `ipconfig /all`, "Dirección física") con la IP que quieras mantener. Guarda y reinicia
la Wi‑Fi del computador.

## Si no puedes o no quieres entrar en el router: dirección fija en el propio computador

La reserva DHCP se puede sustituir fijando la dirección en el ordenador guardián. No hace falta
tocar el router. Solo hay que usar la misma dirección que ya tiene (así no choca con nadie).

- **Windows**, en una ventana de PowerShell como administrador (cambia `Ethernet` por el nombre de
  tu conexión y las direcciones por las tuyas; `192.168.1.1` suele ser el router):

```
netsh interface ipv4 set address name="Ethernet" static 192.168.1.78 255.255.255.0 192.168.1.1
netsh interface ipv4 set dnsservers name="Ethernet" static 192.168.1.1 primary
```

  Deshacer: `netsh interface ipv4 set address name="Ethernet" dhcp` y
  `netsh interface ipv4 set dnsservers name="Ethernet" dhcp`.

- **Linux con NetworkManager** (Ubuntu de escritorio, Mint, Fedora): Ajustes › Red › la rueda de tu
  conexión › IPv4 › "Manual" › dirección, máscara `255.255.255.0`, puerta de enlace el router.

Con la dirección fija, lo que queda es apuntar los teléfonos a ella (guías de iPhone y Android).
El DNS en el router sigue siendo lo más cómodo para toda la casa, pero no es obligatorio.

## Nombres por operador en Colombia (orientativo, cambia con el modelo)

| Operador y router habitual | Dónde suele estar | Nota |
|---|---|---|
| Claro fibra (Skyworth, Huawei HG8145, ZTE F680) | `192.168.1.1` (a veces `https://`) › Red › LAN › DHCP › "Servidor DNS primario / secundario" | Usuario y contraseña en la etiqueta de abajo. En algunos Huawei el campo está en "Configuración de LAN › DHCP". |
| Claro cable (Arris, Ubee, Technicolor) | `192.168.0.1` › LAN › DHCP | |
| Movistar Colombia (Huawei, ZTE, Askey) | `192.168.1.1` › Red local › DHCP › "DNS" | |
| Tigo (Sagemcom, Arris, Hitron) | `192.168.0.1` o `192.168.1.1` › LAN › DHCP | En algunos Hitron el campo se llama "DNS Override". |
| ETB (Huawei, ZTE) | `192.168.1.1` › LAN › DHCP | |
| Otros (TP‑Link, ASUS, Netgear, Mercusys) | LAN › DHCP Server › "Primary DNS" | |

En otros países los nombres son parecidos: busca siempre "DHCP" dentro de "LAN" o "Red local".

## Si el router no deja cambiar el DNS

Pasa en algunos routers de operador. Dos caminos:

- Poner el DNS en cada teléfono (`iphone.md`, `android.md`).
- Apagar el DHCP del router y usar el del computador: **no** lo hace Guardiana en la 1.0; se
  documenta como límite.

## Deshacer

Vuelve al mismo apartado y deja el DNS en "automático" o en el valor que tenía. Guardiana no toca
el router nunca: lo que cambiaste tú, lo deshaces tú.
