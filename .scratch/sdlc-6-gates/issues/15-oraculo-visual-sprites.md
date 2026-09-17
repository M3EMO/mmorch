# Oraculo visual para sprites y assets
Type: grilling
Status: resolved (2026-09-17)
Blocked by: 08, 14
Map: ../map.md

## Question

El usuario quiere que el pipeline lo ayude con sprites y cuestiones visuales de un juego, con un oraculo que APRENDA
de sus veredictos (2026-09-15). Lo determinista (hitbox dentro del alpha, tamanos, paleta) no ayuda con "queda bien".
Decidir: (a) juez visual = modelo con vision ya configurado en providers (gemini-2.5-flash / flash-lite) con rubrica por
tipo de asset (silueta legible al tamano objetivo, paleta coherente, luz consistente, frames de animacion coherentes);
(b) que aprende: los veredictos humanos (imagen + etiqueta + motivo) entran como few-shot del juez y como ejemplos del
checker sintetizado, mismo protocolo que el ticket 08 (sombra hasta kappa >= 0.6); (c) [CORREGIDO 2026-09-17: el ticket 14 no eligio a Hermes como canal; el canal movil quedo en la niebla] Hermes = canal: manda el asset
por Telegram y devuelve el veredicto en un toque (ticket 14); (d) donde vive: `veredictos.jsonl` con `kind: sprite` y
un gate `visual` de la etapa 5 que solo observa hasta tener numero. Que se mide antes de confiar: acuerdo con el usuario
sobre >= 20 assets etiquetados. Sin juego real no hay datos: el ticket espera al primer proyecto con sprites.

## Answer (2026-09-17, decisiones del usuario + evidencia)

Evidencia completa: `research/juez-visual-2026-09-17.md` (dos investigaciones web) y el prototipo medido
`research/sprites_checks.py` (10 defectos inyectados, 10 atrapados, 0 falsos positivos).

- D1 (usuario): se construye TODO ahora, no se espera al juego: capa determinista que BLOQUEA, camino de imagenes y
  juez visual EN SOMBRA. Implementado en `mmorch/sprites.py` + `gate_sprites()` en la etapa 5 + bloque `[sprites]` de
  `sdlc.toml` (dir, lado, paleta, tope_colores, hitboxes, animaciones, referencias). Sin ese bloque el gate no mide nada.
- D2 (usuario): el juez pregunta de a PARES contra una referencia aprobada Y contesta una rubrica binaria por criterio;
  nunca puntaje absoluto (gemini-2.5-flash: Pearson 0.459 pero 32.1% de acierto exacto).
- D3 (usuario): sale de sombra con >= 50 sprites etiquetados por el humano y kappa >= 0.6 (`confiable()`); el ticket
  original pedia 20, la practica publicada pide 50 a 100.
- Ajuste tecnico que salio de la evidencia: el sprite viaja agrandado x8 con vecino-mas-cercano, porque el acierto del
  juez cae con imagenes chicas (59.63% a 384px vs 67.96% a 1536px).
- Hallazgo: `providers.py` NO necesito cambios. El endpoint OpenAI-compatible de Gemini acepta `image_url` con data URL,
  asi que el camino de imagenes fue armar el mensaje, no tocar el proveedor.
- La opinion del juez va a `logs/sdlc/juez_visual.jsonl` y nunca cambia el resultado del gate.
- Lo que queda para cuando exista el juego: etiquetar 50 sprites (kind sprite en veredictos.jsonl) y recien ahi medir
  kappa. Sin datos del usuario, el juez no sale de sombra.
