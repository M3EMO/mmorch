# Cómo no pisar lo que el usuario editó a mano
Type: grilling
Status: resolved
Blocked by: 03
Map: ../map.md

## Question

¿Cómo distingue el mecanismo un archivo que el usuario editó de un archivo viejo sin tocar?

Hay tres casos con riesgos distintos:

- `sdlc.toml`: el usuario cura `files`, `accept_cmd` y comentarios propios (orchestration tiene "Techo curado 2026-09-15"). `tomllib` de la stdlib solo lee y no preserva comentarios al escribir. El repo no tiene `tomlkit` ni `tomli_w`.
- Plantillas de `docs/sdlc/`: hoy todas las copias sondeadas son idénticas a `sdlc/templates/`. Una plantilla nueva en `sdlc/` no llega a un repo que ya tiene la vieja.
- `AGENTS.md`: el puntero es un bloque con marca; el resto del archivo es del usuario.

Opciones a decidir:

- A. Huella de la copia: el mecanismo guarda el hash de cada archivo que escribió (por ejemplo en `docs/sdlc/.convenciones.json`). Si el hash actual coincide, el archivo es intacto y se actualiza; si no coincide, el archivo es del usuario y no se toca.
- B. Archivo al lado: una plantilla distinta recibe la versión nueva como `<nombre>.nuevo.md` y el usuario fusiona.
- C. Solo agregar: el mecanismo agrega claves al final de `sdlc.toml` como texto y nunca reescribe una clave existente. Las plantillas distintas se reportan sin tocarse.
- D. Bloques con marca: el mecanismo solo reescribe texto entre marcas propias (como el puntero de `AGENTS.md`), y todo lo demás es del usuario.

Preguntas derivadas:

- ¿Un archivo que el usuario editó y que además quedó viejo se reporta como conflicto, o se deja en silencio?
- ¿Agregar una dependencia (`tomlkit`) para reescribir TOML con comentarios vale el costo?
- ¿El mecanismo trabaja en una rama propia del repo destino, o en el árbol actual?

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_respeta_ediciones`: en `tmp_path` se arma un repo adoptado con tres ediciones del usuario: un comentario y un `files` curado en `sdlc.toml`, una línea agregada a `spec-review.md` y un párrafo propio en `AGENTS.md`. Tras aplicar el mecanismo:

1. Cada byte editado por el usuario sigue igual.
2. Una plantilla intacta y vieja queda igual a `sdlc/templates/`.
3. La plantilla editada y vieja aparece en el reporte como conflicto con su ruta.
4. `tomllib` lee el `sdlc.toml` resultante sin error y con las claves previas intactas.

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-repos-convenciones-ticket-04.md`.

- El ticket 03 achicó este ticket: `--upgrade` no modifica contenido existente, salvo plantillas intactas y bloques con marca.
- Plantilla intacta: `sdlc/` guarda una lista con el hash de cada versión publicada de cada plantilla. Una copia igual a una versión publicada está intacta, y `--upgrade` la reemplaza.
- Un test exige el hash de cada plantilla actual en la lista. La lista solo crece, y nadie borra un hash viejo.
- Plantilla editada: `doctor` da un aviso con la ruta y un diff (`difflib`) contra la plantilla actual. Nunca es desvío, y nada escribe archivos al lado.
- Bloques: formato de `orchestration-ior` (commit `0073a83`), `<!-- BEGIN SDLC <nombre> v:<n> hash:<h> -->` ... `<!-- END SDLC <nombre> -->`, con `h = sha256(interior.strip())[:8]`.
- Un bloque con hash correcto y `v:` viejo recibe el interior actual. Un interior que no coincide con su hash da un aviso con diff.
- El puntero `## SDLC (pipeline de 6 etapas)` no tiene cierre, así que da un aviso y nadie lo reescribe.
- La línea `@AGENTS.md` de `CLAUDE.md` va sin marcas. Si falta, `--upgrade` la agrega al inicio (enmienda del ticket 03).
- El hash se calcula sobre texto con saltos LF, y la escritura conserva el estilo de salto del archivo.
- `tomlkit` no hace falta: el ticket 03 inserta texto y verifica con `tomllib`.
- La elección entre rama propia y árbol actual pasa al ticket 07.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_respeta_ediciones` arma en `tmp_path` un repo adoptado con ediciones del usuario y verifica trece puntos:

1. Un comentario y un `files` curado en `sdlc.toml` siguen byte a byte iguales.
2. Un párrafo del usuario en `AGENTS.md`, fuera de todo bloque, sigue byte a byte igual.
3. Una plantilla igual a una versión publicada vieja queda igual a la versión actual.
4. Una plantilla editada queda intacta, y `doctor` da un aviso con su ruta y un diff.
5. Un bloque con `v:` viejo y hash correcto queda con el interior actual y la marca nueva.
6. Un bloque cuyo interior no coincide con su hash queda intacto, y `doctor` da un aviso con diff.
7. El puntero del SDLC sin marca de cierre queda intacto, y `doctor` da un aviso.
8. Un `CLAUDE.md` sin `@AGENTS.md` recibe la línea al inicio, y el resto queda byte a byte igual.
9. Un repo con solo plantillas o bloques editados sale con el exit code de "solo avisos".
10. `tomllib` lee el `sdlc.toml` resultante con las claves previas intactas.
11. Un bloque con `v:` mayor al publicado y una plantilla con hash ausente de la lista quedan intactos, con aviso.
12. Una copia CRLF intacta cuenta como intacta y sigue en CRLF tras `--upgrade`.
13. Un `BEGIN` sin `END` o un nombre de bloque duplicado deja el archivo intacto, con un aviso que nombra la línea.

`tests/test_sdlc_convenciones.py::test_lista_de_hashes` exige el hash de cada plantilla actual de `sdlc/templates/` en la lista.
