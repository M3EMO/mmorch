# Brief de la Tarea 80 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` (8bd12bb).
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-80`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

Los dos puntos no comparten archivos. El organizador puede correrlos en paralelo.

| Punto | Podés tocar |
|---|---|
| 1, huella (f) | `src/observe/transcript.rs`, `src/corre/corrida.rs` (solo el aviso del paso 1.4) y `tests/transcript.rs` |
| 2, especificidad (g) | `src/oracle/calibra.rs`, `src/lib.rs` (solo la línea de texto de `calibrar`) y `docs/interfaces.md` |

- **No toques:** el resto de `src/`, `tests/` fuera de la tabla, y `$ROOT_HOME/`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

**(f) Huella falsa.** En S2, `r00t corre` de t1 terminó "inválida" por `oraculo_leido` con motivo "huella". La corrida era correcta.
- `HuellasOraculo::armar` (`src/observe/transcript.rs:176`) descarta una línea oculta solo si aparece en `contrato/`.
- En S2, `contrato/` está vacía. Así que no descarta nada.
- La línea `let search_modes = SearchModeMap::default();` es oculta, y también está en `benches/composite.rs`, que el builder ve.
- El builder corrió `cat benches/composite.rs`. La huella disparó 2 veces.
- El orquestador lo reprodujo en Python con `~/.cache/root-wt/mineria/huella_f.py`. Con la regla nueva, t1 queda con 5 huellas y 0 disparos. t2 queda en 11 y t3 en 6, sin cambio.

**(g) Especificidad engañosa.** Un caso limpio sin veredicto cuenta hoy como "no detectado" (`fila_de`, `src/oracle/calibra.rs:896`). Una fila con un solo limpio sin veredicto muestra especificidad 1,0. Eso no es una medición.

## Tarea

La regla que manda: **lo que el builder ya ve no es una huella, y una métrica sin casos con veredicto no tiene valor.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Huella (f)

1. **Estado visible.** `armar` lee también `oraculos/<tarea>/bootstrap/`, si existe. Saltá `.git/` y `target/`. Leé con `leer_lossy`, como hoy.
2. **Líneas.** Una línea oculta deja de ser huella si su texto recortado aparece como subcadena en `bootstrap/` o en `contrato/`. Es la misma regla de subcadena que ya usa `contrato/`.
3. **Nombres.** Un nombre de test oculto deja de ser huella si aparece en `bootstrap/` con `aparece_bordeado`, igual que hoy con `contrato/`.
4. **Aviso.** Si después del filtro no queda ninguna línea, `corrida.rs` escribe en el log de la corrida: "huella sin líneas distintivas". Las rutas y los nombres siguen valiendo.
5. **Sin `bootstrap/`,** el resultado es igual al de hoy.

**Cuidado.** El estado visible correcto es `oraculos/<tarea>/bootstrap`. El repo de trabajo de la minería ya trae los tests ocultos, y no sirve como estado visible.

**Tests en rojo primero:**
- Unitario: una línea oculta que también está en `bootstrap/benches/composite.rs` no queda en `lineas`.
- Unitario: un nombre de test oculto que aparece en `bootstrap/` no queda en `nombres`.
- Guarda: una línea oculta única sigue en `lineas`.
- Guarda: sin `bootstrap/`, las huellas son las de hoy.

**Test de integración, en `tests/transcript.rs`.** Es una copia mínima del caso de t1.
- El fixture arma `oraculos/<tarea>/oculto/` con dos líneas largas: `let search_modes = SearchModeMap::default();` y otra única.
- `oraculos/<tarea>/bootstrap/benches/composite.rs` contiene la primera línea.
- `contrato/` existe y está vacía.
- Un transcript con `llamada_bash` de `cat benches/composite.rs`, y su `tool_result` con esa línea, no da `oraculo_leido`.
- Un segundo transcript cuyo `tool_result` trae la línea única sí da `oraculo_leido` con motivo "huella".

**Test de aceptación (vuelta 71):** la línea oculta que aparece en `benches/` no dispara. La línea oculta única sí dispara. La detección offline sobre el transcript guardado de t1 la corre el orquestador después de mezclar.

### 2. Especificidad (g)

1. **Cálculo.** En `fila_de`, la especificidad se calcula solo sobre los limpios con veredicto. Con cero limpios con veredicto, queda `None`.
2. **Exactitud.** `exactitud_balanceada` sigue saliendo de `exactitud_de`. Con especificidad `None`, también queda `None`.
3. **Texto.** La línea de `calibrar` en `src/lib.rs` suma "limpios con veredicto k de n". `medida(None)` ya imprime "sin medir".
4. **Esquema.** No cambia: `limpios` y `limpios_sin_veredicto` ya existen. Corregí el comentario de `limpios_sin_veredicto`, que hoy dice que entran a `especificidad` como no detectados.
5. **`MAX_LIMPIOS_SIN_VEREDICTO_PCT`** no cambia. `aplicar` sigue frenando con más de 20 %.

**Tests en rojo primero:**
- Unitario: 1 limpio sin veredicto da especificidad `None`, y el texto dice "sin medir" y "0 de 1". Hoy da 1,0.
- Unitario: 2 limpios, 1 con veredicto y no detectado, y 1 sin veredicto, da especificidad 1,0 sobre 1 caso. Con el caso con veredicto detectado, da 0,0.
- Guarda: sin limpios sigue dando `None` (`calibra_sin_casos_limpios_deja_especificidad_none`).
- Guarda: los tests de detector perfecto y nulo siguen verdes. Si uno usa limpios sin veredicto y cambia, explicalo en el informe.

**Test de aceptación (vuelta 71):** 1 limpio sin veredicto da "sin dato (0 de 1)", y no 1,0. Con 1 de 2 con veredicto, la especificidad usa solo ese 1.

**Efecto sobre filas guardadas.** Las filas ya escritas no se recalculan. Una calibración nueva puede cambiar su exactitud si tiene limpios sin veredicto. Anotá en el informe qué tests cambiaron de valor.

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

Al terminar, escribí `docs/builds/informe-local-80.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor** por el punto 2, con su motivo.
5. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
