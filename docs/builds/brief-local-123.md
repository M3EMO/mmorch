# Brief de la Tarea 123 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-122`. Trae las Tareas 84 a 122.
- La 124 se construye a la vez. Toca solo `src/lib.rs`. No toques ese archivo.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-123`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la mutación corre solo un subconjunto | `src/oracle/mutacion.rs` |
| 2, los casos piden los muertos de la base | `src/oracle/calibra.rs` |

- El punto 2 usa el punto 1. Van en serie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/lib.rs`, `src/oracle/mod.rs`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**La medición del reparto del tiempo.** El orquestador calibró Pruebas de s2-t1 con n=4, con un `podman` envoltorio que anota cada llamada.
- La calibración tardó 527 s.
- La mutación usó 524 s en cinco corridas de entre 100 y 108 s: la base y cuatro casos.
- **Las cinco corrieron los mismos 14 mutantes,** con el mismo `-F`.
- La base mató 3 de 14. Dos casos fueron `assert_borrado` y se detectaron por `mutante_vivo`. Dos fueron limpios y dieron lo mismo que la base.

**Lo que un caso puede cambiar.** Un defecto de Pruebas debilita los tests. Un caso se detecta si un mutante que la base mataba queda vivo (`separar_cambios`, `src/oracle/calibra.rs:1299`). Un mutante que la base dejaba vivo sigue vivo, porque un test más débil no mata más. Un limpio mide el ruido de esos mismos mutantes.

**Por eso, correr en un caso los mutantes que la base dejó vivos no aporta nada.** En t1 son 11 de 14.

**El único hueco.** Si un test debilitado llega a un bucle infinito con un mutante, ese mutante vence su plazo, y un `Timeout` cuenta como muerto. Así, un mutante que la base dejaba vivo podría morir. Ese cambio sube el valor del caso. Nunca da una detección falsa. A lo sumo baja un poco la sensibilidad. El planner lo acepta.

**Cómo pasa hoy el contexto de la calibración a la mutación.** Por valores de hilo: `con_plazo_fijo` (`src/oracle/mutacion.rs:83`) fija el `--timeout` de los casos. `evaluar_copia` (`src/oracle/calibra.rs:1022`) lo usa alrededor del evaluador. La muestra de la base está en `r.mutantes_base` (`src/oracle/calibra.rs:945`).

## Tarea

La regla que manda: **un caso de calibración corre solo los mutantes que la base mató, y copia de la base los que dejó vivos.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La mutación corre solo un subconjunto

1. **Un valor de hilo nuevo,** como `PLAZO_FIJO`, guarda la muestra de la base: cada mutante y su estado. Una función `con_muestra_de_base(muestra, f)` lo fija mientras corre `f`, y lo restaura al salir, también si `f` entra en pánico.
2. **Sin ese valor,** la mutación corre como hoy. Una evaluación normal no cambia.
3. **Con ese valor,** el `-F` del comando solo nombra los mutantes de la muestra que la base dio muertos o vencidos.
4. **El resultado lleva los 14 mutantes.** Los que corrieron llevan su estado del reporte. Los demás llevan su estado de la base, que es vivo.
5. **El valor** es muertos sobre la muestra entera, como hoy.
6. **Sin muertos en la base,** no corre cargo-mutants. El resultado es la muestra de la base, sin cambios.
7. **Las validaciones del reporte** comparan contra el subconjunto pedido, no contra la muestra entera. Un mutante pedido que falta en el reporte sigue siendo un error, como hoy.
8. **Un comentario** nombra el hueco del `Timeout` y la decisión del planner.

**Tests en rojo primero, con un ejecutor falso:**
- Unitario: con una muestra de base de 14 y 3 muertos, el `-F` del comando nombra solo los 3. Hoy nombra los 14.
- Unitario: el resultado trae los 14 mutantes. Los 11 vivos de la base siguen vivos, y los 3 llevan su estado del reporte.
- Unitario: si uno de los 3 queda vivo en el reporte, el valor baja de 3/14 a 2/14.
- Unitario: sin muertos en la base, el ejecutor no recibe ningún comando de `cargo mutants`.
- Unitario: un reporte que no trae uno de los 3 pedidos sigue dando el error de hoy.
- Guarda: sin el valor de hilo, el comando es el de hoy.

### 2. Los casos piden los muertos de la base

1. `evaluar_copia` corre el evaluador dentro de `con_muestra_de_base(r.mutantes_base, ...)`, junto a `con_plazo_fijo`.
2. Vale para los casos con defecto y para los limpios.
3. **La base no lo usa.** La base mide los 14, como hoy.
4. `separar_cambios` no cambia. Los mutantes copiados de la base son iguales, así que no cuentan como cambiados.

**Tests en rojo primero, con un evaluador falso que anota los comandos:**
- Unitario: un caso `assert_borrado` corre la mutación con el `-F` de los muertos de la base. Hoy corre la muestra entera.
- Unitario: un limpio también corre solo los muertos de la base.
- Unitario: la base corre la muestra entera.
- Guarda: los tests de calibración de Pruebas de las Tareas 84 a 121 siguen verdes, salvo los que fijaban el `-F` entero de un caso. Anotá esos en el informe.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Los tests de calibración de Pruebas con `saltar_sin_podman`.
- **La medición de la ganancia:** el orquestador repite el reparto del tiempo de la vuelta (42), sección 6, con el binario de la 123. Pruebas con n=4 debe bajar de 527 s a unos 250 s. La sensibilidad y la especificidad deben dar lo mismo.

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

Al terminar, escribí `docs/builds/informe-local-123.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El comando de un caso**: el texto exacto del comando de mutación de un caso, con su `-F`.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
