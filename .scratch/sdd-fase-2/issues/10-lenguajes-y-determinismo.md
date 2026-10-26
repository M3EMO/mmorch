# Lenguajes, dependencia y determinismo
Type: grilling
Status: resolved
Blocked by:
Map: ../map.md

## Question

¿En qué lenguajes entran los tests de propiedades, cómo llega la dependencia al repo destino y cómo queda determinista el oráculo?

Hechos medidos el 2026-10-23:

- Hypothesis no está instalado en el venv de orchestration ni en el extra `dev` de `pyproject.toml`.
- Los comandos del SDLC corren en un contenedor sin red (`sdlc.toml contenedor`). El contenedor no instala paquetes: un paquete ausente es un error del entorno (`builder-e2e` ticket 07).
- El SDLC corre sobre repos Python, TypeScript (F2), Java (ChatBot) y otros (`_mutantes_texto`).
- Hypothesis genera entradas al azar por default. Un oráculo con azar puede dar verde y rojo en dos corridas del mismo código. `gate_suite_total` ya trata un fallo no repetido como flaky.

Opciones a decidir sobre lenguajes:

- A. Solo Python con Hypothesis. Un repo de otro lenguaje sigue el camino de hoy.
- B. Python con Hypothesis y TypeScript con fast-check. El autor del test elige la librería por `_lenguaje()`.

Opciones a decidir sobre la dependencia:

- C. `sdlc init` agrega la librería al extra de tests del repo destino y a la imagen del contenedor.
- D. El SDLC exige la librería y corta con un error del entorno si falta, sin tocar el repo.

Opciones a decidir sobre el determinismo:

- E. `derandomize=True` (o `--hypothesis-seed` fijo) dentro de los gates. La búsqueda es la misma en cada corrida.
- F. Azar con la base de ejemplos de Hypothesis persistida. Un contraejemplo hallado queda guardado y se repite.

## Test de aceptación propuesto

`tests/test_sdlc_propiedades.py::test_propiedad_determinista`: un test de propiedades que falla solo con una entrada rara. El gate corre dos veces sobre el mismo código y da el mismo veredicto las dos veces. Un repo sin la librería instalada recibe el diagnóstico de error del entorno de la etapa 4, no un fallo del código. Con la opción A, un repo TypeScript no recibe tests de propiedades y su corrida no cambia.

## Answer

Resuelto por grilling con el usuario (2026-10-24). Registro completo: `brainstorms/2026-10-24-sdd-fase-2-ticket-10.md`.

- Los tests de propiedades entran en Python con Hypothesis y en TypeScript con fast-check, desde el inicio. El autor elige la librería por lenguaje.
- La atribución por test sale de un reporte JUnit XML, con un solo parser: `pytest --junitxml`, `node --test --test-reporter=junit` y vitest. Un runner sin JUnit sigue sin atribución (enmienda al ticket 09).
- La etapa 1 detecta la librería por código. Sin librería, la corrida sigue sin propiedades y el run-log anota la falta.
- La librería se agrega al repo por el esfuerzo `orchestration-4u6`, con aprobación del usuario. `doctor` muestra la falta como aviso.
- Dentro de los gates, la semilla es fija. La semilla sale del sha del commit base y queda en el run-log.
- Semilla y perfil llegan por un archivo auxiliar ignorado por git, escrito en el worktree durante la corrida. pytest lo carga con `-p` y `node --test` con `--import`. El repo no cambia.
- El perfil apaga la base de ejemplos de Hypothesis (`database=None`), para que una corrida no dependa de la anterior. Dentro de los mutantes, el perfil corre 20 ejemplos (ticket 09).

### Test de aceptación aprobado

`tests/test_sdlc_propiedades.py::test_propiedad_determinista` usa un test de propiedades que falla solo con una entrada rara:

1. El gate corre dos veces sobre el mismo código y da el mismo veredicto.
2. Dos commits base distintos producen semillas distintas, y el run-log anota cada una.
3. El mismo commit base siempre produce la misma semilla.
4. Un repo Python sin Hypothesis sigue sin propiedades, y el run-log anota la falta.
5. Un repo TypeScript sin fast-check sigue igual, con la misma anotación.
6. Un repo TypeScript con fast-check y `node --test` recibe propiedades y atribución por JUnit.
7. El parser de JUnit lee igual la salida de pytest y la de `node --test`.
8. El archivo auxiliar no aparece en `git status` ni entra a ningún commit.
9. El perfil de 20 ejemplos se activa solo dentro de los mutantes.
10. Un runner sin salida JUnit sigue sin atribución, como dice el caso 11 del ticket 09.
11. Una corrida no deja la carpeta `.hypothesis/` en el worktree.
12. Dentro del contenedor sin red, el archivo auxiliar carga igual que en el host.
13. Sin librería, el prompt de la etapa 1 no pide propiedades. En ese caso, el gate de marcas del ticket 08 pasa sin marcas.

## Comments

- 2026-10-24, desde el esfuerzo `orchestration-4u6` (ticket 09 de `.scratch/repos-convenciones/`, resuelto; commit `b73d92d` de la rama `claude/youthful-bohr-f13d78`): la librería entra con `python -m sdlc propiedades <repo>`. Sin `--aprobar` muestra los cambios planeados; con `--aprobar` declara, instala, commitea y pushea. `doctor` da aviso, nunca desvío, cuando falta. En repos con `contenedor`, un bloque con marcas en `Dockerfile.sdlc` instala fast-check en `/node_modules` de la imagen. ChatBot (Java) queda fuera.
