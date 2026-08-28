# 04 — Formato de la propuesta (hook en sesión + digest)

Type: prototype
Status: resolved
Blocked by: 03

## Question

Prototipar el artefacto que ve el usuario — texto exacto que inyecta el hook
(máx 1 por sesión, solo match fuerte con el proyecto actual) y la sección
"ideas pendientes" del resumen matutino. Reaccionar sobre ejemplos concretos:
largo, tono, qué cita (nota, proyecto, archivo si codegraph), qué acciones
ofrece ("dale"/"no"/silencio), en qué hook engancha (SessionStart vs primer
Stop) sin sumar latencia perceptible.

## Answer

Prototipado 2026-08-12, aprobado por el usuario.

- **Enganche**: hook SessionStart NUEVO (matcher startup), lee
  `adjudications.json` local (ms, cero API), stdout → inyecta al contexto. El
  texto viene PRE-COCINADO por el nightly — el hook no llama LLMs. Stop
  descartado (solo stderr).
- **Artefacto A (propuesta en sesión)**: tarjeta de 2-3 líneas — 💡 mmorch +
  link a la nota + qué aplica y dónde (cita archivo si codegraph) + score y
  "refutado y sobrevivió" + acciones: "dale" (arranca en sandbox) / "no" (no
  se propone más) / ignorar (expira solo). Máx 1 por sesión, solo match ≥0.7
  con el proyecto del cwd.
- **Artefacto B (digest 09:10)**: sección "Ideas pendientes" — una línea por
  idea: proyecto ← fuente (nota o candidata roadmap+lente), gist, score,
  estado (nueva / vence en N días). Footer: ver candidatos.md, aceptar con
  "dale la N" o editando roadmap.md.
- **Ampliación en digest** (pedido del usuario): "ampliá la N" en la sesión
  del digest rinde esa idea como tarjeta formato A completa (justificación,
  cita, acciones) leyendo el detalle de candidatos.md/adjudications.json.
