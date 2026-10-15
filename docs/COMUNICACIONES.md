# Guardiana Comunicaciones

Guardiana Comunicaciones es la siguiente superficie del producto: una experiencia para coordinar conversaciones y canales sin convertir el panel de protección en una bandeja de mensajes opaca.

## Principios de la primera versión

- **Local primero.** La configuración, el registro de cambios y las pruebas viven en el equipo. No se añade un servidor de telemetría.
- **Contenido fuera del panel.** Guardiana puede mostrar el estado y las conexiones que un canal permita auditar; no debe copiar ni interpretar el contenido de una conversación.
- **Permiso por canal.** Cada integración tiene su propio permiso, alcance, dispositivo y forma de revocación.
- **Verdad visible.** Un dato no disponible aparece como desconocido. No se deducen personas, ubicaciones ni relaciones a partir de una hora, una IP o una zona horaria.
- **Una ventana.** Comunicaciones debe vivir en la misma ventana de Guardiana y compartir la revisión, el mapa y los extractos.

## Orden de construcción

1. Identidad local y espacios: persona, dispositivo, grupo y roles mínimos.
2. Adaptadores de canal aislados: correo, mensajería y servicios de IA; cada adaptador declara qué metadatos puede entregar.
3. Registro encadenado local de cambios de configuración, permisos y eventos de transporte.
4. Vista en directo y mapa: solo hechos recibidos del adaptador, con origen, hora, destino y nivel de certeza.
5. Exportación verificable: resumen humano, JSON y PDF con huella y firma local.

La pantalla inicial se publica antes de conectar un canal para fijar esta promesa en la interfaz y permitir revisar la arquitectura con calma. Las conexiones reales deben llegar acompañadas de pruebas, límites explícitos y una ruta de borrado.
