# Hermes como acompañante de la aceptación
Type: grilling
Status: open
Blocked by: 08, 12 (ambos cerrados; desbloqueado 2026-09-17)
Map: ../map.md

## Question

El usuario quiere "entrenar un agente Hermes de forma que haya una aceptación o acompañamiento" (2026-09-15).
Decidir qué rol exacto tiene Hermes en la etapa 1 (aceptación) del pipeline:
(a) canal del nivel humano: recibe el test de aceptación por Telegram/WhatsApp, devuelve aprobar/rechazar y el motivo;
(b) pre-juez: con los ejemplos etiquetados del ticket 08 (3 buenos + 3 malos) da un veredicto previo que el humano confirma;
(c) autor: escribe el test de aceptación desde la tarea en vez de Claude.
Qué se entrena (prompt/skill por gate vs checker sintetizado), con qué datos (los veredictos humanos de `approve_accept`),
y cómo se mide antes de confiarle una aprobación sin humano. Punto de enganche ya previsto: el aprobador de la etapa 1
es reemplazable (hoy: humano que reanuda desde la etapa 2).
