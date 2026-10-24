# Mecanismo de actualización
Type: grilling
Status: resolved
Blocked by: 01
Map: ../map.md

## Question

¿Con qué mecanismo un repo adoptado converge a las convenciones vigentes?

Hoy `init()` es aditivo en parte: copia las plantillas que faltan en cada corrida. Nunca agrega claves a `sdlc.toml` y nunca actualiza una plantilla distinta. El ticket 11 de `sdlc-6-gates` decidió un `contract_version` que el pipeline rechaza si no lo entiende, pero el código no lo tiene.

Opciones a decidir:

- A. Convergencia idempotente: `python -m sdlc init --upgrade <repo>` compara el repo con el estado deseado y aplica solo lo que falta. Correr dos veces no cambia nada la segunda vez.
- B. Migraciones versionadas: `sdlc.toml` guarda `contract_version`. Cada salto de versión es una función con test. El mecanismo corre las migraciones pendientes en orden.
- C. Solo diagnóstico: `python -m sdlc doctor <repo>` lista cada desvío con su arreglo. El usuario aplica a mano.
- D. Diagnóstico más convergencia: `doctor` es el oráculo, y `--upgrade` aplica solo lo que `doctor` marca como seguro.

Preguntas derivadas:

- ¿`sdlc.toml` guarda `contract_version`, como decidió el ticket 11 de `sdlc-6-gates`? El ticket 01 de este mapa pasó la pregunta acá (2026-10-23). Hoy ningún código la lee.
- ¿El pipeline rechaza correr sobre un repo desactualizado, avisa o sigue?
- ¿Una clave nueva con default en código (por ejemplo `cobertura_min`) se escribe en el `sdlc.toml`, o el default alcanza?
- ¿Una clave retirada (por ejemplo `docs/sdlc/sdlc.toml` legacy) se borra, se marca o se deja?
- ¿El mecanismo vive en `sdlc/` (builder) o en `mmorch/` (banco)? La memoria `sdlc-es-builder` dice que el SDLC construye y mmorch mide.

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_actualizar_converge`: en `tmp_path` se arma un repo con el `sdlc.toml` y el `docs/sdlc/` que dejó el `init` de septiembre (6 claves, sin `refutar-tests.md`). El test verifica cuatro cosas:

1. El oráculo de conformidad marca cada desvío por nombre.
2. Tras aplicar el mecanismo, el oráculo no marca ningún desvío.
3. Una segunda aplicación deja `git status --porcelain` vacío.
4. Ninguna aplicación llama a un LLM (`llm` simulado, cero llamadas).

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-repos-convenciones-ticket-03.md`.

- El mecanismo es la opción D con convergencia. `python -m sdlc doctor <repo>` es el oráculo del ticket 01 y nunca escribe.
- `python -m sdlc init --upgrade <repo>` aplica solo los arreglos seguros. El estado deseado es la tabla única del ticket 01; no hay lista de migraciones.
- `contract_version` se retira. `doctor` marca como aviso cada clave desconocida de `sdlc.toml`.
- El pipeline corre `doctor` al inicio, guarda el reporte en `run-log.json` y sigue. Ningún desvío de `doctor` frena una corrida.
- `--upgrade` nunca escribe una clave con default en código. `doctor` marca solo la ausencia de claves sin default (`accept_cmd`, `files`).
- Una costumbre retirada es un desvío de `doctor`, y `--upgrade` nunca borra archivos del usuario. `_cfg()` deja de leer `docs/sdlc/sdlc.toml`.
- La tabla, `doctor` y `--upgrade` viven en `sdlc/` (por ejemplo `sdlc/convenciones.py`), junto a `init()`.
- `--upgrade` nunca crea `sdlc.toml`, porque adoptar un repo es una decisión del usuario con `init`.

### Arreglos seguros (lista cerrada)

- Crear un archivo que falta.
- Agregar una clave sin default que falta en `sdlc.toml`, antes del primer encabezado de tabla. Sin valor adivinable, la clave va comentada con ayuda.
- Agregar un patrón que falta al final de `.gitignore`.
- Agregar un bloque con marca propia al final de un archivo, si la marca no existe.
- Correr el comando de `orchestration-ior` cuando falta `.beads/`, recién con su test verde.
- Actualizar una plantilla intacta y vieja, solo si el ticket 04 aprueba cómo detectar que nadie la editó.

Resguardos de cada escritura: escritura atómica; `tomllib` relee `sdlc.toml` y una lectura fallida deshace la escritura; `doctor` corre al final y el desvío arreglado desaparece sin desvíos nuevos.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_actualizar_converge` arma en `tmp_path` un repo con el `init` de septiembre (6 claves, sin `refutar-tests.md`) y verifica quince puntos:

1. `doctor` marca cada desvío por nombre en su reporte JSON.
2. Tras `--upgrade`, solo quedan desvíos de `.beads/` y de claves sin valor adivinable.
3. Una segunda corrida de `--upgrade` deja `git status --porcelain` vacío.
4. Ninguna corrida llama a un LLM, con `llm` simulado y cero llamadas.
5. Cada línea previa sigue presente y en el mismo orden, con sus bytes salvo el salto de línea final.
6. En un `sdlc.toml` que termina con `[sprites]`, la clave agregada queda en la raíz según `tomllib`.
7. En un repo sin `sdlc.toml`, `--upgrade` no escribe nada y sale con un código propio.
8. Una clave desconocida en `sdlc.toml` aparece como aviso y queda intacta.
9. `--upgrade` no escribe ninguna clave con default en código.
10. Un `docs/sdlc/sdlc.toml` presente es un desvío, `_cfg()` lo ignora y `--upgrade` no lo borra.
11. Si `tomllib` falla al releer `sdlc.toml`, el archivo vuelve a sus bytes originales.
12. `build_feature` guarda el reporte de `doctor` en `run-log.json` y sigue con desvíos presentes.
13. En un repo conforme, `--upgrade` no escribe ningún archivo.
14. Si `doctor` lanza una excepción, `build_feature` guarda el error en `run-log.json` y sigue.
15. Tras `--upgrade`, ningún archivo temporal queda en el repo.
