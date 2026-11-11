# Brief de la Tarea 96 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-95` (d4e6722). Las Tareas 84 a 95 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-96`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el comando de nextest | `src/ficha.rs` |
| 2, 100 o 101 sin junit | `src/oracle/mod.rs` |
| 3, frase y comentario | `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra.rs` |

- Los tres puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**El HIGH de la revisión de la Tarea 95.** `cmd_nextest` (`src/ficha.rs:393`) termina así:

```
s=$?; cp /salida/target/nextest/ci/junit.xml /salida/<xml>.xml || exit 1; exit $s
```

- Si el build se rompe, nextest no escribe junit.
- Con `/salida/target` limpio, el `cp` falla, y el comando sale con 1, no con 101.
- La regla del 101 de la Tarea 95 (`MOTIVO_NO_COMPILA`, `src/oracle/mod.rs:2074`) nunca llega en una evaluación real. El caso cae en infraestructura.
- En la calibración de t1 y t2 sí llegó 101, porque quedaba un junit viejo de la corrida base.

**El mismo `cp` entrega junit viejos.** Si nextest falla antes de escribir, el `cp` copia el junit de la corrida anterior, que habla de otro código.

**Un producto puede borrar su junit.** Hoy, con 100 y sin junit, el resultado es infraestructura. Un producto que borra el junit después de fallar consigue "sin veredicto" en vez de rojo.

**Frase y comentario.** El motivo "ninguna línea del cambio admite este defecto" se usa también para `modulo_borrado`, que no tiene línea. `en_cambio` significa "línea tocada" en `comportamiento_roto` y "archivo tocado" en `modulo_borrado`, y su comentario no lo dice.

## Tarea

La regla que manda: **el código de nextest llega entero, y un junit nunca habla de otra corrida.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El comando de nextest

1. `cmd_nextest` borra el junit viejo antes de correr: `rm -f /salida/target/nextest/ci/junit.xml`.
2. Después del `cp`, el comando sale con el código de nextest, aunque el `cp` falle. La forma es: `s=$?; cp /salida/target/nextest/ci/junit.xml /salida/<xml>.xml 2>/dev/null; exit $s`.
3. Actualizá el comentario de `cmd_nextest`.

**Tests en rojo primero:**
- Unitario: el comando trae el `rm -f` antes de `cargo nextest run`.
- Unitario: el comando no trae `|| exit 1` después del `cp`, y termina con `exit $s`.
- Unitario, con `sh` del host y un `cargo` falso en el `PATH` del test: un `cargo` que sale con 101 sin escribir junit hace que el comando salga con 101. Armá el falso dentro del árbol del test, no en `/tmp`.
- Guarda: un `cargo` falso que sale con 0 y escribe junit deja el junit copiado, y el comando sale con 0.

### 2. 100 o 101 sin junit

1. Solo con `runner = "nextest"`.
2. **101 sin junit:** "la suite no compila", como ya hace la Tarea 95.
3. **100 sin junit:** "la suite falló y no dejó junit". Cada test esperado cuenta como fallo.
4. **0 sin junit, o cualquier otro código sin junit:** infraestructura, como hoy (`sin_junit`, `src/oracle/mod.rs:2689`).
5. Con otro runner, nada cambia.

**Tests en rojo primero:**
- Unitario: con nextest, código 100 y sin junit, cada test esperado es un fallo, con su motivo.
- Unitario: con nextest, código 0 y sin junit, el resultado es infraestructura.
- Guarda: con nextest, código 101 y sin junit, el resultado de la Tarea 95 no cambia.
- Guarda: con pytest, código 1 y sin junit, el resultado es el de hoy.

### 3. Frase y comentario

1. El motivo pasa a "ningún lugar del cambio admite este defecto".
2. El comentario de `en_cambio` dice qué significa en cada defecto: línea tocada en `comportamiento_roto`, archivo tocado en `modulo_borrado`.

**Tests:** actualizá los tests que fijan la frase vieja. La guarda es la suite verde.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

**Después de mezclar, lo hace el orquestador, no vos:** cambiar `contrato.cmd` y `oculto.cmd` en los `tarea.toml` de s2-t1 a s2-t3 a la forma nueva, y actualizar `armar_s2.py`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-96.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
