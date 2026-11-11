# Brief de la Tarea 81 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-80` (89fe8f2). La Tarea 81 comparte `src/oracle/calibra.rs` con la 80.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-81`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

Los puntos 1, 2 y 3 tocan `src/oracle/juez.rs`. Van en serie, en ese orden. El punto 4 no pisa a los otros y puede correr en paralelo.

| Punto | Podés tocar |
|---|---|
| 1, lectura guiada (c) | `src/oracle/juez.rs`, `src/oracle/cambios.rs` (nuevo), `src/oracle/mod.rs` (solo `juzgar_diseno` y la declaración del módulo) |
| 2, extractos guardados | `src/oracle/juez.rs` y `docs/interfaces.md` |
| 3, validación de línea (e) | `src/oracle/juez.rs` |
| 4, huella en `Fila` | `src/oracle/calibra.rs`, `src/oracle/calibra/compartidos.rs` (solo si hace falta leer la huella) y `docs/interfaces.md` |

- **`docs/interfaces.md`** lo tocan los puntos 2 y 4. Regeneralo una sola vez, al final.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos, y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`.

## Por qué

**(c) El juez no ve el cambio.** El juez lee el comienzo de los 5 archivos más grandes y de los citados por sondas, con 6000 caracteres por archivo y 40.000 en total (`extractos`, `src/oracle/juez.rs`). En t1 de S2, Arquitectura cubre 6 de 10 de peso, y Diseño 7 de 13. No hay línea base.

**Medido offline por el orquestador:**
- Hoy, 4 de 24 defectos de Vd caen dentro de un extracto.
- Con el doble de lectura y 80.000 en total, son 11 de 24. Leer más del comienzo no alcanza.
- 6 defectos están en archivos que ningún extracto incluye.
- Los 44 hallazgos del juez caen dentro de un extracto.

**Decisión del planner.** Root evalúa un cambio, y conoce su estado inicial. El juez lee primero lo que cambió. El total sigue en 40.000, así que el costo no sube.

**(e) Línea fuera de lo leído.** Hoy no hay casos. La validación cuida la lectura nueva.

**Revisión de cierre, punto 19.** `Fila` no guarda huella de prompt ni de rúbricas. Una calibración vieja sigue marcando "calibrada" después de cambiar el juez. Este cambio de lectura es justo ese caso.

## Tarea

La regla que manda: **el juez lee primero el cambio, y una calibración vale solo para el juez que la midió.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Lectura guiada por el cambio (c)

1. **Estado inicial.** Si existe `<carpeta de la tarea>/bootstrap/`, es el estado inicial. Verificá que esa carpeta sea la misma que usa `r00t corre` (`src/corre/mod.rs:250`). Sin `bootstrap/`, la lectura es la de hoy.
2. **`src/oracle/cambios.rs`.** Compara el repo con `bootstrap/`, archivo por archivo. Saltá `.git/` y `target/`. Devuelve, por archivo, los rangos de líneas del repo que cambiaron. Un archivo nuevo cambia entero. Un archivo borrado no aporta rangos.
3. **Diff sin dependencias.** Usá un diff de líneas por subsecuencia común más larga. Si la parte distinta de un archivo, después de quitar el prefijo y el sufijo comunes, pasa de 2000 líneas, usá ese rango entero.
4. **Orden de lectura.** Primero, los rangos cambiados con 40 líneas de contexto a cada lado. Los rangos que se tocan se unen. Los archivos van ordenados por líneas cambiadas, de más a menos. Después, el orden de hoy: archivos grandes y archivos citados por sondas.
5. **Topes.** El total sigue en 40.000 caracteres (`extractos_total`). Cada ventana de cambio respeta `extracto_max`. Un archivo que ya entró por su cambio no vuelve a entrar desde el comienzo.
6. **Números reales.** Cada línea lleva su número real, como hoy en `numerar`. Entre dos ventanas del mismo archivo va la marca de recorte.
7. **Secretos.** Las líneas que cita `gitleaks` siguen omitidas en toda ventana.
8. **Huella del prompt.** Sumá una constante `VERSION_LECTURA` a `huella_del_prompt`. Así cambia la huella, y los limpios guardados con la lectura vieja no se reusan.

**Tests en rojo primero:**
- Unitario: un archivo de 2000 líneas con un cambio en la línea 900 da una ventana que incluye las líneas 860 a 940.
- Unitario: un archivo nuevo entra desde su comienzo, antes que los archivos grandes sin cambio.
- Unitario: dos cambios a 50 líneas de distancia dan una sola ventana.
- Unitario: el total del prompt no pasa de 40.000 caracteres de extractos.
- Unitario: una línea de secreto dentro de una ventana sale omitida.
- Guarda: sin `bootstrap/`, los extractos son idénticos a los de hoy.
- Guarda: `huella_del_prompt` cambia respecto de 89fe8f2.

**Test de aceptación (vuelta 73).** En t1, `calibrar` de Arquitectura y de Diseño da línea base. Lo corre el orquestador, con API, después de mezclar.

### 2. Extractos guardados

1. `ResultadoJuez` suma `extractos`: una lista con archivo, primera línea, última línea y motivo. El motivo es `cambio`, `grande` o `sonda`.
2. El campo lleva `#[serde(default)]`. Un resultado viejo lee la lista vacía.

**Tests en rojo primero:**
- Unitario: un juicio con una ventana de cambio y un archivo grande guarda los dos extractos, con sus rangos y motivos.
- Guarda: un `ResultadoJuez` serializado sin el campo se lee con la lista vacía.

### 3. Validación de línea (e)

1. Un hallazgo del juez cuya línea no cae dentro de ningún extracto del mismo archivo pasa a `hallazgos_descartados`.
2. Su motivo es "línea fuera de lo leído".
3. Se valida antes del gate de evidencia.

**Tests en rojo primero:**
- Unitario: un hallazgo en la línea 500, con un extracto de 1 a 150, queda descartado con ese motivo.
- Guarda: un hallazgo en la línea 100 del mismo extracto sigue válido.

### 4. Huella en `Fila`

1. `Fila` suma `huella: Option<String>`, con `#[serde(default)]`.
2. La huella junta `juez::huella_del_prompt()` y el contenido de las rúbricas de la dimensión. `calibrar` la escribe en cada fila nueva.
3. En `aplicar`, una fila de una dimensión con ítems `juez` o `mixto` calibra solo si su huella es la actual. Una fila sin huella cuenta como vieja.
4. En una dimensión solo de sondas, una fila sin huella sigue valiendo. Una fila con huella distinta no vale.
5. El motivo queda visible: el puntaje sin calibrar dice "calibración de otro juez".

**Tests en rojo primero:**
- Unitario: una fila de Diseño con huella vieja no calibra, aunque su exactitud llegue a `UMBRAL`.
- Unitario: una fila de Diseño sin huella no calibra.
- Unitario: una fila de Diseño con la huella actual sí calibra.
- Guarda: una fila de una dimensión solo de sondas, sin huella, sigue calibrando.

**Efecto conocido.** Al mezclar, las filas guardadas de memora y brasa en dimensiones de juez dejan de calibrar. Es lo correcto: el juez cambió. No las recalibres.

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

Al terminar, escribí `docs/builds/informe-local-81.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
