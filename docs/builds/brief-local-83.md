# Brief de la Tarea 83 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-82` (5aa2dc6). La Tarea 83 comparte `calibra.rs` y `juez.rs` con la 82, que todavía no está mezclada.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-83`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, control de gasto por llamada | `src/proveedores/mod.rs`, `src/oracle/juez.rs` (solo `JuezProveedor` y las funciones de cota), `src/oracle/calibra.rs` (solo `Presupuesto` y las cotas), `src/lib.rs` (solo `calibrar`), `src/observe/uso.rs` (solo si hace falta reservar de forma atómica) |
| 2, estabilidad de ítems | `src/oracle/calibra.rs`, `src/oracle/calibra/regla.rs`, `src/tipos.rs`, `src/oracle/mod.rs` y `docs/interfaces.md` |
| 3, extractos en los casos | `src/oracle/calibra.rs` y `docs/interfaces.md` |
| 4, marca de línea omitida | `src/oracle/juez.rs` (solo `numerar` y las ventanas) |

**Orden.**
- Los puntos 1, 2 y 3 comparten `calibra.rs`. Van en serie, en ese orden.
- El punto 4 comparte `juez.rs` con el 1. Va después del 1, y en paralelo con el 2 y el 3.
- `docs/interfaces.md` se regenera una sola vez, al final.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. En esta máquina el comando es `codegraph`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`.

## Por qué

**Cota.** La cota del peor caso se calcula por caso, antes de conocer el prompt. Usa el máximo de entrada, con supuestos. Con los precios de `root.toml`, la cota de Diseño es 3,25 USD por caso con la Tarea 82. Lo medido es cerca de 0,07 USD por juicio. Con 2,56 USD de tope, Diseño no calibra ni un caso.

**Estabilidad.** El orquestador juzgó 4 veces el mismo código de t1 en S2:
- `diseno.errores` dio 3, 0, 0 y 0.
- `diseno.modularidad` dio 0, 2, 0 y 2.
- Varios ítems aparecen y desaparecen entre juicios.

Detectar exige bajar 2 niveles frente a un solo juicio de la base. Con este ruido, un limpio da falsa alarma por azar. La cobertura cruza los dos tercios al azar, y un caso queda sin veredicto.

**Extractos.** Los casos de calibración (`*.casos.jsonl`) no guardan los extractos que leyó el juez. Solo los guarda el veredicto. Así no se puede saber si el juez vio el defecto.

**Línea omitida.** Una línea sola más larga que `extracto_max` hoy se omite en silencio.

## Tarea

La regla que manda: **el gasto nunca pasa el tope, y un ítem que cambia sin cambio de código no mide nada.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Control de gasto por llamada

1. **Antes de cada intento,** Root ya tiene el prompt armado, y conoce sus bytes exactos.
2. **Cota del intento:** bytes del prompt al precio de entrada, más todo `max_tokens` al precio de salida. Un byte es una cota de un token, así que es una cota estricta.
3. **Reserva.** Si lo gastado más las reservas abiertas más la cota del intento no pasa el tope, el intento reserva su cota y sale. Al terminar, la reserva se cierra con el costo real. Usá el mecanismo de reservas que ya existe en `src/observe/uso.rs`.
4. **Atómico.** La revisión y la reserva van bajo el mismo lock de eventos. Dos casos en paralelo no pueden pasar el tope entre los dos.
5. **Sin lugar,** el intento no sale. El caso queda incompleto, como hoy con el tope.
6. **Todos los intentos.** El control corre antes de cada intento de `INTENTOS_MAX`, y antes del reintento del juez. El reintento de transporte vive en `src/proveedores/mod.rs`, así que el control tiene que entrar ahí.
7. **Sin tope,** nada cambia.
8. **El control por caso se va.** `Presupuesto` deja de pedir `peor_caso` por caso. Borrá `peor_caso_de_caso_con_entrada`, `peor_caso_de_home_con_tarea` y `cota_bytes_de_prompt` si quedan sin uso. Un modelo sin precio sigue siendo error de configuración antes de arrancar.
9. `auditar` tiene su propio `--tope-usd`. No lo cambies, y anotá en el informe cómo calcula su cota.

**Tests en rojo primero:**
- **Aceptación (vuelta del 2026-11-11, 3):** precios de 1,74 por millón de entrada y 3,48 por millón de salida, y `max_tokens` de 32.000. Con tope de 0,30 USD, un intento con un prompt de 100.000 bytes sale: su cota es 0,285. El juez falso cuesta 0,07 USD. El segundo intento no sale: 0,07 más 0,285 pasa 0,30.
- Unitario: dos hilos que piden a la vez, con lugar para uno solo, dejan salir uno solo.
- Unitario: el reintento del juez pasa por el mismo control.
- Unitario: una reserva se cierra con el costo real, y el lugar que sobra queda libre.
- Guarda: un modelo sin precio da error antes del primer caso.
- Guarda: sin tope, la calibración corre como hoy.

### 2. Estabilidad de ítems por tarea

1. **Base repetida.** `calibrar` juzga la base 3 veces. Las sondas no se repiten: solo el juez, con la misma entrada.
2. **Ítem inestable.** En esa tarea, un ítem es inestable si sus 3 niveles difieren en 2 o más, o si falta en alguno.
3. **Fuera de todo.** Un ítem inestable no entra a la detección (`calibra/regla.rs`), al puntaje ni a la cobertura.
4. **Cobertura.** La cobertura se mide sobre el peso de los ítems estables.
5. **Dimensión inestable.** Si los ítems estables pesan menos de la mitad del peso de la dimensión, la dimensión queda "inestable en esta tarea". No calibra y no da puntaje.
6. **Nivel de la base.** El nivel de un ítem estable en la base es la mediana de sus 3 niveles.
7. **Guardado.** `Fila` suma `inestables: Vec<String>`, con `#[serde(default)]`. Una fila vieja lee la lista vacía.
8. **`evaluar`** usa la lista de ítems inestables de la última fila de la tarea y la dimensión. Sin fila, nada cambia.
9. **Visible.** El veredicto nombra los ítems inestables de cada dimensión. Un puntaje de una dimensión inestable dice "inestable en esta tarea".
10. Solo aplica a dimensiones con ítems `juez` o `mixto`. Las dimensiones solo de sondas no cambian.

