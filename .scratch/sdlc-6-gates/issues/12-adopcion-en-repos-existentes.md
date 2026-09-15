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

## Adopcion por repo (2026-09-15, sesion 2)

Sonda por repo: worktree detached desde HEAD, dependencias ignoradas enlazadas (como `seed`), comando real.
Cada hueco del pipeline que la sonda expuso quedo corregido con test en `mmorch/sdlc.py`:

- `3df3a38` interprete del repo: la suite y los comandos corrian con el Python de mmorch, no con la `.venv` del repo.
- `1ffb704` `{files}` en `compile_cmd`/`lint_cmd` (los hints `<archivo>` de init nunca se reemplazaban); la config del
  `sdlc.toml` del repo aplica aunque el worktree de HEAD no lo tenga; `seed_globs` desde `sdlc.toml`.
- `8eeccbf` `suite_cmd`: la suite total era pytest siempre; en otro lenguaje compara verde/rojo contra el baseline.
- `4ff200a` etapa 1 y trazabilidad en otros lenguajes: `test_R<n>` como funcion o titulo; prompt sin "pytest".
- `b211eb9` techo con `**/` opcional: `src/**/*.py` dejaba afuera `src/x.py` (Adepor).
- `85a5086` init sin `tests/` no escribe `accept_cmd`: en Adepor, pytest en la raiz recolectaba scrapers con red.
- `31e0bb6` init detecta lenguajes sobre `git ls-files` (no carpetas sin trackear).

| Repo | Estado | Evidencia | Falta (usuario) |
|---|---|---|---|
| Estudio (`app/`, TS) | ENTRA | vitest 17/17 verde en 35 s; tsc limpio; eslint 9 errores previos -> lint por archivo | commitear `sdlc.toml` + `docs/sdlc/` en la rama `init`; elegir la feature de prueba (grilling) |
| Portfolio financiero (Python) | ENTRA | suite en worktree con `.venv` sembrada: 1705 passed, 2 rojos previos, 19 min (tests con mocks, sin broker real) | commitear `sdlc.toml` + `docs/sdlc/`; feature de prueba (grilling) |
| QueTePario/ChatBot (Java) | BLOQUEADO | `mvn` no esta en el PATH (el A/B del 09-10 lo uso) | instalar Maven; despues sonda `mvn -f backend/pom.xml test` |
| Proyecto_Adepor (Python) | NO ENTRA | sin suite; los `test_*.py` de `analisis/` son scrapers con red | definir un oraculo determinista (`accept_cmd`) |

Los `sdlc.toml` quedaron SIN commitear en cada repo (commit = decision del usuario). El backend Python de
"Proyecto SaaS" (Estudio) queda fuera: sin venv propio.

- Commits de adopcion por pedido del usuario (2026-09-15): Estudio `b7e7ab0`, Portfolio `65cf5f1`, ChatBot `0725a12`,
  Adepor `1d5c71e` (solo sdlc.toml + docs/sdlc; el `.beads/issues.jsonl` staged del usuario quedo fuera).
- Maven: la descarga desde la terminal esta bloqueada por permisos; el usuario la corre (Apache 3.9.11 con sha512).
- Candidatas de feature de prueba (evidencia en docs del repo): Estudio = exportar mastery a la wiki
  (spec 2026-05-13 :171 y :248); Portfolio = shortfall por ticker en `scripts/cost_audit.py`
  (`docs/automatizacion/roadmap_proposals_futuros.md:36`). Pendiente: grilling de requisitos con el usuario.
