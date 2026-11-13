# Brief de la Tarea 109 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-108` (1f60826). Trae la 108 y la 108b. Las Tareas 84 a 108 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-109`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, lectores de `PlazoVencido` | `src/oracle/descarga.rs`, `src/oracle/contenedor.rs`, y cada archivo con un llamador de `contenedor::ejecutar` que salga de la búsqueda del punto 1.1 |
| 2, tests de calibración | `src/oracle/calibra.rs`, solo en su módulo de tests |

- Los dos puntos no comparten archivos. Corren en paralelo.
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

**`PlazoVencido` es un error nuevo, y no todos sus lectores lo conocen.** La Tarea 108 hizo que `contenedor::ejecutar` devuelva `ErrorOraculo::PlazoVencido` cuando vence el plazo, en vez de `ErrorOraculo::Infra`. Varios llamadores miran solo `Infra`:
- `src/oracle/descarga.rs:742` manda cualquier otra variante a `Diagnostico::Desconocido`. El mensaje cambió de "infraestructura: timeout tras N s; contenedor X eliminado" a "la descarga falló: error de infraestructura del oráculo: timeout tras ...".
- `src/oracle/descarga.rs:1326` y `src/oracle/contenedor.rs:726` también miran `ErrorOraculo::Infra`.
- La 108 ya rompió un test con podman de esta forma. La 108b lo arregló. Un test con `saltar_sin_podman` no corre en tu suite, así que solo la búsqueda los encuentra.

**Dos huecos de tests de la 108**, en su sección 3:
- El test de 2 de 5 limpios vencidos usa n = 10, y también lo corta `MIN_LIMPIOS`. No aísla la regla del 20 % (`MAX_LIMPIOS_SIN_VEREDICTO_PCT`).
- Solo Estructura tiene un test de limpio vencido. Diseño y Arquitectura, que tienen juez, no.

## Tarea

La regla que manda: **un plazo vencido es infraestructura en todo lector, y cada regla de calibración tiene un test que la aísla.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Lectores de `PlazoVencido`

1. **Buscá los llamadores.** Uní los llamadores de `contenedor::ejecutar` de codegraph y de `grep -w ejecutar` en `src/` y en `tests/`. Sumá todo `match` o `matches!` sobre `ErrorOraculo::Infra` que reciba un error de esa función.
2. **En cada uno,** `PlazoVencido` recibe el mismo trato que `Infra`, salvo que el código ya lo distinga a propósito, como `mod.rs` desde la Tarea 108.
3. **En `descarga.rs:742`,** `PlazoVencido` va a `Diagnostico::Infra`. El mensaje vuelve a ser el de antes de la 108.
4. Revisá los tests con `saltar_sin_podman` que esperen `Infra` por un plazo. Cambiá solo esos.
5. El informe lista cada lugar revisado y la decisión.

**Tests en rojo primero:**
- Unitario: un `ejecutar` falso que devuelve `PlazoVencido` en la descarga da `Diagnostico::Infra`, con el mensaje "infraestructura: timeout tras N s; contenedor X eliminado".
- Un test por cada otro lugar que cambie.

### 2. Tests de calibración

1. **La regla del 20 %, aislada.** Dos tests unitarios sobre `aplicar`, con una fila armada a mano y `MIN_LIMPIOS` cumplido.
   - Con 2 de 10 limpios sin veredicto, que es 20 %, la dimensión calibra.
   - Con 3 de 10, que es 30 %, no calibra.
2. **Un limpio vencido con juez.** Un test con un juez falso, en Diseño. Un limpio vence el plazo de Root. La calibración sigue, y el limpio queda sin veredicto con el motivo "la suite pasó su plazo".

Si un test pasa desde el principio, es una guarda. Anotalo en el informe.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

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

Al terminar, escribí `docs/builds/informe-local-109.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Lectores revisados**: cada llamador de `contenedor::ejecutar` y cada `match` sobre `Infra`, con su decisión.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
