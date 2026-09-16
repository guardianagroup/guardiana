# Guía: iPhone y iPad

Sirve cuando no quieres tocar el router, o el router no deja cambiar el DNS. Solo vale para la
Wi‑Fi de casa: con datos móviles, el iPhone no pasa por Guardiana.

## Pasos

1. En el panel de Guardiana › "Modo Hogar", apunta la IP del computador (ejemplo `192.168.1.39`).
2. En el iPhone: **Ajustes › Wi‑Fi** › toca la **(i)** junto a tu red de casa.
3. Baja hasta **Configurar DNS** › **Manual**.
4. Borra los servidores que aparezcan (el botón rojo) y pulsa **Añadir servidor**.
5. Escribe la IP del computador. Si quieres respaldo cuando el computador esté apagado, añade
   debajo la IP del router (normalmente `192.168.1.1`); esas consultas no las verá Guardiana.
6. **Guardar** (arriba a la derecha).
7. Abre Safari y entra en `http://comprobar.guardiana.hogar`. Si carga "Este dispositivo ya pasa
   por Guardiana", listo. Si no carga, apaga y enciende la Wi‑Fi y vuelve a probar.

## Lo que hay que saber

- **iCloud Private Relay** (Ajustes › tu nombre › iCloud › Relay privado) hace que Safari resuelva
  los nombres por su cuenta: esas consultas no pasan por Guardiana. Guardiana no lo bloquea; lo
  anota como "tráfico fuera de vista" cuando lo detecta.
- Si tienes una **VPN** activa, tampoco pasa por Guardiana.
- Este cambio es solo para esa red Wi‑Fi. En otras redes, el iPhone usa su DNS normal.

## Deshacer

Ajustes › Wi‑Fi › (i) › Configurar DNS › **Automático**. Hazlo antes de apagar el computador
guardián por mucho tiempo, o el iPhone se quedará sin DNS en casa.
