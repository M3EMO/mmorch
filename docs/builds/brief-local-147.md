# Brief de la Tarea 147 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-145`. Trae las Tareas 84 a 145 y la 127b.
- **Arrancá cuando el orquestador termine la calibración de Eficiencia.** Esa calibración necesita la laptop sin otro trabajo.
- La 146 se construye a la vez. Toca `src/oracle/mod.rs`, `src/oracle/contenedor.rs`, `src/oracle/descarga.rs`, `src/cli.rs` y `src/lib.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-147`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la regla de la 136 fuera | `src/oracle/calibra/inyectar.rs` |
| 2, el aviso de la sonda que no corrió | `src/oracle/calibra.rs` |
| 3, `comparar_con_ancla` fuera, y la ruta de la carga | `src/oracle/eficiencia.rs` |
| 4, los enlaces simbólicos, el SHA-256 del tar, y `excluir` | `src/colab_calibra.rs` |

- Los cuatro puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 146, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a la CLI `colab` real ni a podman real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Los informes de la 143, la 144 y la 145 dejaron siete limpiezas.** El planner las decidió en la vuelta (59).

**1. La regla de la 136 no corre en producción.** Solo Eficiencia tiene `LastreDeMemoria`, y desde la 143 siempre pide la sonda. Sin lista de la sonda, el caso queda no aplicable. La regla de la 136 (el nombre llamado en un test viejo) solo la prueba un test unitario. Código que nunca corre es superficie que alguien tiene que leer.

**2. Una sonda que no corrió no avisa.** Con `None`, la fila guarda `corrio: false` y `motivo: None`. El motivo "la sonda no corrió" solo aparece en el caso no aplicable. Un error de la sonda sí avisa.

**3. `comparar_con_ancla` es pública y no la usa producción.** Desde la 144, solo la usan dos tests.

**4. El control de carga lee el `/proc/loadavg` real en los tests de `tests/`.** La 144 lo apaga solo bajo `cfg(test)` del crate. Un test de `tests/` que llame a `medir` con ejecutores falsos lee la carga real. Si la laptop está cargada durante una suite, ese test puede fallar sin un defecto.

**5. Los enlaces simbólicos de un repo no se copian al paquete.** Un repo con enlaces daría en Colab una calibración distinta, sin aviso. Los repos de S2 no tienen enlaces.

**6. El SHA-256 del tar de vuelta viaja dentro del mismo tar.** Cubre un archivo cambiado, pero no un tar entero reemplazado con su manifiesto.

**7. El parámetro `excluir` de `copiar_dir` quedó sin uso.** Todas sus llamadas pasan `&[]`.

## Tarea

La regla que manda: **el código que no corre sale, y lo que puede fallar en silencio avisa.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo los borrados, que empiezan con su guarda.

### 1. La regla de la 136 fuera

1. **Borrá la rama de `lastre_de_memoria`** que elige por nombre llamado en un test viejo, y las funciones que queden sin uso, como `llamados_de_tests_viejos`. Uní sus llamadores con codegraph y `grep -w` antes de borrar.
2. **Sin lista de la sonda,** el caso queda no aplicable, como desde la 143.
3. **`lugar_por`** queda con un solo valor de producción: `sonda`. Si el tipo lo permite sin cambio de esquema, dejá el valor `nombre` para leer filas viejas.

**Guardas:**
- Los tests de `LastreDeMemoria` de la 143 siguen verdes.
- Los tests que solo probaban la regla de la 136 se borran. Anotá cada uno en el informe.

### 2. El aviso de la sonda que no corrió

1. **Con la sonda pedida y sin resultado (`None`),** la fila guarda el motivo "la sonda no corrió" en `sonda_lastre.motivo`.
2. **Ese motivo llega a los avisos de la calibración,** igual que un error de la sonda.

**Tests en rojo primero:**
- Unitario: con la sonda pedida y sin resultado, la fila guarda el motivo y la calibración trae el aviso. Hoy no trae nada.

### 3. `comparar_con_ancla` fuera, y la ruta de la carga

1. **Borrá `comparar_con_ancla`** y pasá sus dos tests a la función que usa producción, si prueban algo que no está cubierto. Si no, borralos y anotalo.
2. **La ruta de `/proc/loadavg` se puede cambiar** con una variable de entorno, `ROOT_LOADAVG`. Sin la variable, es `/proc/loadavg`. Su comentario dice que existe para los tests de `tests/`.
3. **Los tests de `tests/` que llaman a `medir`** apuntan `ROOT_LOADAVG` a un archivo temporal con una carga baja. Si un test no puede fijar la variable sin pisar a otros, anotalo y proponé otra forma.

**Tests en rojo primero:**
- Unitario: con `ROOT_LOADAVG` apuntando a un archivo con `9.00 ...`, el control de carga lo lee. Hoy lee siempre `/proc/loadavg`.

### 4. Los enlaces simbólicos, el SHA-256 del tar, y `excluir`

1. **Un repo con un enlace simbólico hace fallar `colab empaquetar`,** y el mensaje nombra el enlace. No lo copia ni lo salta en silencio.
2. **El guion de cierre del runtime imprime** `ROOT_SHA_VUELTA=<sha256 del tar>` después de armar la vuelta. La laptop compara ese valor con el SHA-256 del tar que bajó, antes de abrirlo. Si difieren, la importación no corre.
3. **Borrá el parámetro `excluir` de `copiar_dir`,** después de unir sus llamadores.

**Tests en rojo primero:**
- Unitario: un repo con un enlace hace fallar el empaquetado, con el nombre del enlace. Hoy lo salta.
- Unitario: una vuelta cuyo `ROOT_SHA_VUELTA` no coincide con el tar bajado no se importa.
- Guarda: `copiar_dir` sin `excluir` copia lo mismo que hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 146 y la 147.
- La prueba de la 141 en Colab, con el marcador `ROOT_SHA_VUELTA`.

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

Al terminar, escribí `docs/builds/informe-local-147.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Lo que se borró**: cada función, parámetro o test, con sus llamadores antes de borrar.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