**Tests en rojo primero:**
- **Aceptación (vuelta del 2026-11-11, 3):** un juez falso alterna 0 y 2 en un ítem de Diseño. Ese ítem queda inestable y fuera de la cobertura.
- Unitario: un ítem que falta en 1 de 3 juicios queda inestable.
- Unitario: un ítem con 3, 2 y 3 queda estable, con nivel 3 en la base.
- Unitario: con ítems estables de menos de la mitad del peso, la dimensión queda "inestable en esta tarea" y no calibra.
- Unitario: un limpio que baja 2 niveles en un ítem inestable no cuenta como falsa alarma.
- Unitario: `evaluar` con una fila que lista un ítem inestable no usa ese ítem en el puntaje.
- Guarda: una dimensión solo de sondas calibra igual que hoy.
- Guarda: una fila sin `inestables` se lee con la lista vacía.

### 3. Extractos en los casos de calibración

1. Cada caso guardado en `*.casos.jsonl` suma los extractos que leyó el juez: archivo, primera línea, última línea y motivo.
2. La base guarda los extractos de sus 3 juicios.
3. El campo lleva `#[serde(default)]`. Un caso viejo lee la lista vacía.

**Tests en rojo primero:**
- Unitario: un caso con defecto guarda sus extractos, y el rango de uno incluye la línea del defecto cuando el juez la leyó.
- Guarda: un caso viejo sin el campo se lee con la lista vacía.

### 4. Marca de línea omitida

1. Una línea sola más larga que `extracto_max` no se omite en silencio.
2. En su lugar va la marca "línea N omitida por largo", con su número real.
3. La marca cuenta dentro de `extracto_max` y de `extractos_total`.

**Tests en rojo primero:**
- Unitario: una línea de 7000 caracteres, con `extracto_max` de 6000, da la marca con su número.
- Guarda: los tests de ventanas de las Tareas 81 y 82 siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador. Después de mezclar, el orquestador calibra Diseño y Arquitectura en t1 de S2.

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

Al terminar, escribí `docs/builds/informe-local-83.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Funciones borradas**: las cotas que quedaron sin uso, con sus llamadores revisados.
6. **Cota de `auditar`**: cómo la calcula hoy.
7. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
