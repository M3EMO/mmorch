# Qué costumbres entran al contrato de actualización
Type: grilling
Status: resolved
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
- ¿Entra la regla de formato de archivo como costumbre? La regla tiene tres casos:
  - Un humano escribe la configuración de su propio proyecto, y el archivo usa TOML.
  - Un programa le pasa datos a otro programa, y el archivo usa JSON con un JSON Schema.
  - Una herramienta externa exige YAML (pre-commit, bd, GitHub Actions, frontmatter), y el archivo usa YAML.

  El usuario prefirió JSON sobre YAML para contratos entre proyectos (chat del 2026-10-23). Falta decidir si la regla entra al oráculo. Con la regla adentro, el oráculo marca como desvío un YAML de configuración propia.

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_lista_unica_de_costumbres`: una sola tabla en código (por ejemplo `sdlc/convenciones.py`) declara cada costumbre elegida con su ruta y su chequeo. El test verifica tres cosas:

1. Cada costumbre de la tabla tiene una función de chequeo determinista sin red.
2. Un repo vacío en `tmp_path` falla cada chequeo, y el mismo repo tras aplicar el mecanismo pasa cada chequeo.
3. Una costumbre fuera de la tabla no cambia ningún archivo del repo (por ejemplo, el mecanismo nunca escribe `docs/claude/` con la opción A).

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-repos-convenciones-ticket-01.md`.

- Entran cuatro grupos: SDLC, SDD, `.gitignore` de secretos y `bd init`. Quedan fuera `docs/claude/` y `tests/README.md`.
- `bd init` entra al contrato. Su forma de aplicación (append de reglas y hooks de bd sin reescribir los md) la define `orchestration-ior`.
- Crear `AGENTS.md` cuando falta también lo decide `orchestration-ior`.
- `docs/sdlc/gates/` no entra al contrato del repo. El chequeo y la medición de cada gate pasan a mmorch; la orquestación queda en `sdlc/` (`orchestration-bpn`).
- `contract_version` pasa al ticket 03.
- El mecanismo nunca crea un archivo de constitución. El oráculo reporta "sin constitución" como aviso. El usuario declara `constitution` por repo.
- Los artefactos de corrida de `docs/sdlc/` quedan fuera del contrato. La limpieza de main es el bd `orchestration-eeq`.
- Una costumbre nueva entra con una fila en la tabla única y su chequeo determinista. Hace falta ticket propio solo si toca archivos editados a mano.
- La regla de formato entra como aviso: TOML para configuración propia, JSON con schema entre programas, YAML cuando una herramienta lo exige. El oráculo nunca convierte un archivo.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_lista_unica_de_costumbres` arma repos en `tmp_path` y verifica once puntos:

1. Cada fila de la tabla única tiene un chequeo determinista sin red, o el test falla.
2. Un repo vacío muestra un desvío por cada costumbre de SDLC, `.gitignore` de secretos y `.beads/`; el grupo SDD chequea solo repos con áreas declaradas, y cada área necesita un glob válido.
3. Tras aplicar el mecanismo, el oráculo no muestra desvíos de SDLC, SDD ni `.gitignore`.
4. El test solo verifica que el oráculo marca `.beads/`, porque ese desvío depende de `orchestration-ior`.
5. El mecanismo nunca escribe `docs/claude/`, `tests/README.md`, un archivo de constitución, áreas en `sdlc.toml` ni los artefactos de corrida de `docs/sdlc/`.
6. Una constitución ausente y un YAML fuera de la lista aparecen como avisos, y ningún aviso causa una escritura.
7. Un secreto ya trackeado aparece en el reporte, y el mecanismo no lo borra del índice ni del historial.
8. Un repo conforme, armado con el `init` actual más los arreglos, muestra cero desvíos.
9. El reporte del oráculo es JSON, con desvíos y avisos en listas separadas.
10. El exit code distingue tres estados: sin desvíos, solo avisos y con desvíos.
11. Dos corridas sobre el mismo repo dan el mismo reporte, byte a byte.

La idempotencia y las cero llamadas al LLM quedan en el ticket 03. El respeto de ediciones manuales queda en el ticket 04. Las rutas por máquina quedan en el ticket 05.
