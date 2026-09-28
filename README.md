# GUARDIANA 1.0.1 · firma en el Mac

Antes: en Safari, con la sesión de GitHub abierta, baja los dos archivos de la ejecución
[36473167217](https://github.com/guardianagroup/guardiana/actions/runs/36473167217) (abajo, «Artifacts»):
**guardiana-repro** y **guardiana-instaladores**. Se quedan en Descargas.

1. Pulsa el botón de copiar del recuadro de abajo (a la derecha, dos cuadraditos).
2. Abre **Terminal** (Cmd + espacio, escribe «Terminal», Enter).
3. Pega con **Cmd + V** y pulsa **Enter**.
4. Cuando lo pida, escribe la contraseña de la clave de firma (no se ve al escribir) y pulsa Enter.
5. Espera unos 15 minutos a que salga **HECHO** y pásale a Claude el archivo `guardiana-firmado-1.0.1.zip`.

```
curl -fL -o firmar github.com/guardianagroup/guardiana/raw/mac/firmar-1.0.1 && bash firmar
```

`firmar-1.0.1` comprueba las huellas de lo que compiló GitHub, firma, anota en Rekor y deja un archivo
para Claude, que sube el registro y publica la web. Este Mac no sube nada a GitHub.
