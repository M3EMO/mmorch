# Brief de la Tarea 120 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-119`. Trae las Tareas 84 a 119.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-120`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el digest inyectado | `src/oracle/calibra.rs`, `src/oracle/calibra/target.rs`, `tests/calibra.rs` |
| 2, dos tests sin guarda | `src/oracle/calibra/defectos.rs`, `tests/oraculo_home.rs` (solo esos dos tests) |
| 3, la carrera ETXTBSY | `src/limpieza.rs` (solo su módulo de tests) |
| 4, el aviso del borrado | `src/oracle/eficiencia.rs`, `src/oracle/mod.rs` |
| 5, el desempate alfabético | `src/oracle/informe.rs` |
| 6, la instrucción de la vuelta | `src/corre/corrida.rs` |

- Los puntos 1 y 2 tocan archivos de `calibra/`. Van en serie. Los demás no comparten archivos y corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/tipos.rs`, `src/oracle/contenedor.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.** Si un punto lo pide, detenete y escribilo en el informe.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La auditoría de la Tarea 118.** La suite sin podman corrió con un `podman` falso al frente del `PATH`. 43 tests sin `saltar_sin_podman` lo llamaron.

**41 tests de calibración leen el digest de la imagen.** Su camino es `calibrar_tarea_con` (`src/oracle/calibra.rs:1992`) o `calibrar_tarea`, después `leer_digest` (`src/oracle/calibra.rs:1578`), y después `target::digest_de_imagen` (`src/oracle/calibra/target.rs:51`). Esa función corre `podman image inspect`. Esos tests inyectan su evaluador, pero no el digest. Su resultado depende de las imágenes de la laptop. 28 viven en `src/oracle/calibra.rs` y 13 en `tests/calibra.rs`. La lista está en `docs/builds/informe-local-118.md`, sección 4.

**Dos tests lanzan contenedores sin su guarda:**
- `panico_en_entrada_lo_detecta_la_sonda_nativa` (`src/oracle/calibra/defectos.rs:309`), por `evaluar_copia_con` con `PodmanReal`.
- `oraculo_home_evaluar_y_llamadas_escriben_solo_en_su_home` (`tests/oraculo_home.rs:55`), por `evaluar` con `PodmanReal`.

**La carrera ETXTBSY.** `barrido_con_otros_valores_de_sin_podman_llama_al_programa` falló una vez en cinco con `Text file busy`. `podman_falso` (`src/limpieza.rs:380`) escribe un script. Mientras tanto, otro hilo hace fork, y el hijo hereda el descriptor de escritura. Ese descriptor recién se cierra cuando el hijo hace `exec`. Si el test lanza el script en ese momento, el kernel da ETXTBSY. El hook corre la suite en cada commit, así que una falla en cinco frena un commit de cada cinco.

**Tres arreglos chicos que el planner asignó a esta tarea:**
- `eficiencia::medir` ignora un error de `borrar_dir_dentro`. La carpeta puede dejar un `target` de varios GB, y el disco tiene 39 GB libres.
- Con dos ids desconocidos de igual razón, el resumen nombra el último del vector. El resultado debe ser determinista.
- La instrucción de la vuelta de eficiencia puede escribir "sin valor veces la referencia". Una medida sin valor no tiene regresión, y no debe aparecer.

## Tarea

La regla que manda: **un test sin podman no llama al podman real, y su resultado no depende de la laptop.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El digest inyectado

1. **La lectura del digest es una dependencia** de `calibrar_tarea_con`, como el evaluador. Producción pasa `target::digest_de_imagen`.
2. **Los tests pasan un lector falso** que da un digest fijo, sin llamar a podman.
3. Los 41 tests de la lista siguen sin `saltar_sin_podman`. Prueban lógica de calibración.
4. Uní los llamadores de `calibrar_tarea_con` y `calibrar_tarea`. Anotá cada firma cambiada en el informe.

**Tests en rojo primero:**
- Unitario: una calibración con un lector falso que da `sha256:fijo` usa ese digest donde hoy usa el real. Por ejemplo, en la decisión de reusar el `target` de la base. Hoy usa el de la laptop.
- Unitario: el lector falso se llama una vez por calibración.
- **Guarda dinámica:** corré la suite sin podman con un `podman` falso al frente del `PATH`, como la auditoría de la 118. El falso anota cada llamada en tu worktree. Ningún test de la lista lo llama. Anotá el número de llamadas antes y después.

### 2. Dos tests sin guarda

1. Los dos tests llevan `crate::saltar_sin_podman!()` al principio, o el macro equivalente de `tests/`.
2. No cambies lo que prueban.

**Tests en rojo primero:**
- La guarda dinámica del punto 1 cuenta estos dos tests. Después del cambio, no llaman al falso.

### 3. La carrera ETXTBSY

1. **`podman_falso` prueba el script antes de devolverlo.** Lo lanza con un argumento de sonda, como `--sonda`. Si el error trae el código 26 de Linux (ETXTBSY, `raw_os_error() == Some(26)`), espera 10 ms y prueba otra vez, hasta 50 veces. Anotá el motivo del 26 en un comentario.
2. **El script trata la sonda sin efectos.** Con `--sonda`, sale con 0 y no deja ninguna marca.
3. **Por qué alcanza.** Si una prueba lanza el script, ningún hijo guarda el descriptor de escritura. Los forks nuevos ya no lo heredan, porque el descriptor está cerrado.
4. Solo cambia el helper de tests. El código de producción no reintenta.
5. Si `src/corre/corrida.rs:3436` y `tests/corre.rs:233` tienen la misma carrera, anotalo en el informe. No los cambies.

**Tests en rojo primero:**
- Unitario: un helper que escribe el script mientras otro hilo hace fork en bucle da ETXTBSY sin la prueba. Con la prueba, 200 repeticiones no fallan. Si no se puede reproducir en rojo, anotalo, y dejá la repetición como guarda.

### 4. El aviso del borrado

1. Si el borrado de la carpeta de trabajo falla, el veredicto suma un aviso con la carpeta y el error.
2. La medida no cambia.
3. Usá la lista de avisos que ya tiene el veredicto. No cambies el esquema.

**Tests en rojo primero:**
- Unitario: con un borrado que falla, el veredicto trae el aviso con la ruta. Hoy no trae nada.

### 5. El desempate alfabético

1. Después de los cuatro ids conocidos, en su orden fijo, los ids desconocidos van en orden alfabético.

**Tests en rojo primero:**
- Unitario: los ids `zeta` y `alfa`, con razón 1,3 los dos, y `zeta` primero en el vector. El resumen nombra `alfa`. Hoy nombra `zeta`.

### 6. La instrucción de la vuelta

1. `PROMPT_EFICIENCIA` y su lista nombran solo medidas con regresión y con los dos números.

**Tests en rojo primero:**
- Unitario: un veredicto con una medida con regresión y otra sin valor da una instrucción que no contiene "sin valor".

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- `panico_en_entrada_lo_detecta_la_sonda_nativa` y `oraculo_home_evaluar_y_llamadas_escriben_solo_en_su_home`, ahora con su guarda.
- Que `target::digest_de_imagen` siga leyendo el digest real en una calibración con podman.
- El aviso del borrado con un borrado real que falla. El unitario usa un borrado falso.

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

Al terminar, escribí `docs/builds/informe-local-120.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La guarda dinámica**: las llamadas al `podman` falso antes y después, por test.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
