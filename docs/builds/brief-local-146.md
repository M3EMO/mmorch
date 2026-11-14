# Brief de la Tarea 146 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-145`. Trae las Tareas 84 a 145 y la 127b.
- **Arrancá cuando el orquestador termine la calibración de Eficiencia.** Esa calibración necesita la laptop sin otro trabajo.
- La 147 se construye a la vez. Toca `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra.rs`, `src/oracle/eficiencia.rs` y `src/colab_calibra.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-146`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el mensaje de plazo con su motor | `src/oracle/mod.rs`, `src/oracle/contenedor.rs`, `src/oracle/descarga.rs` |
| 2, `--n` y `--semilla` de `colab empaquetar` | `src/cli.rs`, `src/lib.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 147, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La Tarea 145 dejó dos puntos sin hacer,** porque su brief prohibía los archivos que piden. Ahora ningún builder corre a la vez, y esta tarea los puede tocar.

**1. El mensaje de plazo.** En modo `bwrap` (Tarea 139), un plazo vencido dice "contenedor bwrap eliminado". No hay contenedor en ese modo. El texto vive en el `#[error]` de `ErrorOraculo::PlazoVencido`, en `src/oracle/mod.rs`. `src/oracle/descarga.rs:756` repite el mismo texto. En modo podman, el texto "infraestructura: timeout tras N s; contenedor X eliminado" no debe cambiar: lo fija un test de la Tarea 109.

**2. Las banderas de `colab empaquetar`.** La 145 dejó hechas las constantes de 4 y 42, la forma `tarea:dimension:n`, `empaquetar_cli_con(pedido, n, semilla)` y el manifiesto. Falta exponer `--n` y `--semilla` en la línea de comandos. `src/lib.rs:985-1005` desestructura `ColabAccion::Empaquetar` sin `..` y arma `PedidoEmpaquetar` campo por campo. Hoy el comando siempre usa 4 y 42.

## Tarea

La regla que manda: **un mensaje nombra lo que corrió, y la línea de comandos deja elegir lo que el paquete ya sabe guardar.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El mensaje de plazo con su motor

1. **`ErrorOraculo::PlazoVencido` suma un campo `motor`.** Vale "contenedor" en modo podman y "proceso bwrap" en modo `bwrap`.
2. **El texto queda** "timeout tras N s; <motor> <nombre> eliminado". En modo podman es el texto de hoy, letra por letra.
3. **`contenedor.rs` llena `motor`** según el modo. Usá la misma lectura del modo que ya usa `argumentos_bwrap`.
4. **`descarga.rs:756`** usa el mismo campo, en vez de repetir el texto.
5. **Los `match` sobre `PlazoVencido`** con `{ .. }` no cambian. Uní codegraph y `grep -w PlazoVencido`, y anotá cada lugar que sí cambia.

**Tests en rojo primero:**
- Unitario: en modo `bwrap`, el texto de un plazo vencido no contiene "contenedor". Hoy lo contiene.
- Guarda: en modo podman, el texto es "infraestructura: timeout tras N s; contenedor X eliminado", el de la Tarea 109.
- Guarda: los tests de `descarga.rs` sobre el plazo siguen verdes.

### 2. `--n` y `--semilla` de `colab empaquetar`

1. **`ColabAccion::Empaquetar` suma** `n: u32`, con valor por defecto 4, y `semilla: u64`, con valor por defecto 42. Usá las constantes de la 145 como valores por defecto, si se alcanzan desde `cli.rs`. Si no, anotalo.
2. **`lib.rs` llama** a `empaquetar_cli_con(pedido, n, semilla)`.
3. **La forma `tarea:dimension:n`** sigue cambiando la `n` de un trabajo.

**Tests en rojo primero:**
- Unitario: `r00t colab empaquetar ... --n 8 --semilla 7` llega a `empaquetar_cli_con` con 8 y 7. Hoy la bandera no existe.
- Unitario: sin las banderas, llegan 4 y 42.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 146 y la 147.
- La prueba de la 141 en Colab, con un paquete armado con `--n 4 --semilla 42`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan los comandos, regeneralos.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-146.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los lugares de `PlazoVencido`**: cada uno, y si cambió.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
