# Oraculo visual para sprites y assets
Type: grilling
Status: open
Blocked by: 08, 14
Map: ../map.md

## Question

El usuario quiere que el pipeline lo ayude con sprites y cuestiones visuales de un juego, con un oraculo que APRENDA
de sus veredictos (2026-09-15). Lo determinista (hitbox dentro del alpha, tamanos, paleta) no ayuda con "queda bien".
Decidir: (a) juez visual = modelo con vision ya configurado en providers (gemini-2.5-flash / flash-lite) con rubrica por
tipo de asset (silueta legible al tamano objetivo, paleta coherente, luz consistente, frames de animacion coherentes);
(b) que aprende: los veredictos humanos (imagen + etiqueta + motivo) entran como few-shot del juez y como ejemplos del
checker sintetizado, mismo protocolo que el ticket 08 (sombra hasta kappa >= 0.6); (c) Hermes = canal: manda el asset
por Telegram y devuelve el veredicto en un toque (ticket 14); (d) donde vive: `veredictos.jsonl` con `kind: sprite` y
un gate `visual` de la etapa 5 que solo observa hasta tener numero. Que se mide antes de confiar: acuerdo con el usuario
sobre >= 20 assets etiquetados. Sin juego real no hay datos: el ticket espera al primer proyecto con sprites.
