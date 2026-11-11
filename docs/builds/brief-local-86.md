# Brief de la Tarea 86 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-85` (3cd5898). Las Tareas 82 a 85 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-86`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

**Regla nueva para los subagentes.** En la Tarea 85, un subagente editó otro worktree por error. Desde esta tarea:
- El organizador da a cada subagente la ruta absoluta de su worktree.
- Antes de cada commit, el organizador corre `git -C <ruta> status`. Compara los archivos cambiados con la tabla de abajo. Un archivo que no está en la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, cerrojo del spawn y la señal | `src/corre/ejecutor.rs`, `src/corre/corrida.rs` |
| 2, borrar `ENTRADA_JUEZ_BYTES` | `src/oracle/calibra.rs` |
| 3, guarda real de cargo audit | `src/oracle/perfiles/herramientas.rs`, solo en su módulo de tests |

- Los tres puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función o de borrar un símbolo, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. En esta máquina el comando es `codegraph`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`.

## Por qué

**Carrera de la señal.** El informe de la Tarea 85 dejó un hueco. `HIJO_EN_CURSO` (`src/corre/ejecutor.rs:307`) se llena después del spawn de `podman run`. Si la señal llega entre el spawn y el registro, el hilo de la señal no ve el hijo. Root sale, y el hijo sigue vivo con la credencial montada. Quitar el contenedor por nombre no alcanza si podman todavía no lo registró.

**`ENTRADA_JUEZ_BYTES`.** La constante (`src/oracle/calibra.rs:1215`) quedó sin usos cuando la Tarea 83 borró las cotas por caso.

**cargo audit sin guarda real.** El commit 4f3358c hace que cargo audit corra sobre una copia sin `.cargo/audit.toml` (`src/oracle/perfiles/herramientas.rs:421` a `424`). gitleaks tiene tres guardas con la herramienta real. cargo audit solo tiene un test del texto del comando: `comando_cargo_audit_corre_sobre_copia_sin_audit_toml`. Nadie probó que la herramienta real no lea un `audit.toml` del repo.

## Tarea

La regla que manda: **ningún hijo con la credencial sobrevive a Root, y cada guarda de aislamiento usa la herramienta real.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 2.

### 1. Cerrojo del spawn y la señal

1. El spawn de `podman run` y el registro de su pid en `HIJO_EN_CURSO` van juntos, bajo un mismo cerrojo.
2. El hilo de la señal toma ese cerrojo antes de leer el pid. Así, o no hay hijo, o ya está registrado.
3. Con el cerrojo tomado, el hilo marca "terminando". Un spawn que llega después de esa marca no lanza nada y devuelve error.
4. El cerrojo cubre solo el spawn y el registro. La espera del hijo queda afuera.
5. `terminar_hijo_en_curso` y el orden de la Tarea 85 no cambian: primero el hijo y los contenedores, después la copia.

**Anexo del orquestador: revisión del código de la señal de la Tarea 85.** Un revisor leyó 3cd5898 y halló tres defectos del mismo punto. El orquestador verificó el primero. Detalle en `/home/mpera/.cache/root-wt/orquestador/revision-senal-85.md`.

6. **La marca "terminando" se mira también antes de crear la copia.** Hoy, entre dos roles, el hilo principal llama a `preparar_credencial` (`ejecutor.rs:212`) mientras la señal quita contenedores. Después de la marca, `lanzar` no crea copia ni lanza `podman run`.
7. **Los comandos de la señal no pisan `HIJO_EN_CURSO`.** Hoy `quitar_builders_de_este_proceso` usa `correr_proceso`, que registra su propio pid y lo deja en `None` al salir (`ejecutor.rs:307-323`, `361-410`). Esos `podman ps` y `podman rm` no se registran.
8. **El registro se limpia apenas termina el `wait`.** Hoy el pid queda registrado después del `wait()` (`ejecutor.rs:462-478`). En esa ventana, la señal puede mandar SIGKILL a un pid reciclado.

**Tests en rojo del anexo:**
- Unitario: con la marca puesta, `lanzar` no crea la copia de la credencial.
- Unitario: después de `quitar_builders_de_este_proceso`, `HIJO_EN_CURSO` conserva el pid del hijo que ya estaba.
- Unitario: después del `wait`, el registro queda vacío antes de volver.

**Tests en rojo primero:**
- Unitario: con una costura de test entre el spawn y el registro, una señal en esa ventana termina el hijo. Hoy el hijo sobrevive.
- Unitario: un spawn pedido después de "terminando" no lanza ningún proceso.
- Guarda: los tests de señal de las Tareas 84 y 85 siguen verdes, con el cerrojo `serie_de_pruebas`.

### 2. Borrar `ENTRADA_JUEZ_BYTES`

1. Uní los llamadores de codegraph y de grep, en `src/` y en `tests/`.
2. Sin llamadores, borrala con su comentario.
3. Con un llamador, no la borres. Explicalo en el informe.

Este punto no cambia comportamiento. La guarda es la suite verde.

### 3. Guarda real de cargo audit

1. Escribí `guarda_cargo_audit_ignora_el_audit_toml_del_repo`, junto a las guardas de gitleaks. Empieza con `crate::saltar_sin_podman!();`, como ellas.
2. El repo de prueba trae un `Cargo.lock` con un crate y una versión que tengan un aviso en la base de RustSec. La base se monta en `/rustsec` (`tiene_rustsec`). No hace falta red: cargo audit lee el lock con `--no-fetch`.
3. El repo trae `.cargo/audit.toml` que ignora ese aviso por su id.
4. El veredicto de Seguridad trae el hallazgo igual.
5. Elegí el aviso desde la base que usa la imagen. Anotá en un comentario el id del aviso y su fuente.
6. Si el test no puede montar la base de RustSec sin podman real, dejalo escrito y con `saltar_sin_podman`. El orquestador lo corre al mezclar.

**Rojo primero, con podman.** No podés ver el rojo sin podman. Para verlo, el orquestador puede quitar la exclusión de `.cargo/audit.toml` en una copia local y correr solo este test. Anotá en el informe el comando exacto para eso.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-86.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
6. **Guarda de cargo audit**: el aviso elegido, su fuente, y el comando para ver el rojo con podman.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
