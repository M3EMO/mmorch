# Repos-convenciones, ticket 09 (librería de tests de propiedades): Brainstorm / Discovery Notes
Date: 2026-10-24 · Goal: decidir cómo agrega el mecanismo Hypothesis o fast-check a un repo (`.scratch/repos-convenciones/issues/09-libreria-de-propiedades.md`, bd `orchestration-4u6`).

Contexto heredado: decisión de `sdd-fase-2` (ticket 10, P3, "C con A"): Hypothesis en Python y fast-check en TypeScript; sin librería, la corrida sigue sin propiedades; `doctor` avisa; el mecanismo agrega la librería con aprobación del usuario. Choque: la lista cerrada del ticket 03 no modifica contenido existente.

Hechos (sonda de solo lectura, 2026-10-24): ningún repo tiene Hypothesis ni fast-check.
- Python con `pyproject.toml`: orchestration, mf-moe. Python con `requirements.txt`: Portfolio, Adepor (`requirements.txt` y `requirements-analisis.txt`).
- TypeScript: Estudio (`app/package.json`), ssb (`package.json`), f2-sueldo-neto y prototipos 04, 06, 10 (`package.json`, imagen `f2-sdlc`).
- ChatBot es Java (imagen `chatbot-sdlc`); la decisión de `sdd-fase-2` no cubre Java.

## Summary / key decisions

- Test de aceptación aprobado con 20 puntos (P5). Ticket 09 resuelto.
- Respaldo C en Python: buscar la carpeta `hypothesis` en `site-packages` del entorno, sin ejecutar el intérprete (ajuste de P2 anunciado en P5).
- Declarar e instalar (P4): con `uv.lock`, `uv add --optional dev hypothesis`; `pyproject.toml` sin `uv.lock`, `hypothesis` al extra `dev` y `pip` del `.venv`; `requirements.txt`, línea al final y `pip` del `.venv`; TypeScript, `npm install -D fast-check` en la carpeta del `package.json`. `tomllib` relee `pyproject.toml`; el commit incluye el archivo de dependencias y su lock; sin `.venv`, declara sin instalar y avisa.
- Repos con `contenedor`: `--aprobar` agrega a `Dockerfile.sdlc` un bloque con marcas del ticket 04 que instala fast-check en `/node_modules` de la imagen, y reconstruye la imagen con el comando de `sdlc/builder.py` (con red). Un bloque editado da aviso con diff y no se toca. `Dockerfile.sdlc` entra como fila nueva de la tabla de costumbres; este ticket cuenta como su ticket propio (regla P5 del ticket 01) (P3).
- Detección: primero el archivo de dependencias (`pyproject.toml` con dependencias, extras y grupos; cada `requirements*.txt`; `package.json` con `dependencies` y `devDependencies`; Estudio en `app/package.json`). Si no aparece, se prueba con el entorno del repo (P2).
- Mecanismo: comando aparte `python -m sdlc propiedades <repo>` que muestra el cambio y lo aplica con `--aprobar`, con commit y push según las reglas del ticket 07 (P1).

## Q&A log

### P1 — Mecanismo
- Asked: ¿A (comando aparte con `--aprobar`), B (aviso con el comando exacto) o C (arreglo nuevo de `--upgrade`)?
- Captured: el usuario eligió A.

### P2 — Detección de la librería
- Asked: ¿A (archivo de dependencias), B (lockfile) o C (importar con el intérprete del repo)?
- Captured: el usuario respondió "A y si no C".
- Interpretación de Claude (a confirmar en el test): C solo corre si A no encuentra la librería y el repo tiene entorno local (`.venv`/`venv` o `node_modules`). En TypeScript, C mira si existe `node_modules/fast-check`, sin ejecutar nada. Sin entorno, vale el resultado de A.
- Hecho para P3 (2026-10-24): los F2 no tienen dependencias (`package.json` sin `dependencies`), no tienen `node_modules`, y su imagen `f2-sdlc` (`Dockerfile.sdlc`) instala solo `typescript` global y corre sin red.

### P3 — Repos con `contenedor`
- Asked: ¿A (solo `package.json`, aviso de la imagen), B (`--aprobar` edita `Dockerfile.sdlc`) o C (no aplicar; listar los tres cambios)?
- Captured: el usuario respondió "C pero me gustaria poder trabajar en docker con este sistema, entonces b?".
- Hechos (2026-10-24): `Dockerfile.sdlc` es parte del bootstrap del builder (`sdlc/builder.py:6`, `:63`): el builder lo escribe y construye la imagen con `docker build -f Dockerfile.sdlc` (con red). `mmorch/sandbox.py` monta el worktree y corre con `-w`. El inventario del mapa no lista `Dockerfile.sdlc` como costumbre.
- Captured (final): el usuario confirma B con tres resguardos.
- Dato técnico: Node resuelve `node_modules` subiendo carpetas hasta la raíz, así que un paquete instalado en `/node_modules` de la imagen es importable desde el worktree montado.

### P4 — Declarar e instalar por formato
- Asked: ¿confirmas los cuatro formatos?
- Captured: el usuario confirma.

### P5 — Test de aceptación
- Asked: ¿apruebas el test de 14 puntos? El usuario preguntó si faltaba algo.
- Captured: Claude agregó los puntos 15 a 20 (rollback si falla el gestor o el build, estilo `==` de `requirements.txt`, archivo correcto en Adepor, idempotencia, intérprete de `_repo_python()`). El usuario aprobó.
- Hecho: Portfolio y Adepor fijan versiones con `==` (por ejemplo `requests==2.34.2`).

## Open flags (pending input)
- Ninguno.
