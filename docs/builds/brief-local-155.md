# Brief de la Tarea 155 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (66).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-154`, cuando la 154 termine. Las dos tocan `src/colab_calibra.rs`.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-155`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la caché entera y los nombres secretos que avisan | `src/colab_calibra.rs` |
| 2, la guarda de los checksums de cargo | `src/colab_calibra.rs` |
| 3, la extracción sin dueños | `src/colab_calibra.rs`, `src/oracle/colab.rs` |

- Los tres puntos comparten `src/colab_calibra.rs`. Van en serie, en este orden.
- Un test puede leer `tools/colab/notebook-juez.py`, pero no lo cambies.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/perfiles/`, `src/oracle/descarga.rs`, los comandos de las sondas, `src/lib.rs`, `src/cli.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`. `serde_json` ya es dependencia, y alcanza para leer `.cargo-checksum.json`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**En Colab, el repo bueno no compila.** La prueba de la 141 con la cadena 152 dio una base de 0,111 contra 1,0 de la laptop. El orquestador guardó las 20 llamadas a bwrap y encontró dos defectos.

**1. Faltan nueve fuentes de la caché.** `cargo nextest run` dio "failed to calculate checksum of: /cache/vendor/syn-2.0.74/src/token.rs ... No such file or directory". `copiar_dir` salta los archivos cuyo nombre marca `es_nombre_secreto`, y "token" es uno de sus segmentos. Faltan `syn-1.0.109/src/token.rs`, `syn-2.0.74/src/token.rs`, `mio-0.8.11/src/token.rs`, `mio-1.0.2/src/token.rs`, `cc-1.1.12/src/parallel/job_token.rs`, y cuatro de tests y benches.

**El filtro protege claves del usuario.** `copiar_dir` copia cinco cosas: `tools/colab`, `oraculos/<tarea>`, la caché de cargo, la base de `rustsec` y `configs`. La caché y `rustsec` vienen de crates.io y de la base de avisos. Un archivo fuente de una dependencia no es una clave. Además, un archivo saltado en silencio rompe la calibración sin aviso. Un test oculto llamado `test_token.py` desaparecería igual.

**2. El `tar` dentro de bwrap no puede cambiar el dueño.** Los pasos de gitleaks y de cargo audit copian el repo con `tar -C /repo ... -cf - . | tar -C /salida/copia -xf -`. Dentro de bwrap, el `tar` que extrae da "Cannot change ownership to uid 65534, gid 65534: Invalid argument" en cada archivo.

**La causa está en la extracción del paquete.** El runtime corre como root. Ahí, `tar -xzf` conserva el dueño de cada archivo del paquete: el uid de la laptop. bwrap con `--unshare-user --uid 0` mapea solo root a root. Un archivo de otro uid aparece adentro como 65534. Toda copia que conserva dueños falla: este `tar`, y también los `cp -a` de `descarga.rs` y de los comandos de tareas. Si el runtime extrae el paquete sin conservar dueños, todo queda de root, y root mapea a 0 dentro de bwrap.

## Tarea

La regla que manda: **el paquete lleva todo lo que cargo verifica, nada se salta en silencio, y el runtime no conserva dueños de la laptop.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La caché entera, y los nombres secretos que avisan

1. **La caché de cargo y `rustsec` se copian enteras.** Sin filtro de nombres secretos ni de `.env*`. Los enlaces simbólicos siguen sin copiarse. Si la caché trae uno, anotalo en el informe.
2. **En `tools/colab`, `oraculos/<tarea>` y `configs`, un archivo con nombre secreto o `.env*` hace fallar el empaquetado.** Da `ErrorOraculo::Uso`, con la ruta del archivo. Hoy se salta en silencio.
3. **`EXCLUIDOS` no cambia,** y sigue en las cinco copias. Desde la 153 incluye `registros`.
4. **Elegí la forma más simple:** dos funciones de copia con nombres claros, o un parámetro con dos valores. Anotá la elección.

**Tests en rojo primero:**
- Unitario: con `cache/rust/<h>/vendor/syn-2.0.74/src/token.rs` en el `ROOT_HOME` falso, el paquete lo trae. Hoy no lo trae.
- Unitario: con `rustsec/crates/jsonwebtoken/RUSTSEC-0000-0000.md`, el paquete lo trae.
- Unitario: con `oraculos/tarea-prueba/oculto/test_token.py`, el empaquetado falla y nombra el archivo. Hoy lo salta.
- Guarda: los tests de exclusión de la 141 y de la 153 siguen verdes.

### 2. La guarda de los checksums de cargo

1. **Después de copiar la caché,** el empaquetado lee cada `.cargo-checksum.json` bajo `vendor/` en la copia.
2. **Cada archivo de su lista `files` debe estar en la copia.** Si falta uno, el empaquetado falla con `ErrorOraculo::Infra`, con el crate y el archivo.
3. **Un `.cargo-checksum.json` que no se puede leer** también hace fallar el empaquetado, con su ruta.
4. **No compares el SHA-256 de cada archivo.** Cargo ya lo hace. La guarda solo revisa que el archivo esté.

**Tests en rojo primero:**
- Unitario: un crate falso con `.cargo-checksum.json` que lista `src/a.rs`, sin ese archivo en la copia, hace fallar el empaquetado con el crate y el archivo. Hoy pasa.
- Unitario: un `.cargo-checksum.json` mal formado hace fallar el empaquetado con su ruta.
- Guarda: un crate con todos sus archivos pasa.

### 3. La extracción sin dueños

1. **`script_preparar` extrae el paquete con `tar --no-same-owner -xzf`.**
2. **`src/oracle/colab.rs:165` también extrae con `--no-same-owner`.**
3. **Un comentario dice por qué:** bwrap con `--unshare-user` mapea solo root a root, y un archivo de otro uid aparece como 65534. Citá el mensaje de Colab.
4. **Revisá `tools/colab/notebook-juez.py`.** Extrae con `tarfile` y `filter="data"`. Ese filtro no aplica los dueños del tar. Si un test lo puede confirmar, escribilo. Si no, anotalo.
5. **Los comandos de las sondas no cambian.** No toques `COMANDO_GITLEAKS`, `COMANDO_CARGO_AUDIT` ni `COMANDO_LOCK`.

**Tests en rojo primero:**
- Unitario: el guion de `script_preparar` lleva `--no-same-owner` en su `tar`. Hoy no lo lleva.
- Unitario: el guion de `colab.rs` lleva `--no-same-owner` en su `tar`. Hoy no lo lleva.
- Integración, si `tar` está en el entorno de los tests: un tar con un archivo de uid 4242, extraído con el comando de `script_preparar` como el usuario de los tests, deja el archivo con el uid del usuario. Si el test no puede correr, que quede ignorado con su motivo.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 153, la 154 y la 155.
- La prueba de la 141 en Colab otra vez. La base de Colab debe dar 1,0. `find /content/root-paquete ! -uid 0 | head` no debe imprimir nada.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-155.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las copias de `copiar_dir`**: cada llamador, y qué regla aplica ahora.
5. **Las extracciones de tar**: cada una, en qué máquina corre, y si conserva dueños.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
