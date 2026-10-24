# Cómo no pisar lo que el usuario editó a mano
Type: grilling
Status: claimed
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
