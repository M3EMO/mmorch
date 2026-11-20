# Brief de la Tarea 167 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (90) y (91).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde 2020ecc, en `claude/root-e7813e`.** Es el commit de la mezcla de las Tareas 110 a 166, y el commit congelado.
- Trabajá en una rama nueva: `laptop/tarea-167`.
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

## El congelamiento

**El binario congelado es el de 2020ecc, y esta tarea no lo reemplaza.** Lo que mide es el binario que viaja **adentro** de cada paquete, con `--binario`. Ese sigue siendo el congelado.

**El binario de esta tarea solo corre en la laptop:** arma el paquete del juez con el tope nuevo, y maneja el transporte de `colab calibrar`. No mide nada.

**Por eso esta tarea no toca nada de lo que mide:** ni rúbricas, ni el prompt del juez, ni sondas, ni `src/oracle/`, ni `tools/colab/`.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el tope de S2 en 30 USD | `src/colab_calibra.rs` |
| 2, ningún exec largo en el transporte | `src/colab_calibra.rs` |

- Los dos puntos comparten archivo. Van en serie: primero el 1.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `tools/`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema,** ni del manifiesto del paquete.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- La tarea no necesita podman, claves de API reales ni el `ROOT_HOME` real.
- **No llames a podman real, a la CLI `colab` real ni a ningún builder real.** Los tests usan el `ColabCli` falso.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`. El orquestador lo corre al revisar.

## Por qué

**1. El usuario subió el tope de S2 a 30 USD (P124 a).** P107 a cuesta de 13,5 a 17 USD, y el saldo con el tope de 25 era de 18,03 USD. Antes de cada caso, Root reserva el peor caso. Si no entra, la calibración se detiene y sus filas quedan incompletas, sin contar. `TOPE_S2_USD`, en `src/colab_calibra.rs`, vale 25.0 por P107 a.

**2. Tres execs largos o sin reintento cortaron runtimes enteros.** Las doce calibraciones corren en tandas, con varios runtimes a la vez. Tres runtimes se perdieron, cada uno en un paso distinto:
- **la preparación:** "colab superó 3900 s". Es `SEG_PREPARAR_CALIBRAR`, 3600, más `MARGEN_EXEC`, 300. El trabajo nunca se lanzó.
- **el lanzamiento:** "colab superó 180 s". Es `PLAZO_LOCAL_TRABAJO`, en el exec de `lanzar-N.py`. Ese guion tarda menos de un segundo, y no se reintenta.
- **el cierre:** "colab superó 900 s". Es `SEG_CIERRE_CALIBRAR`, 600, más el margen. El trabajo ya había terminado con código 0, y su resultado se perdió.

Es el mismo defecto de la 157c: el cliente de `colab exec` a veces no vuelve, más con varias sesiones a la vez. La 161 sacó del exec al trabajo, con el patrón de lanzar aparte y consultar el estado. La preparación y el cierre siguen siendo execs largos, y el lanzamiento no tolera una falla.

## Tarea

La regla que manda: **ningún paso del runtime depende de que un exec largo vuelva, y un exec corto que falla se reintenta sin repetir el trabajo.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El tope de S2 en 30 USD

1. **`TOPE_S2_USD` pasa a 30.0.** Su comentario dice: "P124 a del usuario: el tope de S2 sube de 25 a 30 USD".
2. **Los textos que nombran el tope** lo leen de la constante, como hoy. No escribas 30 en ningún texto.
3. **Los tests que suponen 25** pasan a 30: por ejemplo, el saldo con 6,55 USD gastados pasa de 18,45 a 23,45, y el tope agotado del test de la 153 pasa de 25 a 30 USD gastados. Anotá cada uno.

**Tests en rojo primero:**
- Unitario: con 6,97 USD gastados, `tope_s2_restante` da 23,03. Hoy da 18,03.
- Unitario: con 30 USD gastados y `--juez`, el empaquetado falla con el tope agotado. Con 29, no falla.
- Guarda: la línea "gasto de S2" dice "de 30".

### 2. Ningún exec largo en el transporte

1. **Una sola función corre un guion aparte en el runtime y consulta su estado.** Es la que la 161 armó para el trabajo, generalizada. La usan tres pasos: la preparación, cada trabajo y el cierre.
2. **Cada paso deja su propio archivo de estado,** escrito en un temporal y renombrado: `preparar.estado`, `trabajo-N.estado` y `cierre.estado`. Cada uno trae el código, y la hora de inicio y de fin.
3. **El estado del cierre trae también `ROOT_SHA_VUELTA`.** Root lo compara con el tar bajado, como hoy.
4. **El lanzamiento es idempotente.** Si ya existe el archivo de pid o el de estado de ese paso, el guion de lanzar devuelve ese pid y no lanza otro proceso.
5. **El lanzamiento se reintenta hasta 3 veces.** Gracias al punto 4, un reintento nunca corre el paso dos veces.
6. **Las consultas toleran 5 fallas seguidas,** como hoy en el trabajo. El motivo de la última falla va en el mensaje, como desde la 162.
7. **Los plazos totales no cambian:** `SEG_PREPARAR_CALIBRAR`, `SEG_TRABAJO_CALIBRAR` y `SEG_CIERRE_CALIBRAR`, recortados por `lim.restante()`.
8. **La subida en trozos y la bajada de la vuelta** no cambian. Ya son llamadas de un solo archivo, con su plazo.
9. **El formato del paquete de ida y el de la vuelta no cambian.** Un paquete armado con el binario congelado corre con el binario de esta tarea.

**Tests en rojo primero:**
- Unitario: con el `ColabCli` falso, la preparación se lanza aparte y se consulta. Ningún exec de la preparación usa `--timeout 3600`. Hoy usa 3600.
- Unitario: lo mismo con el cierre, sin `--timeout 600`.
- Unitario: el primer exec de lanzar no vuelve en su plazo, y el segundo devuelve el pid del primero. El paso corre una sola vez.
- Unitario: con 3 lanzamientos fallidos, el error nombra el paso y dice "3 intentos".
- Unitario: el estado del cierre trae `ROOT_SHA_VUELTA`, y Root lo compara con el tar.
- Integración, con `python3`, como en la 162: los guiones de lanzar y de estado de cada paso corren en un temporal, con un comando falso, y dan `corriendo`, `fin` y `murio`. El lanzar repetido devuelve el mismo pid.
- Guarda: un paquete armado con el binario de la base corre con el binario de esta tarea, en el `ColabCli` falso.
- Guarda: los tests de la 161 y de la 162 siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de `laptop/tarea-167`.
- Una tanda real de `colab calibrar` con el binario de esta tarea como transporte, y los paquetes de siempre.
- El paquete del juez armado con el binario de esta tarea y `--binario` en el congelado.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave va en una línea impresa, en un argumento ni en un archivo. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-167.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los pasos del runtime**: cada paso, cómo se lanza y cómo se consulta ahora.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
