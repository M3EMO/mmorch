# Brief de la Tarea 126 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-125`. Trae las Tareas 84 a 125.
- La 127 se construye a la vez. Toca `src/oracle/mutacion.rs` y `src/oracle/calibra.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-126`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el helper único | `src/prueba_ejecutable.rs` (nuevo, solo tests), `src/lib.rs` (solo la línea `#[cfg(test)] mod`) |
| 2, los helpers de `src/` | Los módulos de tests de `src/corre/ejecutor.rs`, `src/corre/corrida.rs`, `src/limpieza.rs`, `src/ficha.rs`, `src/oracle/colab.rs` y `src/oracle/pendientes.rs` |
| 3, los helpers de `tests/` | `tests/comun/ejecutable.rs` (nuevo), `tests/colab.rs`, `tests/corre.rs`, `tests/entrega.rs`, `tests/test_seguro.rs` |
| 4, la aserción que falló | El módulo de tests de `src/corre/ejecutor.rs` |

- El punto 1 va primero. Después, los puntos 2 y 3 corren en paralelo. El punto 4 comparte `ejecutor.rs` con el 2, y va después del 2.
- **Solo tests.** Ningún punto cambia código de producción.
- **No toques:** `src/oracle/mutacion.rs`, `src/oracle/calibra.rs`, el código de producción de cualquier archivo, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**`vencimiento_sigue_borrando_el_contenedor_por_nombre` falló tres veces en suites completas.** Falló en la suite con podman de la cadena 110 a 125, en un hook de la 121 y en una corrida de la 125. No había builders. Solo, pasa 3 de 3. Falla en `src/corre/ejecutor.rs:1567` con "no pidió borrar por nombre".

**La causa es la carrera ETXTBSY.**
1. `programa_falso` (`src/corre/ejecutor.rs:1368`) escribe un script y le da permisos. No lo prueba antes de devolverlo.
2. Mientras se escribe el script, otro hilo de la suite puede hacer fork. El hijo hereda el descriptor de escritura, y lo cierra recién en su `exec`.
3. El test lanza el script por `cortar_vencido` → `borrar_contenedor` → `correr_paso_de_quitar`. El kernel da ETXTBSY, el código 26 de Linux.
4. `borrar_contenedor` devuelve error, el script nunca corre, y el log queda vacío.
5. `serie_de_pruebas` solo ordena los tests de `ejecutor.rs` entre sí. Los forks de otros módulos siguen.

**El arreglo ya existe, pero repetido.** La 120 lo puso en `src/limpieza.rs`. La 122 lo puso en `src/corre/corrida.rs:3827` y en `tests/corre.rs`. Cada helper prueba su script con `--sonda`, con reintentos ante el código 26.

**Quedan muchos helpers sin el arreglo.** `grep -n "from_mode(0o7"` en `src/` y `tests/` da 20 lugares. Algunos son carpetas, no ejecutables: `src/oracle/mod.rs:677` y `src/oracle/perfiles/herramientas.rs:571`. Los demás escriben un ejecutable de tests.

## Tarea

La regla que manda: **un test que escribe un ejecutable lo recibe ya probado, de un solo helper.** Hacé un commit por punto. Cada punto empieza con su test en rojo, si se puede reproducir.

### 1. El helper único

1. **`src/prueba_ejecutable.rs`,** con `#[cfg(test)]`, ofrece `escribir_ejecutable(ruta, contenido) -> PathBuf`.
2. **Escribe el archivo** con permisos `0o755`.
3. **Si el contenido empieza con `#!/bin/sh`,** agrega después de esa línea: `[ "$ROOT_SONDA_EJECUTABLE" = 1 ] && exit 0`.
4. **Prueba el ejecutable** con la variable `ROOT_SONDA_EJECUTABLE=1`, sin argumentos. Con el código 26, espera 10 ms y prueba otra vez, hasta 50 veces. Un comentario explica el 26.
5. **Un binario copiado,** que no es un script de `sh`, se prueba igual, pero solo hasta que el `exec` no dé 26. Su código de salida no importa.
6. Si sigue con el código 26 tras 50 intentos, el helper entra en pánico con un mensaje claro.

**Tests en rojo primero:**
- Unitario: un script con la sonda sale con 0 y no deja marcas.
- Unitario: 200 repeticiones del helper, mientras otro hilo hace fork en bucle, no fallan. Si el rojo no se reproduce, anotalo, y dejá la repetición como guarda.

### 2. Los helpers de `src/`

1. Cada helper de tests de la tabla que escribe un ejecutable usa `escribir_ejecutable`.
2. Los helpers con su propia sonda `--sonda`, de la 120 y la 122, pasan al helper único. Borrá la sonda vieja después de unir sus llamadores.
3. Si un script no empieza con `#!/bin/sh`, anotalo en el informe con su decisión.
4. El informe lista cada lugar cambiado.

**Tests en rojo primero:**
- La guarda del punto 1 cubre la carrera. Cada test que usa un helper cambiado sigue verde.

### 3. Los helpers de `tests/`

1. `tests/comun/ejecutable.rs` copia el helper del punto 1. `tests/` no ve los módulos de tests de `src/`. Un comentario nombra el original.
2. Cada helper de `tests/` de la tabla lo usa. Si un archivo ya tiene un `mod` común, seguí su forma.

**Tests en rojo primero:**
- Igual que el punto 2.

### 4. La aserción que falló

1. La primera aserción de `vencimiento_sigue_borrando_el_contenedor_por_nombre` muestra también el mensaje de `cortar_vencido`. Así, si vuelve a fallar, se ve la causa.

**Tests en rojo primero:**
- No hace falta. Es un cambio del texto de la aserción. Anotalo como guarda.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

Ninguno nuevo. La prueba de la tarea es la suite completa. El orquestador corre la suite con podman de la cadena con la 126 dos veces seguidas. `vencimiento_sigue_borrando_el_contenedor_por_nombre` no debe fallar.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-126.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los lugares cambiados**: cada helper, su archivo y su línea, y si era un script o un binario.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada helper borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
