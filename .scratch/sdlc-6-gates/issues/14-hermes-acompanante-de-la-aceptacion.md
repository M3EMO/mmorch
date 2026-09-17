# Hermes como acompañante de la aceptación
Type: grilling
Status: resolved (2026-09-17)
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

## Answer (2026-09-17, grilling HITL con el usuario)

Captura completa (fuera de git): `brainstorms/2026-09-17-ticket-14-hermes-aceptacion.md`.

- D1: el rol es REFUTAR el test de aceptacion antes de la aprobacion humana: propone casos borde como R nuevos; el
  veredicto sigue siendo humano. Motivo: los rechazos humanos fueron de intencion de producto, y los defectos reales
  (huecos en leadlag, clave con "|" en reposicion) aparecieron despues del build, por refutacion.
- D4/D5/D8 (reencuadre del usuario: "el SDLC pueda plantar un agente hermes, no tanto que viva en el proceso SDLC"):
  Hermes no es una etapa. La skill "refutar tests" es COMUN y vive en mmorch. Plantar un Hermes en un repo es una
  herramienta OPCIONAL del SDLC, sin default al adoptar; si se planta, usa la skill comun y guarda memoria en un perfil
  local por repo (`%LOCALAPPDATA%\hermes`), nunca en git. Hoy no hay evidencia de que un Hermes por repo aporte.
- D2/D10: el runtime sale de una medicion: deepseek-reasoner desde mmorch contra Hermes Agent 0.21.1
  (`Documents/Hermes/hermes-agent`, proveedor DeepSeek incluido), ambos con deepseek-reasoner.
- D3/D6: oraculo por ejecucion, sin juez LLM. El refutador propone TESTS. Acierto = falla con la version con defecto y
  pasa con la corregida; falla en ambas = falsa alarma. Banco: leadlag (`wt-e570693d` vs `6a7e2d0`), reposicion
  (`wt-8d07232f` vs `wt-f8223a1e`), export-mastery sin defecto conocido. 3 corridas por caso; atrapado = 2 de 3;
  aprueba con los 2 defectos atrapados y como maximo 1 falsa alarma.
- D7: aprender = datos + skill con gate: decisiones sobre propuestas y defectos post-build alimentan banco y memoria; una
  reescritura de la skill queda solo si el banco completo no empeora.
- D9/D11: la skill corre SIEMPRE antes de pedir el veredicto, hasta 3 propuestas; cada una compila, falla en HEAD y nombra
  R<n> o se descarta; las aceptadas entran al test con commit; cada decision queda en `veredictos.jsonl` como refutacion.
- Tickets nuevos: 16 (banco), 17 (skill + medicion), 18 (conexion a la etapa 1). Ticket 15 corregido: Hermes no es canal.
