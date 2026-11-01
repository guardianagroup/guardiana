# La VPN de GUARDIANA (en preparación)

> «Otra sección aparte, que sea la VPN… del máximo nivel… tecnología nunca antes vista.» «Y siempre código abierto.»
> (el dueño, 1 nov 2026, 16:21 y 16:36)

Investigación de la agencia: ronda 87 (Gemini y la revisión de Claude), en el repositorio de la web,
`herramientas/agencia-rondas/ronda87/`.

## Lo que la hace distinta

Una VPN que es a la vez el DNS de la casa, el antivirus y el cortafuegos del mismo programa:

- **El túnel y el escudo hablan entre sí.** Un programa que el antivirus marca pierde el túnel y la red al instante.
- **Aviso de lo que sale fuera del túnel**, con el nombre del programa, su firma y el destino; y una prueba de fugas que
  la persona lanza cuando quiere (DNS, IPv6, WebRTC, la ruta de DHCP que usa TunnelVision, CVE-2024-3661).
- **La ruta, en el mapa de Órbita**: por qué países y cables pasa, con las fuentes de Órbita.
- **Postcuántica**: intercambio híbrido (clásico + ML-KEM) que alimenta la clave precompartida de WireGuard, al estilo
  de Rosenpass y de Mullvad.
- **Dos operadores** en el doble salto: entrada y salida en proveedores y países distintos.

## Fases

1. **Túnel y escudo en Windows.** WireGuard en Rust (GotaTun, de Mullvad, o boringtun) con el adaptador Wintun;
   el cortafuegos de Windows (WFP, `crates/apps/src/cortafuegos.rs`) como interruptor de corte: nada sale si no es por el
   túnel, salvo hacia el propio servidor; el DNS de la casa dentro del túnel; IPv6 y WebRTC cerrados fuera del túnel.
   Servidores: los actuales (Núremberg, Helsinki y Miami) con WireGuard, y un alta anónima de la llave pública del equipo
   (sin cuenta y sin guardar a quién pertenece).
2. **Postcuántico y doble salto**; el aviso en vivo de lo que sale fuera del túnel.
3. **Anticensura** (AmneziaWG; el túnel dentro de los relés de llamadas que ya existen), defensa contra el análisis de
   tráfico (relleno y paquetes señuelo, bajo demanda) y servidores sin disco.
4. **Compilación reproducible, auditoría externa** y, si algún día se decide, móviles (hoy GUARDIANA es solo para PC).

## Reglas

- **Sin registros**: los servidores no guardan qué llave se conecta ni a dónde va. Lo que sí se hace contra el abuso, sin
  guardar nada: cerrar el puerto 25 de salida y frenar los barridos de puertos.
- **Sin promesas absolutas**: lo que la VPN no protege se dice en su pantalla.

## Decisiones pendientes del dueño

Gratuita o de pago; un segundo proveedor y país para las salidas (coste por confirmar); y la política contra el abuso.
