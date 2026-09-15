# Adopción en los repos existentes
Type: task
Status: open
Blocked by: (05 y 11 cerrados) — desbloqueado 2026-09-15
Map: ../map.md

## Question

Aplicar la convención del ticket 11 a los repos vivos del usuario: QueTePario/ChatBot, Estudio, Portfolio financiero, Proyecto_Adepor, Lotus, y la propia orchestration. Para cada uno: qué le falta (aceptación ejecutable, docs/sdlc, gates), en qué orden se adopta, y una feature de prueba por repo corrida por el pipeline con sus números. Registrar qué repos NO pueden entrar todavía y por qué. AFK con checklist para el usuario donde haga falta acceso o decisión de producto.

## Resolucion (en curso, 2026-09-15, HITL)

- D5 (usuario acepta): un repo entra por `python -m mmorch.sdlc init <repo>` (sdlc.toml con techo de archivos,
  accept_cmd, suite, `approve_accept`; docs/sdlc/ con plantillas; puntero en AGENTS.md) y el pipeline gana la
  etapa 1 "aceptacion": Claude escribe `tests/test_sdlc_<id>.py` desde la tarea; gates deterministas: compila
  (collect-only), falla en HEAD (rojo por diseño), nombra R<n>. Aprobacion humana OBLIGATORIA por default
  (`approve_accept = true`), desactivable por repo. La corrida se detiene en `awaiting_approval` y se reanuda
  desde la etapa 2 (la etapa es el checkpoint). El aprobador es reemplazable: ticket 14 (Hermes).

## Ejecucion

- Mecanismo HECHO (2026-09-15): `python -m mmorch.sdlc init <repo>` + etapa 1 "aceptacion" en `mmorch/sdlc.py`,
  tests en `tests/test_sdlc_adopcion.py` (init idempotente; etapa 1: rojo -> awaiting_approval; verde en HEAD -> rechazo;
  sin R<n> -> rechazo; approve_accept=false sigue). `build_feature` arranca en la etapa 1 cuando el llamador no trae tests.
- Aplicado a `orchestration` (sdlc.toml en la raiz, docs/sdlc/, AGENTS.md). Pendiente por repo, a pedido del usuario:
  QueTePario/ChatBot, Estudio, Portfolio financiero, Proyecto_Adepor (`python -m mmorch.sdlc init <ruta>` y ajustar
  `accept_cmd`/`files`). Lotus ya no existe.
