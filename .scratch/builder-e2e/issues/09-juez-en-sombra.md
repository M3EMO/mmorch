# Juez en sombra con rúbricas
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cómo opina un verificador separado sobre el residuo no computable, sin bloquear nunca?

- Entradas: capturas de escritorio y móvil del driver y el texto de la página.
- Rúbricas con anclas 1/3/5 (texto SEO y calidad visual, ticket 03) y comparaciones de a pares entre entregables.
- Verificador: `deepseek-reasoner` (el mejor medido); la opinión queda en el run-log.
- Medir cuándo podría pasar a bloquear: acuerdo con el juicio del usuario sobre pares etiquetados.

## Answer

Resuelto (2026-10-15). Módulo `mmorch/juez_sombra.py`, conectado al builder (sección opcional `juez` del plan).

- Generaliza el juez visual de `sprites.py`: compara dos entregables de a pares, con las rúbricas del ticket 03 como anclas y criterios binarios, sin puntajes numéricos.
- Juzga cada par dos veces con el orden invertido; si los órdenes no coinciden, la opinión es "inconsistente".
- Texto: `DEFAULT_VERIFIER` (deepseek-reasoner). Imagen: el VLM de `sprites.py` (`glm-4.6v`).
- Todo va a `logs/sdlc/juez_sombra.jsonl`. Una falla del juez queda registrada y nunca tumba una corrida. La opinión nunca cambia el código de salida del builder.
- Sale de sombra con kappa ≥ 0,6 sobre 20 pares etiquetados por el usuario (`confiable()`, reusa `sprites.kappa`).
- El driver guarda la evidencia con un tercer argumento: `texto.txt`, `escritorio.png` y `movil.png`.

Corrida real: `prototipo-04` contra `prototipo-06`, los dos sitios de F2 armados por el builder (capturas en `research/09-juez/capturas/`).

- Texto SEO: gana `prototipo-04` en los dos órdenes (opinión consistente). Coincide con el juicio del orquestador: `prototipo-04` explica aportes y Ganancias y etiqueta cada línea del desglose.
- Visual: no corrió. GLM devolvió saldo insuficiente (código 1113); la falla quedó en sombra.

Pendiente: recargar saldo de Zhipu o elegir otro VLM, y que el usuario etiquete pares para medir kappa.
