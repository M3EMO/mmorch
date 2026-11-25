# Brief de la Tarea 170 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (95) y (96).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde 31965b2, en `claude/root-e7813e`.** Es la mezcla de las Tareas 167 a 169, y el congelado X.
- Trabajá en una rama nueva: `laptop/tarea-170`.
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

## Qué es esta tarea

**El usuario eligió volver al prompt del juez de 2020ecc (P127 a).** El prompt de la Tarea 169 estabilizó la base, pero el juez empezó a bajar niveles sobre código limpio. La especificidad de Arquitectura en s2-t1 cayó de 1,0 a 0,67.

**Esta tarea cambia lo que Root mide, solo para volver atrás.** Con el prompt de 2020ecc, las huellas de Arquitectura, Diseño y Estructura vuelven a ser las de 2020ecc. Las filas medidas con 2020ecc vuelven a contar. Estructura vuelve a estar calibrada, sin gasto.

**Lo que queda de la 169:** la fase `"juez"` de los eventos. La fase no entra a la huella.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el prompt de 2020ecc | `src/oracle/juez.rs` |
| 2, las guardas de las huellas | solo tests: `src/oracle/calibra.rs` y `src/colab_calibra.rs` |

- Los dos puntos van en serie: primero el 1.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** el resto de `src/oracle/`, `src/proveedores/`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema,** y **sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- La tarea no necesita podman, claves de API reales ni el `ROOT_HOME` real.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a ningún builder real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá con `grep -rn`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **el prompt fijo del juez es el de 2020ecc, byte por byte, y las nueve huellas son las de 2020ecc.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El prompt de 2020ecc

1. **Tomá el texto de 2020ecc:** `git show 2020ecc:src/oracle/juez.rs`. Copiá su `PROMPT_SISTEMA` entero sobre el de hoy.
2. **El cambio de hoy tiene tres partes. Las tres vuelven atrás:**
   - la línea `REFUTA POR DEFAULT: ningún nivel vale sin una cita textual que cumpla su ancla.` vuelve a ser `REFUTA POR DEFAULT: el nivel de cada ítem parte en 0 y solo sube con evidencia citada textual.`;
   - salen las tres líneas que empiezan con `Para cada ítem, buscá primero`, `Revisá las anclas en orden` y `Si el código del tema está`;
   - la línea que empieza con `Solo si ningún extracto trae código del tema` vuelve a ser la de 2020ecc, que empieza con `Sin evidencia para un ítem`.
3. **`MARCA_RECORTE` y `VERSION_LECTURA` no cambian.** Son los de 2020ecc.
4. **`FASE` queda en `"juez"`.**
5. **Los tests de la 169 sobre el texto del prompt se dan vuelta:** `juez_p169_huella_cambia_respecto_de_la_base`, `juez_p169_sistema_no_dice_parte_en_0` y `juez_p169_sistema_dice_escala_de_0_a_3_y_regla_del_tema`. Renombralos con el prefijo `juez_p170_`.

**Tests en rojo primero:**
- Unitario: `huella_del_prompt()` es igual a `"9263145aa4df4c7a"`, la de 2020ecc. Hoy es distinta.
- Unitario: el prompt del sistema contiene "parte en 0". Hoy no lo contiene.
- Unitario: el prompt del sistema no contiene "de 0 a 3" ni "Solo si ningún extracto trae código del tema".
- Guarda: el `Prompt` que arma `construir_prompt` sigue llevando la fase `"juez"`.
- Guarda: los otros tests del juez de la 169 siguen verdes: el nivel 3 con cita real, el nivel 3 sin cita, `sin_evidencia`, la respuesta truncada y el prompt determinista.

### 2. Las guardas de las huellas

1. **En `src/oracle/calibra.rs`,** `huella_p169_las_tres_del_juez_cambian` pasa a pedir igualdad. Arquitectura da `"60c5fb9b0d576398"`, Diseño da `"19b81dea4119d429"`, y Estructura da `"afbc695e7b008336"`. Renombralo `huella_p170_las_tres_del_juez_son_las_de_2020ecc`.
2. **`huella_p169_fila_de_diseno_con_la_huella_de_la_base_no_calibra` se da vuelta:** una fila de Diseño con `"19b81dea4119d429"` calibra otra vez. Una fila con la huella de la 169, `"9b2a621ebb5c4632"`, no calibra. Renombralo con el prefijo `huella_p170_`.
3. **En `src/colab_calibra.rs`,** `huellas_de_las_nueve_dimensiones_son_fijas` vuelve a los literales de 2020ecc en las tres dimensiones del juez. Las seis de oráculo no cambian.
4. **`huella_p169_las_seis_de_oraculo_no_cambian` no cambia.**

**Tests en rojo primero:** los tres tests de los puntos 2.1 a 2.3 fallan sobre la base, antes del punto 1. Después del punto 1, pasan.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **La suite con podman** de `laptop/tarea-170`.
2. **El diff contra 2020ecc:** `git diff 2020ecc laptop/tarea-170 -- src/oracle/juez.rs` muestra solo la línea de `FASE` y tests. La zona de `PROMPT_SISTEMA` no aparece.
3. **El diff contra X:** `git diff 31965b2 laptop/tarea-170 --stat` toca solo `src/oracle/juez.rs`, `src/oracle/calibra.rs`, `src/colab_calibra.rs` y el informe.
4. **Después de la mezcla y del congelamiento Y,** el estado sobre una copia del real:
   - Funciona, Seguridad, Eficiencia, Completitud y Pruebas, calibradas;
   - Estructura, calibrada con sus 3 filas de P107 a;
   - Arquitectura y Diseño, sin calibrar por sus números, no por "calibración de otro juez";
   - las 2 filas de juez-x ya no cuentan.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave va en una línea impresa, en un argumento ni en un archivo.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-170.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las huellas**: las nueve, de la base y del final, con sus 16 primeros caracteres.
5. **Tests que cambiaron de valor o de nombre**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
