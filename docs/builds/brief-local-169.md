# Brief de la Tarea 169 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (94).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/tarea-168`,** cuando el orquestador la revise. Trae 2020ecc, la 167 y la 168.
- Trabajá en una rama nueva: `laptop/tarea-169`.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Una versión nueva del juez

**Esta tarea cambia lo que Root mide, solo en el juez.** El usuario lo decidió en P125 a. Después de la mezcla, el orquestador congela una versión nueva.
- **Cambia el prompt fijo del juez.** Por eso cambia `huella_del_prompt`, y con ella la huella de Arquitectura, Diseño y Estructura. Sus filas viejas dejan de calibrar. Es lo esperado.
- **Las huellas de las seis dimensiones de oráculo no cambian.** Su huella no lleva el prompt.
- **Nada más cambia:** ni rúbricas, ni anclas, ni sondas, ni el modelo, ni `MAX_TOKENS_JUEZ`, ni el gate de evidencia, ni la regla de inestables, ni la regla de cobertura.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el prompt del juez | `src/oracle/juez.rs` |
| 2, la fase de los eventos del juez | `src/oracle/juez.rs` |
| 3, las huellas | solo tests: `src/oracle/calibra.rs` |

- Los tres puntos comparten archivos. Van en serie: 1, 2 y 3.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** el resto de `src/oracle/`, `src/proveedores/`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema** de `Fila`, `Veredicto`, `Evento` ni de la respuesta del juez.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- La tarea no necesita podman, claves de API reales ni el `ROOT_HOME` real.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a ningún builder real.** Los tests usan un `ModeloJuez` falso.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`. El orquestador lo corre al revisar.

## Por qué

**El juez no repite su propio nivel sobre el mismo código.** La base de cada calibración se juzga tres veces. En P107 a, cuatro pares de tarea e ítem quedaron inestables en la base:
- `arquitectura.adecuacion` en s2-t1: falta en un juicio, porque el juez dijo `sin_evidencia`;
- `arquitectura.adecuacion` en s2-t3: varía de 0 a 3;
- `diseno.errores` en s2-t2: varía de 0 a 3;
- `diseno.errores` en s2-t3: varía de 1 a 3.

Un ítem inestable sale de la dimensión en esa tarea. Un defecto sobre ese ítem ya no se puede detectar. Así quedaron sin detectar 8 de los 12 defectos que contaron y no se detectaron. Además, 2 de los 3 defectos sin veredicto caen sobre esos ítems.

**La causa más probable es una contradicción del prompt con las anclas.**
- El prompt dice: "el nivel de cada ítem parte en 0 y solo sube con evidencia citada textual".
- Pero el ancla 0 de estos ítems pide citar un defecto: un error que se traga, o un lock que rompe la ficha.
- El ancla 3 describe una ausencia: "ningún error se traga", "sin riesgo citado".
- Y el gate exige al menos una cita válida para cualquier nivel, también para el 3.

Sobre código sin defecto, el juez no tiene un camino estable. A veces cita algo y da 3. A veces da 0 por "parte en 0". A veces da `sin_evidencia`. Las tres salidas aparecen en la base de P107 a.

**Lo que no es la causa:**
- **El tope de tokens no.** Las 108 llamadas usaron de 2184 a 18186 tokens de salida, de un tope de 32 000. Ninguna salió truncada.
- **El parser no.** Ninguna respuesta dio JSON inválido. Los faltantes dicen "el juez no encontró evidencia", que es la respuesta `sin_evidencia` del propio juez.
- **Lo que el juez lee no.** En los 26 casos que contaron, la línea del defecto cae dentro de un extracto del prompt.

## Tarea

La regla que manda: **sobre el mismo código, el juez tiene un solo camino para cada ítem: buscar el código del tema, revisar las anclas en orden y citar lo que encontró.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El prompt del juez

En `PROMPT_SISTEMA`, de `src/oracle/juez.rs`, hacé estos tres cambios. El resto del texto queda igual, byte por byte.

1. **Reemplazá esta línea:**
   ```
   REFUTA POR DEFAULT: el nivel de cada ítem parte en 0 y solo sube con evidencia citada textual.
   ```
   por esta:
   ```
   REFUTA POR DEFAULT: ningún nivel vale sin una cita textual que cumpla su ancla.
   ```
2. **Después de la línea** `Cada ítem trae evidencia que cumple el ancla de su nivel.`, **agregá estas tres líneas:**
   ```
   Para cada ítem, buscá primero en los EXTRACTOS el código que nombran sus anclas: por ejemplo, una llamada de E/S y su error, o un lock y su alcance.
   Revisá las anclas en orden, de 0 a 3. El nivel es la primera ancla que el código citado cumple.
   Si el código del tema está y no muestra el defecto de las anclas 0, 1 ni 2, el nivel es 3, y la cita es ese código.
   ```
3. **Reemplazá esta línea:**
   ```
   Sin evidencia para un ítem, responde \"nivel\":null, \"motivo\":\"sin_evidencia\" y \"evidencia\":[].
   ```
   por esta:
   ```
   Solo si ningún extracto trae código del tema del ítem, responde \"nivel\":null, \"motivo\":\"sin_evidencia\" y \"evidencia\":[].
   ```

**Tests en rojo primero:**
- Unitario: `huella_del_prompt()` es distinta del literal de la base. Calculá el literal sobre la base y escribilo en el test.
- Unitario: el prompt del sistema no contiene "parte en 0". Hoy lo contiene.
- Unitario: el prompt del sistema contiene "de 0 a 3" y "Solo si ningún extracto trae código del tema".
- Guarda: con un `ModeloJuez` falso, un nivel 3 con una cita que existe en el repo pasa el gate.
- Guarda: un nivel 3 sin evidencia no da nivel: el gate lo frena, como hoy.
- Guarda: una respuesta `sin_evidencia` se lee igual que hoy.
- Guarda: una respuesta truncada sigue dando sin veredicto, con su causa.
- Guarda: `construir_prompt` es determinista: la misma entrada da el mismo prompt.
- Guarda: el prompt de una entrada de prueba crece menos de 1024 bytes.

### 2. La fase de los eventos del juez

1. **`FASE` pasa de `"juez-diseno"` a `"juez"`.** Una llamada del juez puntúa Estructura, Arquitectura y Diseño juntas. El nombre de hoy dice solo una.
2. **Buscá con `grep -rn` los lectores de `"juez-diseno"`** en `src/`, `tests/` y `tools/`. Si alguno filtra eventos por esa fase, detenete y anotalo en el informe.
3. **`CORRIDA_JUEZ_DISENO`, de `src/oracle/mod.rs`, no cambia.** Está fuera de tu lista.

**Tests en rojo primero:**
- Unitario: el `Prompt` que arma `construir_prompt` lleva la fase `"juez"`. Hoy lleva `"juez-diseno"`.
- Guarda: el gasto de S2 cuenta la llamada igual. La regla P112 a mira `via`, no la fase.

### 3. Las huellas

Solo tests, en `src/oracle/calibra.rs`. Calculá cada literal sobre la base, antes del punto 1.

**Guardas:**
- `huella_de` da, para Funciona, Seguridad, Completitud, Pruebas, Eficiencia y Entrega, los mismos valores que en la base.
- `huella_de` da, para Arquitectura, Diseño y Estructura, valores distintos de los de la base.
- Una fila de Diseño con la huella de la base ya no calibra: `Version::calibrada` no la cuenta.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de `laptop/tarea-169`.
- La medición real de Diseño, Arquitectura y Estructura con la versión nueva. Esa medición dice si el arreglo sirve.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Oráculo.** Ningún test lleva nombres de tests del dataset ni contenido de `oraculos/`.
- **Claves.** Ninguna clave va en una línea impresa, en un argumento ni en un archivo. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-169.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las huellas**: las nueve, de la base y del final, con sus 8 primeros caracteres.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`, y los lectores de `"juez-diseno"`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
