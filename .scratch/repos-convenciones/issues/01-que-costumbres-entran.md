# Qué costumbres entran al contrato de actualización
Type: grilling
Status: open
Blocked by:
Map: ../map.md

## Question

¿Qué costumbres del inventario del mapa mantiene al día el mecanismo de actualización?

El inventario lista tres grupos con dueños distintos:

- Grupo SDLC: `sdlc.toml`, plantillas de `docs/sdlc/`, puntero en `AGENTS.md`. `init()` ya los conoce.
- Grupo SDD: `docs/specs/<slug>/`, constitución, áreas. Hoy los crean la etapa 6 o el usuario.
- Grupo `new-project`: `docs/claude/`, `tests/README.md`, `.gitignore` de secretos, `bd init`. Hoy solo aplican a repos nuevos.

Opciones a decidir:

- A. Solo el grupo SDLC. El mecanismo es una extensión de `init`.
- B. Grupos SDLC y SDD. El mecanismo declara la constitución y ofrece áreas, pero nunca las escribe solo.
- C. Los tres grupos. El mecanismo y `new-project` comparten una sola lista de costumbres.

Preguntas derivadas:

- ¿Las decisiones no implementadas del ticket 11 de `sdlc-6-gates` (`contract_version`, `docs/sdlc/gates/`) entran, se implementan antes o se retiran?
- ¿`AGENTS.md` se crea cuando falta? Hoy `init` solo agrega el puntero a un archivo existente, y 9 de 10 repos sondeados no tienen `AGENTS.md` con puntero.
- ¿La constitución default `GOAL.md` sirve fuera de orchestration? Ningún otro repo sondeado tiene `GOAL.md`; varios tienen `CLAUDE.md`.
- ¿Los artefactos de corrida en `docs/sdlc/` (`spec.md`, `plan.md`, `run-log.json`, `suite.log`) son mesa de trabajo o historia? ChatBot y los prototipos F2 los tienen commiteados.
- ¿Una costumbre nueva entra al contrato con un ticket propio, o la agrega cualquier commit a `sdlc/`?

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_lista_unica_de_costumbres`: una sola tabla en código (por ejemplo `sdlc/convenciones.py`) declara cada costumbre elegida con su ruta y su chequeo. El test verifica tres cosas:

1. Cada costumbre de la tabla tiene una función de chequeo determinista sin red.
2. Un repo vacío en `tmp_path` falla cada chequeo, y el mismo repo tras aplicar el mecanismo pasa cada chequeo.
3. Una costumbre fuera de la tabla no cambia ningún archivo del repo (por ejemplo, el mecanismo nunca escribe `docs/claude/` con la opción A).
