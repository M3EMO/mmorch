# Spec: Hermeticidad de `_codegraph_context` (sin autoindex sorpresivo)

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.
Cada requisito lleva un ID `R<n>` unico. El gate `trazabilidad` cruza esos IDs con `plan.md` y con los tests.

## Contrato

Archivo modificado (unico): `mmorch/project_loop.py`. No se tocan tests ni otros archivos. El docstring del modulo se conserva.

Funcion publica bajo prueba:

```python
def _codegraph_context(repo: str, tarea: str) -> str: ...
```

- `repo`: path del repositorio, absoluto o relativo; se normaliza con `Path(repo).resolve()`.
- `tarea`: texto de la tarea; no participa de las reglas R1-R3.
- Retorno: `str`; `""` cuando no se indexa, texto de contexto cuando `sync`/`index` corren.

Helper invocado (firma exacta observada por los tests, ya existente en el modulo):

```python
def _cg(bin_: str, repo: str, sub: str, *, arg: str = "", timeout: float = 60.0) -> Any: ...
```

- Retorno: objeto con atributos `returncode: int`, `stdout: str`, `stderr: str`.
- `sub`: subcomando (`"init"`, `"index"`, `"sync"`).

Variables de entorno:

- `MMORCH_CODEGRAPH_AUTOINDEX`: leida con `os.environ.get("MMORCH_CODEGRAPH_AUTOINDEX", "0")`; solo el valor exacto `"1"` habilita el autoindex.
- `CODEGRAPH_BIN`: binario del indexador; su resolucion via `shutil.which` no cambia.

Docstring de `_codegraph_context`: debe declarar que el default de `MMORCH_CODEGRAPH_AUTOINDEX` es `0` (sin opt-in no se corre `init`/`index`) y que un repo bajo `tempfile.gettempdir()` no se indexa ni con opt-in.

## Requisitos

Orden de evaluacion unico y comun a R1-R3: (1) resolver `repo`; (2) si existe `<repo>/.codegraph`, ir al camino R3; (3) si no existe, leer `MMORCH_CODEGRAPH_AUTOINDEX`; (4) si el valor no es `"1"`, R1; (5) si es `"1"`, evaluar `tempfile.gettempdir()`, R2; (6) solo si nada de lo anterior corta, correr `init` e `index`.

- R1: si `<repo>/.codegraph` no existe y `MMORCH_CODEGRAPH_AUTOINDEX` no vale `"1"` (ausente, `""`, `"0"`, cualquier otro valor), la funcion devuelve `""` sin invocar `_cg` ni `shutil.which`, sin ejecutar `init` ni `index`, y sin crear ni modificar nada dentro del repo (en particular, no crea `.codegraph`). El default de la variable pasa de `'1'` a `'0'`.
- R2: si `<repo>/.codegraph` no existe y `MMORCH_CODEGRAPH_AUTOINDEX == "1"`, pero el path resuelto de `repo` cae dentro del path resuelto de `tempfile.gettempdir()` (comparacion `is_relative_to` sobre paths resueltos), la funcion devuelve `""` sin invocar `_cg`.
- R3: si `<repo>/.codegraph` ya existe, la funcion se comporta como hoy: la primera invocacion a `_cg` es con `sub == "sync"` y el resto de las llamadas (resolucion de binario, armado del contexto) sigue sin cambios; R1 y R2 no aplican en este camino.

## Casos borde

- R1: repo con archivos `.py` pero sin `.codegraph`, `MMORCH_CODEGRAPH_AUTOINDEX` ausente del entorno y `CODEGRAPH_BIN` ausente: no se toca el repo y el retorno es `""`.
- R1: `MMORCH_CODEGRAPH_AUTOINDEX` presente con valores distintos de `"1"` (`""`, `"0"`, `"true"`, `"yes"`, `"01"`) tratados como sin opt-in.
- R1: repo leido y no escrito: la ausencia de `.codegraph` se verifica despues de la llamada.
- R2: repo creado en un subdirectorio inmediato del tempdir del sistema; tambien el propio tempdir raiz y paths con symlinks o componentes `..` que resuelven dentro del tempdir.
- R2: `repo` pasado como path relativo que resuelve dentro del tempdir: la comparacion usa paths resueltos, no strings.
- R2: opt-in y repo temporal -> cero llamadas a `_cg`, ni siquiera `sync`.
- R3: `.codegraph` presente como directorio vacio: `sync` se invoca igual.
- R3: `.codegraph` presente y opt-in ausente: el camino R3 prevalece (no se aplica R1).

## Criterios de exito

- R1: `repo` sin `.codegraph`, `MMORCH_CODEGRAPH_AUTOINDEX` ausente, `shutil.which` y `_cg` espiados -> retorno `""`, `calls == []`, `(repo / ".codegraph").exists() is False`.
- R1: `MMORCH_CODEGRAPH_AUTOINDEX == "0"` con repo sin `.codegraph` -> retorno `""`, `calls == []`.
- R2: `MMORCH_CODEGRAPH_AUTOINDEX == "1"`, `CODEGRAPH_BIN` seteado, `repo = tempfile.mkdtemp() / "repo"` creado -> retorno `""`, `calls == []`.
- R3: `CODEGRAPH_BIN` seteado, `repo / ".codegraph"` creado -> `calls` no vacio y `calls[0] == "sync"`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_sin_indice_previo_no_indexa_sin_opt_in |
| R2 | test_R2_con_opt_in_pero_repo_temporal_no_indexa |
| R3 | test_R3_con_indice_presente_sincroniza |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.