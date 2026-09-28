# GUARDIANA 1.0.1 · firma en el Mac

1. Abre **Terminal** (Cmd + espacio, escribe «Terminal», Enter).
2. Escribe esto y pulsa **Enter**:

```
curl -fL -o firmar github.com/guardianagroup/guardiana/raw/mac/firmar-1.0.1 && bash firmar
```

3. Cuando lo pida, escribe la contraseña de la clave de firma (no se ve al escribir) y pulsa Enter.
4. Espera unos 15 minutos a que salga **HECHO** y pásale a Claude el archivo `guardiana-firmado-1.0.1.zip`
   (se abre una ventana con él).

`firmar-1.0.1` toma lo que compiló GitHub de la rama lanzamiento-1.0.1 de la web, comprueba sus huellas,
firma, anota en Rekor y deja un archivo para Claude, que sube el registro y publica la web.
Este Mac no sube nada a GitHub.
