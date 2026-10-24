# Mecanismo de actualización
Type: grilling
Status: open
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
