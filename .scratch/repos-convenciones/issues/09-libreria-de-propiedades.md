# Cómo agrega el mecanismo la librería de tests de propiedades
Type: grilling
Status: resolved
Blocked by:
Map: ../map.md

## Question

¿Cómo agrega el mecanismo Hypothesis o fast-check a un repo, si la lista cerrada del ticket 03 no modifica contenido existente?

Decisión recibida de la sesión de SDD fase 2 (2026-10-24, ticket 10 de `sdd-fase-2`, P3, `brainstorms/2026-10-24-sdd-fase-2-ticket-10.md`, rama `claude/gifted-bartik-c4a833`). El usuario eligió "C con A":

- El SDLC escribe tests de propiedades en Python (Hypothesis) y en TypeScript (fast-check).
- Si la librería falta en el repo, la corrida sigue sin propiedades y el run-log lo anota. La falta nunca corta la corrida.
- La adopción va por este esfuerzo. `doctor` muestra la falta como aviso.
- El mecanismo agrega la librería al repo con aprobación del usuario: extra de tests en Python, devDependency en TypeScript, imagen del contenedor.

Choque con decisiones de este mapa:

- El ticket 03 fija una lista cerrada de arreglos seguros, y ninguno modifica contenido existente. Agregar una dependencia modifica `pyproject.toml` o `package.json`.
- El ticket 03 dice que `--upgrade` aplica solo arreglos seguros sin preguntar. Esta decisión pide aprobación del usuario.
- Un repo con `contenedor` necesita la librería dentro de la imagen, y el mecanismo no construye imágenes.

Opciones a decidir:

- A. Comando aparte con aprobación, como `areas` del ticket 08: propone el cambio en un archivo y aplica con `--aprobar`.
- B. `doctor` da el aviso con el comando exacto (`uv add --optional dev hypothesis`, `npm i -D fast-check`), y el usuario lo corre.
- C. Arreglo nuevo de la lista cerrada, solo si el archivo de dependencias tiene la sección esperada.

Preguntas derivadas:

- ¿Cómo detecta `doctor` la falta: por el archivo de dependencias, por el lockfile, o importando la librería con el intérprete del repo?
- ¿Qué pasa con un repo sin `pyproject.toml` (por ejemplo, `requirements.txt` en Adepor)?
- ¿La imagen del contenedor queda fuera del mecanismo, con un aviso que nombra la imagen?

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_libreria_de_propiedades`: en `tmp_path`, un repo Python sin Hypothesis da un aviso de `doctor` que nombra la librería, y un repo TypeScript sin fast-check da el aviso equivalente. Ningún aviso cuenta como desvío. Con la opción elegida aplicada y aprobada, el aviso desaparece, y los bytes previos del archivo de dependencias siguen iguales salvo la línea agregada.

## Answer

Resuelto por grilling con el usuario (2026-10-24). Registro completo: `brainstorms/2026-10-24-repos-convenciones-ticket-09.md`.

- La librería entra con un comando aparte, `python -m sdlc propiedades <repo>`. Sin `--aprobar`, el comando muestra los cambios planeados y no escribe nada.
- `--aprobar` declara, instala, commitea y pushea con las reglas del ticket 07. Un agente pide OK antes de correrlo.
- `doctor` busca la librería primero en el archivo de dependencias. Si no aparece, mira el entorno local: `site-packages` del `.venv` o `node_modules/fast-check`, sin ejecutar nada.
- La falta de la librería es aviso, nunca desvío. ChatBot (Java) queda fuera, porque la decisión de `sdd-fase-2` cubre solo Python y TypeScript.
- Con `uv.lock`, `--aprobar` corre `uv add --optional dev hypothesis`.
- Con `pyproject.toml` sin `uv.lock`, `--aprobar` suma `hypothesis` al extra `dev` e instala con el `pip` del entorno.
- Con `requirements.txt`, `--aprobar` suma una línea al final, con `==` si el archivo fija versiones.
- En TypeScript, `--aprobar` corre `npm install -D fast-check` en la carpeta del `package.json`.
- Con `contenedor`, `--aprobar` agrega a `Dockerfile.sdlc` un bloque con marcas del ticket 04. Ese bloque instala fast-check en `/node_modules` de la imagen, y la imagen se reconstruye con el comando de `sdlc/builder.py`.
- `Dockerfile.sdlc` entra como fila nueva de la tabla de costumbres. Este ticket cuenta como su ticket propio, por la regla de P5 del ticket 01.
- Sin entorno local, `--aprobar` declara la librería, no la instala y da un aviso.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_libreria_de_propiedades` arma repos en `tmp_path`, con el gestor de paquetes y `docker build` simulados, y verifica veinte puntos:

1. Un repo Python sin Hypothesis da un aviso que nombra la librería, y nunca un desvío.
2. Un repo TypeScript sin fast-check da el aviso equivalente.
3. Hypothesis declarado en el extra `dev`, en un grupo de desarrollo o en un `requirements*.txt` no da aviso.
4. Hypothesis sin declarar pero presente en `site-packages` del `.venv` no da aviso.
5. Un repo Java no da aviso.
6. `propiedades <repo>` sin `--aprobar` muestra los comandos planeados y no escribe nada.
7. Con `uv.lock`, `--aprobar` corre `uv add --optional dev hypothesis`.
8. Sin `uv.lock`, el extra `dev` suma `hypothesis`, el resto de `pyproject.toml` queda byte a byte igual y `tomllib` lo lee.
9. Con `requirements.txt`, la línea nueva queda al final, aunque el archivo no termine en salto de línea.
10. En Estudio, `npm install -D fast-check` corre en `app/`.
11. Con `contenedor`, `Dockerfile.sdlc` suma el bloque con marcas, y el build usa el comando de `sdlc/builder.py`.
12. Un bloque editado en `Dockerfile.sdlc` da un aviso con diff y queda intacto.
13. Sin `.venv`, la librería queda declarada, `pip` no corre y el reporte da un aviso.
14. El commit incluye el archivo de dependencias, su lock y `Dockerfile.sdlc`, y sigue las reglas del ticket 07.
15. Un gestor de paquetes que falla deja cada archivo con sus bytes originales y sin commit.
16. Si el build de la imagen falla, `Dockerfile.sdlc` vuelve a sus bytes originales y el reporte lo dice.
17. La línea nueva de `requirements.txt` usa `==` y la versión instalada si las otras líneas usan `==`.
18. En Adepor, la línea va a `requirements.txt` y no a `requirements-analisis.txt`.
19. Una segunda corrida de `--aprobar` con la librería ya presente no escribe ni commitea nada.
20. `--aprobar` elige el intérprete con la misma regla que `_repo_python()` del pipeline.
