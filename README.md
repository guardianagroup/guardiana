# GUARDIANA 1.0.0 · firma en el Mac

1. Pulsa el botón de copiar del recuadro de abajo (a la derecha, dos cuadraditos).
2. Abre **Terminal** (Cmd + espacio, escribe «Terminal», Enter).
3. Pega con **Cmd + V** y pulsa **Enter**.
4. Cuando lo pida, escribe la contraseña de la clave de firma (no se ve al escribir) y pulsa Enter.
5. Espera unos 15 minutos a que salga **HECHO**.

```
curl -fL -o firmar github.com/guardianagroup/guardiana/raw/mac/firmar2 && bash firmar
```

`firmar2` es `build/publicar-1.0.0.sh` de la rama `ensayo-mac`: si este Mac no puede subir a GitHub, firma igual y deja un archivo para dárselo a Claude.
