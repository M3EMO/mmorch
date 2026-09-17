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

## Sesion 3 (2026-09-15): features de prueba, oraculo de Adepor, seguridad

- Seguridad Adepor (`9bcf506`): `credentials.json` (cliente OAuth Google) y `token.json` (refresh token, scopes Drive +
  Sheets) fuera del indice. SIGUEN en `origin/main` de GitHub y en el historial: el usuario debe revocar/rotar y decidir
  si purga el historial y pushea.
- Oraculo Adepor (opcion A, `b197ffe` en main): `tests/test_oraculo_motor.py`, unittest hermetico sin red, 15 tests,
  7/7 mutantes manuales muertos; `accept_cmd`/`suite_cmd` = unittest + self-test de split_forward. `min_ev_escalado`
  queda FUERA: el codigo usa escalones y `Reglas_IA.txt:196` una formula; decide el usuario. Limite: la etapa 1 y el
  test-compile corren pytest para tests .py y la `.venv` de Adepor no tiene pytest.
- Features de prueba aceptadas por el usuario, tests escritos desde el borrador R1..R4 y verificados (rojo en HEAD,
  verde con una implementacion de referencia descartada):
  - Portfolio `sdlc/shortfall-por-ticker` (`3cb0bc4`): `tests/test_sdlc_shortfall_por_ticker.py`.
  - Estudio `sdlc/export-mastery` (`fa7b2fb`): `app/src/sdlc/export_mastery.test.ts`, tsc limpio.
  Pendiente: veredicto humano sobre cada test (no se registra en nombre del usuario) y despues la corrida por el server
  con `resume_branch` + `from_stage=2` + `verdict`.
- Huecos de mmorch hallados: `21d888a` PYTHONDONTWRITEBYTECODE (mutantes del mismo tamaño en el mismo segundo reusaban el
  .pyc: 2 falsos vivos de 7); `c3e9bed` el server reconoce tests nombrados de cualquier lenguaje.

- Veredictos del usuario 2026-09-15 (2 rechazos, registrados en `logs/sdlc/veredictos.jsonl`): shortfall-por-ticker
  rechazado porque quiere medir relaciones y patrones entre datos macro; export-mastery rechazado porque quiere YAML.
  Segunda vuelta: Estudio `1c259df` (YAML con subject/generated/concepts) y Portfolio rama `sdlc/macro-leadlag`
  (`fb8b2b4`): `mapa_lead_lag` sobre variaciones, ranking por correlacion OOS, 4/4 mutantes muertos. Adepor: el
  usuario decidio que manda el codigo en `min_ev_escalado` (`c276b73`, 16 tests).

## Sesion 4 (2026-09-16/17): corridas reales, Adepor fuera, huecos reforzados

- Decision del usuario (2026-09-17): Proyecto_Adepor queda FUERA de este ticket (no corre feature de prueba).
- Corridas por el server con veredicto:
  - Estudio export-mastery (TS): BUILT en `mmorch/wt-92141994`, US$0.018, 6 min, 0 intervenciones, 21 tests verdes.
    Primer repo no Python validado de punta a punta (compile_cmd tsc atrapo `topic.quiz` posiblemente undefined).
  - Portfolio macro-leadlag: la primera corrida (`wt-e570693d`) paso R1..R4 pero borraba filas con NaN antes de
    diferenciar (lag 2 en vez de 3 con 25% de huecos). El usuario aprobo R5 (`255f77d`); segunda corrida BUILT en
    `mmorch/wt-11293607`, US$0.42, 67 min, 0 intervenciones; la revision de Claude bloqueo una vez con test. Salida
    identica a una referencia en 12 series macro reales. Recorte manual de codigo muerto (`6a7e2d0`, 333 -> 80 lineas).
- Huecos de mmorch hallados por estas corridas y corregidos con test:
  - `018f4bf` baseline con archivo nuevo (`git checkout HEAD -- nuevo` explotaba).
  - `4c190c0` lock: dos jobs del server se pisaban el worktree (globals de sdlc.py); ahora cada job corre en su proceso.
  - gate `codigo-muerto` (etapa 5, antes del lint): fraccion de un .py NUEVO que corre la aceptacion, `cobertura_min`
    0.8 medido (construido 0.533, recortado 0.947); coder -> Claude -> humano.
  - mutacion solo sobre las lineas que cambio la feature; test de revision con nombre por feature; diffstat del job
    contra la base del worktree; veredicto repetido no suma ejemplo (log deduplicado 17 -> 7, respaldo `.bak-2026-09-17`).
- Estado: Estudio y Portfolio ENTRAN con numeros; ChatBot BLOQUEADO hasta instalar Maven; Adepor FUERA.
- ChatBot DESBLOQUEADO (2026-09-17): el usuario instalo Maven 3.9.11 (Java 17). Sonda en worktree de HEAD:
  `mvn -q -B -f backend/pom.xml test` verde, 13 tests (Core 4, Bot 5, Webhook 4), 39 s, arbol limpio. Falta la feature
  de prueba (grilling con el usuario). mmorch `db9d58c` = los 6 huecos de la sesion 4.
- ChatBot reposicion (2026-09-17): corrida BUILT en `mmorch/wt-8d07232f` (US$0.014, 12 min, mutacion 0.857) con un
  defecto real que la revision de Claude probo con test (clave de deduplicacion con "|" sin escapar) y el pipeline
  descarto. Huecos corregidos: accept() en repos no Python tambien corre el test pytest de la revision; el baseline de
  `suite_cmd` se mide sin los tests de aceptacion (en Java/TS no compilaban en la base y el gate solo observaba).
  Pendiente: reanudar ChatBot desde la etapa 5 y verificar antes del merge. ChatBot no tiene remoto.
