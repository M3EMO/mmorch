# Brief de la Tarea 165 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (84).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-164`. Trae las Tareas 84 a 164 y la 162b.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-165`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el builder que sale con error, y el singular | `src/corre/corrida.rs` |
| 2, el lock que no se puede leer, en el inventario | `src/oracle/descarga/inventario.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/descarga.rs`, `src/oracle/informe.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **No cambies `bloqueos`, `no_listo` ni `sin_decision`.**
- **Sin cambio de esquema.** `vuelta_instalacion` ya existe desde la 164.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- La tarea no necesita podman, claves de API reales ni el `ROOT_HOME` real.
- **No llames a podman real, a la CLI `colab` real ni a ningún builder real.** Los tests de `corre` usan sus lanzadores falsos.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`. El orquestador lo corre al revisar.

## Por qué

**1. Un builder que sale con error, ante un producto que no instala, no deja marca.** Desde la 164, la vuelta de instalación corre solo si todos los roles terminaron con 0. Si un rol sale con otro código y el producto no instala, `vuelta_instalacion` queda `None`, y no sale ningún aviso. Pero es el mismo caso que una vuelta que no corre: el agente no pudo. Por P122 (a), eso va a un humano. Además, con 1 vuelta el aviso dice "1 vueltas corridas".

**2. El mismo lock roto da dos decisiones.** En `src/oracle/descarga/inventario.rs`, un `Cargo.lock` que no se puede leer se vuelve un `Rechazo`, con el motivo "Cargo.lock inválido". Un rechazo da el corte "fuente no aceptada", que queda en `SinDecision`. Si el lock llega a `preparar_rust`, en cambio, da "lock inválido", con su arreglo y `NoListo`. Por P121 (a), un lock que no se puede leer da `NoListo`. El test viejo de `LOCK_ROTO` daba "fuente no aceptada" por este camino.

## Tarea

La regla que manda: **el mismo defecto da la misma decisión, y todo caso que el agente no puede arreglar va a un humano.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El builder que sale con error, y el singular

1. **Si un rol sale con un código distinto de 0 y la evaluación es un corte "no instala" o "lock inválido",** la fila suma `vuelta_instalacion` con 0 vueltas, `instalo` falso y `pide_humano` verdadero.
2. **El motivo dice** "el builder salió con código N", con el rol.
3. **`Veredicto.avisos` suma** "pide intervención humana", como con las vueltas agotadas.
4. **No corre ninguna vuelta.** El builder ya falló.
5. **El aviso usa el singular con 1:** "1 vuelta", y el plural con 2.

**Tests en rojo primero:**
- Unitario: un rol con código 1 y un corte "no instala" dejan `pide_humano` verdadero, con el motivo del código. Hoy dejan `vuelta_instalacion` en `None`.
- Unitario: con 1 vuelta agotada, el aviso dice "1 vuelta". Hoy dice "1 vueltas".
- Guarda: un rol con código 1 y un producto que instala no suman `vuelta_instalacion`.
- Guarda: los 8 tests `corre_instalacion_*` de la 164 siguen verdes, salvo el texto del singular.

### 2. El lock que no se puede leer, en el inventario

1. **El inventario no rechaza un `Cargo.lock` que no se puede leer.** No suma un `Rechazo` por eso.
2. **Sigue revisando las fuentes de `Cargo.toml`,** como hoy con una lista de paquetes vacía.
3. **El lock llega a `preparar_rust`,** que ya da el corte "lock inválido", con su arreglo y `NoListo`.
4. **Una fuente no aceptada de verdad** sigue dando su rechazo, como hoy.

**Tests en rojo primero:**
- Unitario: un `Cargo.lock` que no parsea no deja ningún rechazo en el inventario. Hoy deja "Cargo.lock inválido".
- Integración, si las costuras de la 163 alcanzan: una evaluación con un `Cargo.lock` que no parsea da el corte "lock inválido" y `NoListo`. Hoy da "fuente no aceptada".
- Guarda: una dependencia git sin `rev` sigue dando su rechazo.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 165.
- La corrida real de `corre` sobre b8d0db4058 con su lock real.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Oráculo.** Ningún prompt al builder lleva ids de tests, evidencia ni contenido de `oraculos/`.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-165.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La decisión de un lock roto**: lo que da ahora, por cada camino.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
