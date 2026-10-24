# Cómo agrega el mecanismo la librería de tests de propiedades
Type: grilling
Status: open
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
