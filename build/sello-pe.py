#!/usr/bin/env python3
"""Fixes the timestamp inside a Windows .exe so two builds of the same code are the same file.

The reproducible build promised on the site failed its first real test on 20 September 2026: two
runs of `build/repro.sh` on the same commit gave two different SHA-256 for `guardiana.exe`. The
whole difference was four bytes — the COFF `TimeDateStamp`, which the linker fills with the clock.
zig's linker does not take `--no-insert-timestamp`, so the field is rewritten here, after linking,
with the date of the commit (`SOURCE_DATE_EPOCH`), which is a date that means something and is the
same for everyone building that commit.

    python3 build/sello-pe.py <archivo.exe> <epoch>
"""
import struct
import sys


def sellar(ruta, epoch):
    datos = bytearray(open(ruta, "rb").read())
    if datos[:2] != b"MZ":
        raise SystemExit(f"sello-pe: {ruta} no empieza por MZ: no es un ejecutable de Windows")
    pe = struct.unpack_from("<I", datos, 0x3C)[0]
    if datos[pe : pe + 4] != b"PE\0\0":
        raise SystemExit(f"sello-pe: {ruta} no tiene cabecera PE donde debería")
    sitio = pe + 8
    antes = struct.unpack_from("<I", datos, sitio)[0]
    struct.pack_into("<I", datos, sitio, epoch & 0xFFFFFFFF)
    # El directorio de depuración lleva su propio sello; los binarios de publicación salen sin él
    # (`--strip-all`), pero si algún día lo llevan hay que sellarlo también y conviene enterarse.
    opcional = pe + 24
    magic = struct.unpack_from("<H", datos, opcional)[0]
    dir_debug = opcional + (112 if magic == 0x20B else 96) + 6 * 8
    if dir_debug + 8 <= len(datos):
        rva, tam = struct.unpack_from("<II", datos, dir_debug)
        if rva and tam:
            print(f"sello-pe: AVISO: {ruta} lleva directorio de depuración ({tam} bytes): "
                  "puede llevar otro sello dentro y no quedar reproducible", file=sys.stderr)
    open(ruta, "wb").write(datos)
    print(f"sello-pe: {ruta}: {antes} → {epoch}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    sellar(sys.argv[1], int(sys.argv[2]))
