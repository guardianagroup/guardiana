#!/usr/bin/env python3
"""Convierte los textos de bienvenida del instalador en los RTF que enseña WiX.

El instalador de Windows enseña la pantalla de bienvenida como RTF, y escribir RTF a mano lleva
a erratas invisibles (una tilde mal escapada sale como un signo raro en la pantalla que ve el
comprador). Los textos viven en .txt, uno por idioma, y este guion hace los .rtf:

    python3 build/wix/textos/generar-rtf.py

Los .rtf resultantes se guardan en el repositorio: la compilación del MSI no depende de Python.
"""
import pathlib
import sys

AQUI = pathlib.Path(__file__).resolve().parent
CABECERA = (
    "{\\rtf1\\ansi\\ansicpg1252\\deff0"
    "{\\fonttbl{\\f0\\fswiss\\fcharset0 Segoe UI;}}\\f0\\fs20\n"
)


def rtf(texto: str) -> str:
    fuera = [CABECERA]
    for linea in texto.strip("\n").split("\n"):
        escapada = []
        for ch in linea:
            if ch in "\\{}":
                escapada.append("\\" + ch)
            elif ord(ch) < 128:
                escapada.append(ch)
            else:
                # \uN? es la forma de RTF de escribir una letra que no es ASCII; el «?» es lo
                # que enseña un lector antiguo que no entienda Unicode.
                escapada.append(f"\\u{ord(ch)}?")
        fuera.append("".join(escapada) + "\\par\n")
    fuera.append("}\n")
    return "".join(fuera)


def main() -> int:
    hechos = 0
    for origen in sorted(AQUI.glob("bienvenida-*.txt")):
        destino = origen.with_suffix(".rtf")
        destino.write_text(rtf(origen.read_text(encoding="utf-8")), encoding="ascii")
        print(f"{destino.name}: {destino.stat().st_size} bytes")
        hechos += 1
    return 0 if hechos else 1


if __name__ == "__main__":
    sys.exit(main())
