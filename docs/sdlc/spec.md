# Spec: Anclaje de STORE_PATH via mmorch.paths.repo_root

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.
Cada requisito lleva un ID `R<n>` unico. El gate `trazabilidad` cruza esos IDs con `plan.md` y con los tests.

## Contrato

- Archivo modificado: `mmorch/synth_store.py`.
- Clases: ninguna.
- Metodos: ninguno.
- Import a nivel de modulo: `from .paths import repo_root`.
- Constante a nivel de modulo: `STORE_PATH = repo_root() / "synth_checkers.json"`.
- Tipo de `STORE_PATH`: `pathlib.Path` (resultado de `repo_root() / str`).
- No se modifican `mmorch/paths.py`, `tests/test_paths.py` ni ningun otro archivo.
- El archivo de estado `synth_checkers.json` permanece versionado en la raiz del repositorio.

## Requisitos

- R1: Al importar `mmorch.synth_store`, primero se importa `repo_root` desde `mmorch.paths`; luego, en tiempo de import, se evalua `repo_root()` y se define `STORE_PATH` como `repo_root() / "synth_checkers.json"`.
- R2: `mmorch/synth_store.py` no contiene ninguna ocurrencia que case con la expresion regular `__file__.*(parents\[1\]|parent\.parent)`, ni en codigo, ni en comentarios, ni en strings.
- R3: El cambio no altera la resolucion de `mmorch.paths`: sin `MMORCH_HOME`, `home()` es la raiz del checkout; con `MMORCH_HOME`, `home()` y los logs de `metrics`/`feedback` caen dentro de `MMORCH_HOME`.
- R4: La importacion de `mmorch.synth_store` es valida en Python 3.12 sin dependencias externas y no realiza escrituras de estado en tiempo de import.

## Casos borde

- R1: `MMORCH_HOME` no seteado: `repo_root()` resuelve a la raiz del checkout.
- R1: `MMORCH_HOME` seteado a un directorio temporal: `STORE_PATH` sigue apuntando a `synth_checkers.json` en la raiz del checkout, no al directorio temporal.
- R2: La aguja prohibida aparece en un comentario o string: cuenta como ofensa porque el gate lee el archivo completo.
- R2: El archivo no contiene la aguja prohibida: no integra la lista de ofensores del gate.
- R3: El repositorio se importa desde un `cwd` distinto: `STORE_PATH` se resuelve por paquete, no por `cwd`.
- R4: Import en subprocess con `MMORCH_HOME` ausente o presente: termina con codigo 0 y sin efectos secundarios de escritura.

## Criterios de exito

- R1: `python -c "import mmorch.synth_store as s; print(s.STORE_PATH)"` ejecutado en la raiz del repo imprime la ruta absoluta de `synth_checkers.json` dentro de la raiz del checkout.
- R2: `grep -E "__file__.*(parents\\[1\\]|parent\\.parent)" mmorch/synth_store.py` no produce coincidencias.
- R3: `MMORCH_HOME=/tmp/mmorch_test python -c "import mmorch.synth_store as s; print(s.STORE_PATH)"` imprime la ruta absoluta de `synth_checkers.json` en la raiz del checkout, no en `/tmp/mmorch_test`.
- R4: `python -c "import mmorch.synth_store"` termina con codigo 0 y no crea ni modifica `synth_checkers.json`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_gate_sin_anclas_de_estado_fuera_de_paths, test_sin_env_default_es_el_checkout |
| R2 | test_gate_sin_anclas_de_estado_fuera_de_paths |
| R3 | test_estado_va_a_mmorch_home, test_sin_env_default_es_el_checkout |
| R4 | test_gate_sin_anclas_de_estado_fuera_de_paths |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.