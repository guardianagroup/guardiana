# Guía: Android

Solo para la Wi‑Fi de casa: con datos móviles, el teléfono no pasa por Guardiana. Los menús cambian
con la marca (Samsung, Xiaomi, Google...); los nombres de abajo son los más comunes.

## Antes: "DNS privado" tiene que estar apagado

Android trae un ajuste llamado **DNS privado** (Ajustes › Red e internet › DNS privado, o
Conexiones › Más ajustes de conexión › DNS privado en Samsung). Si está en "Automático" o con un
nombre de servidor, el teléfono resuelve por su cuenta con DNS cifrado y **no pasa por Guardiana**.
Ponlo en **Desactivado**. Guardiana lo detecta y lo anota como "evasión de DNS"; no lo corta.

## Pasos

1. En el panel de Guardiana › "Modo Hogar", apunta la IP del computador (ejemplo `192.168.1.39`).
2. **Ajustes › Wi‑Fi** (o Conexiones › Wi‑Fi) › mantén pulsada tu red › **Modificar** (o toca el
   engranaje › el lápiz).
3. **Opciones avanzadas** › **Ajustes de IP: Estática**.
4. Deja la IP que ya tiene el teléfono y la puerta de enlace (normalmente `192.168.1.1`).
5. **DNS 1:** la IP del computador. **DNS 2:** vacío, o la IP del router si quieres respaldo
   cuando el computador esté apagado (esas consultas no las verá Guardiana).
6. Guardar. Apaga y enciende la Wi‑Fi.
7. Abre el navegador y entra en `http://comprobar.guardiana.hogar`. Si carga, listo.

## Lo que hay que saber

- Chrome y otras apps pueden traer su propio **DNS seguro**. En Chrome: menú › Configuración ›
  Privacidad y seguridad › Usar DNS seguro › desactivar, o Guardiana no verá lo de Chrome.
- Con una **VPN** activa, nada pasa por Guardiana.
- Solo cambia esa red Wi‑Fi. Fuera de casa, el teléfono usa su DNS normal.

## Deshacer

Misma pantalla › Ajustes de IP: **DHCP**. Y, si lo apagaste, vuelve a poner el DNS privado como lo
tenías. Hazlo antes de apagar el computador guardián por mucho tiempo.
