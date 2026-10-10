# GUARDIANA — Cómo comprobar que lo que tienes es lo que publicamos

Brief §10. GUARDIANA se publica de forma que cualquiera pueda comprobar, sin fiarse de nadie, que
el programa instalado es exactamente el que se publicó y que se compiló del código público.

## En un minuto: `guardiana verify`

En el equipo donde está instalada, ejecuta:

```bash
guardiana verify
```

Muestra, en este orden:

1. **Huella del programa**: el SHA‑256 del ejecutable instalado.
2. **Firma**: si junto al ejecutable hay un archivo `.minisig`, lo comprueba con la clave pública
   que va dentro del propio programa. Si el programa lleva la clave de desarrollo (versiones de
   prueba), lo dice tal cual.
3. **Registro público**: si hay un `ledger.jsonl` (junto al programa o descargado por ti), busca la
   huella en él y dice a qué versión y fecha corresponde. Si no está, lo dice.
4. **Servicio**: si está instalado y en marcha.
5. **DNS del sistema**: a qué resolutor apunta el equipo y si GUARDIANA está de primario.
6. **Modo Vigilante y Modo Hogar**: en qué aparatos está encendido el Modo Vigilante, y si el Modo
   Hogar está encendido, apagado o apartado.
7. **Puertos abiertos hacia la red local**: con Modo Hogar apagado debe listar **ninguno**.
8. **Listas**: fecha y tamaño de las listas que lleva dentro.
9. **Extracto**: comprueba la cadena de hashes y cuenta los eventos.

`guardiana verify --json` da lo mismo en formato para máquinas. El panel lo muestra en `/verify`.

## El registro público: `ledger.jsonl`

Una línea por versión publicada, en este repositorio, **antes** de que exista la descarga:

```json
{"version":"1.0.0","commit":"<git>","date":"2026-10-06T10:00:00Z","files":[{"name":"guardiana-1.0.0-windows-x64.msi","sha256_unsigned":"…","sha256_signed":"…","minisign":"…"}],"rekor_uuid":"…"}
```

- `sha256_unsigned`: huella del binario tal cual sale de la compilación reproducible.
- `sha256_signed`: huella del mismo binario después de la firma de código de Windows (cambia porque
  la firma se añade dentro del archivo). En Linux ambas coinciden.
- `minisign`: la firma minisign del binario sin firmar.
- `rekor_uuid`: la entrada en Rekor (registro de transparencia público de Sigstore) donde se subió
  esta línea. Sirve para demostrar que la línea existía en esa fecha y no se cambió después.
- `reproducible`: si el archivo sale idéntico de `build/repro.sh` (`true`) o no (`false`: los
  instaladores y el navegador se compilan en las máquinas de GitHub y se comprueban por su huella).

Lo que se sube a Rekor es la línea **antes** de saber su `rekor_uuid` (no se puede meter dentro de
una entrada su propio número). Para comprobarla: toma la línea de `ledger.jsonl`, pon
`"rekor_uuid":""`, escríbela sin espacios y con un salto de línea al final, y su SHA‑256 tiene que
ser el `data.hash` de la entrada de Rekor:

```bash
tail -n 1 ledger.jsonl | python3 -c 'import json,sys; l=json.loads(sys.stdin.read()); u=l["rekor_uuid"]; l["rekor_uuid"]=""; open("linea.json","w").write(json.dumps(l,separators=(",",":"))+"\n"); print(u)'
sha256sum linea.json   # igual al hash de https://rekor.sigstore.dev/api/v1/log/entries/<uuid>
```

La firma minisign de esa misma línea va dentro de la entrada de Rekor, con la clave pública.

## Reproducir la compilación

`build/repro.sh` compila GUARDIANA dentro de un contenedor fijado por huella (mismo compilador,
mismas dependencias, misma fecha de compilación tomada del commit) y escribe los SHA‑256 de los
binarios. Quien lo ejecute sobre el mismo commit debe obtener las mismas huellas que
`sha256_unsigned` en `ledger.jsonl`. Si alguien ajeno lo hace, se anota aquí:

| Fecha | Quién | Versión | Resultado |
|---|---|---|---|
| — | — | — | todavía nadie |

## Comprobar a mano, sin GUARDIANA

```bash
sha256sum guardiana-1.0.0-linux-x86_64          # compara con ledger.jsonl
minisign -Vm guardiana-1.0.0-linux-x86_64 -P <clave pública publicada en la web>
```

La clave pública minisign está en `build/pubkey/minisign.pub` de este repositorio y en la web.
Si esa clave cambiara alguna vez, sería un aviso serio: la clave es la identidad del programa.
