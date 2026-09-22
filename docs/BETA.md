# Guardiana — Guía para las casas fundadoras (beta cerrada)

Versión 0.1.0 · septiembre de 2026. Gracias por probar Guardiana antes que nadie. Esta guía es
para diez casas: cinco con Windows y cinco con Linux. Lee entera la sección "Lo que no hace"
antes de instalar; es corta y evita malentendidos.

## 1 · Qué recibes

Un enlace de descarga privado y, aparte, este documento. En el enlace hay:

| Archivo | Para |
|---|---|
| `guardiana-0.1.0-windows-x64.msi` | Windows 10 u 11, 64 bits |
| `guardiana_0.1.0_amd64.deb` | Debian 11+, Ubuntu 20.04+ y derivados |
| `guardiana-0.1.0-linux-x86_64.tar.gz` | Cualquier Linux con systemd |
| `SHA256SUMS` | Las huellas de los tres archivos |
| `ledger.jsonl` | El registro público con esas mismas huellas y la firma |

En esta beta el programa **no lleva firma de código de Windows**, y hoy no hay ninguna comprada:
un certificado cuesta dinero y **no quita el aviso**, así que ese esfuerzo va a que el programa sea
gratis y comprobable. Medido en un Windows 11 el 15 de septiembre de 2026 con este mismo archivo: al abrirlo sale
**"Advertencia de seguridad de Abrir archivo"**, con "Publicador desconocido" y "Este archivo no
tiene ninguna firma digital válida"; se sigue con **"Ejecutar"**. Según el navegador y los ajustes
puede salir además la pantalla azul "Windows protegió su PC", donde el camino es "Más información"
→ "Ejecutar de todas formas". Conviene saber que ese aviso tampoco desaparece de golpe el día que
firmemos: desde 2024 Windows lo muestra a cualquier instalador que su sistema de reputación no
conozca todavía, firmado o no, y esa reputación se gana con las descargas. Por eso te damos la
huella: comprobarla es lo único que de verdad te dice que el archivo es el nuestro.

## 2 · Antes de instalar: comprueba la huella (dos minutos)

Windows, en PowerShell:

```
Get-FileHash .\guardiana-0.1.0-windows-x64.msi
```

Linux:

```
sha256sum guardiana_0.1.0_amd64.deb
```

Compara el resultado con la línea correspondiente de `SHA256SUMS`. Si no coincide, no instales y
escríbenos. Cómo comprobar además la firma minisign: `docs/VERIFY.md`.

## 3 · Instalar

**Windows:** doble clic en el `.msi`, siguiente, siguiente. Se instala en "Archivos de
programa\Guardiana", registra el servicio "Guardiana" y crea dos accesos en el menú Inicio.
**No cambia el DNS de tu equipo al instalar.**

**Linux (.deb):**

```
sudo apt install ./guardiana_0.1.0_amd64.deb
```

**Linux (tarball):**

```
tar xzf guardiana-0.1.0-linux-x86_64.tar.gz
cd guardiana-0.1.0-linux-x86_64
sudo ./instalar.sh
```

Los dos dejan el servicio en marcha y **tampoco cambian el DNS al instalar**.

## 4 · Los primeros cinco minutos

1. Abre el panel: menú Inicio › Guardiana (Windows) o `guardiana panel` (Linux).
2. Primera pantalla, la radiografía: verás "Guardiana está observando: 0 horas de 24". Es normal
   que esté vacía; el DNS de tu equipo todavía no pasa por Guardiana.
3. Pulsa "Vigilar este computador". Te explica qué va a cambiar (el DNS de tu equipo pasa a
   apuntar a Guardiana, con el resolutor anterior como respaldo) y pide tu confirmación. Solo
   entonces cambia algo.
4. Navega un rato. Vuelve al panel: cada servicio con su categoría y, cuando toque, una frase.
5. Deja el equipo así 24 horas. Antes de ese plazo el botón "cortar" no existe.

## 5 · Modo Hogar (los teléfonos)

En el panel, página "Modo Hogar" › "Encender". Te dará la IP de tu computador y un código QR.
Después, o bien pones esa IP como DNS en el router (toda la casa de una vez; guía en
`docs/guias/router.md`) o bien en cada teléfono (`docs/guias/iphone.md`, `docs/guias/android.md`).
Desde el teléfono, abre `http://comprobar.guardiana.hogar`: si carga la página, ya pasa por
Guardiana.

Tres cosas que tienes que saber, dichas ahora y no después:

- **Si el computador se apaga o se duerme, los teléfonos que apunten solo a él se quedan sin
  DNS.** Guardiana avisa si tu equipo tiene la suspensión activada. Un portátil con la tapa
  cerrada no vale como guardián. Si pones el DNS en el router, añade como secundario el DNS del
  propio router: los teléfonos no se quedarán sin internet, a cambio de que esas consultas no
  las vea Guardiana.
- **La IP del computador tiene que ser fija.** Guardiana avisa si la asigna el router y puede
  cambiar. La solución es una "reserva DHCP" en el router (guía del router).
- **Solo en la Wi‑Fi de casa, por nombre de servicio.** Con datos móviles el teléfono no pasa por
  Guardiana. Guardiana ve nombres de servicios, no qué app los pidió, ni contenido.

La prueba dura 7 días y el contador vive en tu equipo. Para la beta os daremos una clave de
licencia que la extiende.

## 6 · Qué queremos que nos cuentes

Lo que más nos sirve, en este orden:

1. **Cualquier pantalla que no entiendas.** Si una frase no se entiende a la primera, está mal.
2. **Algo que dejó de funcionar** en el equipo o en un teléfono después de instalar o de un
   corte. Qué era, a qué hora, y si "deshacer" lo arregló.
3. **Una foto de la radiografía** tras 24 horas, si quieres compartirla. Sin nombres de
   dispositivos si prefieres.
4. La salida de `guardiana verify` (menú › comprobar la instalación, o el comando).
5. Si el equipo se reinició, si Guardiana volvió a estar vigilando sola.

Escribe a `hola@guardianagroup.com`. Nada de lo que Guardiana anota sale de tu casa: lo que nos
cuentes, lo cuentas tú.

## 7 · Desinstalar (y dejar todo como estaba)

- **Windows:** Configuración › Aplicaciones › Guardiana › Desinstalar.
- **Linux (.deb):** `sudo apt remove guardiana`.
- **Linux (tarball):** `sudo ./desinstalar.sh`.

Al desinstalar, Guardiana apaga el Modo Hogar, quita su regla del cortafuegos y restaura el DNS
del equipo exactamente como estaba. Si pusiste la IP en el router o en un teléfono, eso lo
deshaces tú en el mismo sitio (ponlo en "automático"). El extracto queda en el equipo
(`C:\ProgramData\Guardiana` o `/var/lib/guardiana`); bórralo si no lo quieres.

## 8 · Lo que no hace

`docs/WHAT_IT_DOES_NOT_DO.md`. Resumen: no ve contenido, no ve qué app pidió cada nombre, no ve
lo que esquiva el DNS (VPN, DNS cifrado propio), no ve lo que hace el router por su cuenta, no
corta nada sin tu decisión ni antes de 24 horas, no es control parental, no contacta ningún
servidor nuestro. Nunca diremos "100 % seguro".
