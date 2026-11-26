# Informe de la Tarea 172

Base: 5a4f295. Rama: `laptop/tarea-172`. Sin push. No usé `codegraph`; los llamadores salen de `grep -w`.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (5a4f295) | 4098 | 0 | 5 |
| Final | 4144 | 0 | 5 |

Los tests que necesitan podman no corrieron. Ningún test nuevo usa podman. El orquestador corre la suite con podman al mezclar.
Los commits pasaron el hook completo al primer intento. No hubo reintentos por tests de tiempo.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1. Paquete de `corre` | `--corre` no existía | 17 de `colab_corre`, rojos por `todo!()`: oráculos de las tareas pedidas, `.env` y `.credentials.json` fuera, `sk-` frena, manifiesto en orden, tarea inexistente, plugin, versiones. Guardas sin rojo previo: 5 de `cli.rs` | 9d70c79 |
| 2. Runtime de `corre` | No había guion de preparación ni de corrida | 9, rojos porque los guiones no existían: salta hechas, sigue tras código 1, no filtra clave ni token, borra el token, corta por plazo, estado final, versión de `claude`, falta `red-builder.sh`, camino feliz | 957d33e |
| 3. Importación | `--corre` no existía en `importar` | 11 de `importar_corre_*`, rojos por `todo!()`. Guardas sin rojo previo: 4 de `cli.rs` | 46bc21b |
| Docs | `docs/interfaces.md` lista los comandos | Sin test nuevo. `tests/interfaces.rs` sigue verde | commit de docs |

Notas de honestidad sobre el rojo:
- Los tests de `cli.rs` de los puntos 1 y 3 nacieron junto con los campos de clap. No los vi en rojo. Son guardas.
- En el punto 2, un helper de Python mal escrito saltó los tests en silencio. Se corrigió y se vio el rojo real.
- En el punto 3, el rojo es por `todo!()`. No es rojo de lógica.

## 3. Qué falló o quedó incompleto

- **El paquete no trae lo que `preparar-runtime.sh` exige.** Ese guion pide `versiones.env` y `herramientas/`. El paquete de `corre` no los lleva, como manda el brief. En Colab, `preparar-corre.sh` falla en el paso 1 hasta que el orquestador agregue esos archivos a la carpeta. Es la pregunta 1.
- **`red-builder.sh` no existe** en esta base. Es de la Tarea 171. `preparar-corre.sh` termina con error claro si falta. El camino feliz solo se probó con un falso.
- **Sin probar en Colab:** `preparar-corre.sh` con apt y npm reales, `bwrap` y la prueba de aislamiento. Lo hace el orquestador.
- **Sin test:** el rechazo de rutas con `..` en la vuelta. No pude armar ese tar sin trucos.
- **Sin lock en `colab-corre.json`.** Dos importaciones a la vez pueden perder una entrada. Si falla la copia de transcripts después de agregar la fila, un reintento duplica la fila. Es la pregunta 6.

## 4. El paquete

Entra:
- `r00t`, con su SHA-256 en el manifiesto;
- `GOAL.md` y `tools/colab/`;
- `versiones-builder.env`, con `CLAUDE_VERSION` y `RUST_VERSION`;
- `manifiesto.json`: pasada, commit, config, tareas en orden, plugins y SHA de cada archivo;
- `root-home/`: `root.toml` (revisado), `configs/<config>.toml`, `calibracion.json`, `calibracion/`, `eventos.jsonl`, `oraculos/<tarea>/` de cada tarea pedida;
- `plugins/<n>-<nombre>/`, uno por cada `plugin_dir` de la config.

Nunca entra:
- claves, tokens, `.credentials.json`, `.env*`;
- el `oraculos/` de una tarea no pedida;
- cualquier otro archivo de la raíz de `ROOT_HOME`;
- enlaces simbólicos (frenan el empaquetado).

La revisión final recorre el paquete armado, salvo `r00t`. Frena `.env*`, `.credentials.json` y un texto con `sk-` y 16 caracteres o más de clave. El error nombra la ruta y nunca el valor.

## 5. Tests que cambiaron de valor

Ninguno. No edité tests de otros archivos. Agregué tests en `src/colab_corre.rs` y `src/cli.rs`.
Los tests de los guiones del punto 2 viven en `src/colab_corre.rs`, porque la tabla no permite un archivo de tests nuevo.
En `cli.rs`, `rustup_sha256` pasó a `Option<String>` y `archivo` de `importar` a `Option<PathBuf>`. Ningún test existente dejó de compilar.
Edité `docs/interfaces.md` (dos filas nuevas), porque el brief pide regenerar la lista de comandos. No está en la tabla. Es la pregunta 7.

## 6. Llamadores revisados

- `ColabAccion::Empaquetar` (campos nuevos y `rustup_sha256` opcional): `grep -w` en `src/cli.rs` y `src/lib.rs`. Los tests existentes usan `..`.
- `ColabAccion::Importar` (`archivo` opcional, campo `corre`): `src/cli.rs` y `src/lib.rs`. Solo cambió el despacho.
- `colab_empaquetar_con`: el código de impresión pasó a `imprimir_paquete`. Sin cambio de salida. Sus tests siguen verdes.
- `src/colab_calibra.rs` no cambió.

## 7. Preguntas para el planner

1. **Falta `versiones.env` y `herramientas/` en el paquete de `corre`.** Decisión: no los agregué. `preparar-corre.sh` llama a `preparar-runtime.sh` como manda el brief. Hay que decidir si el paquete los lleva, o si el orquestador los copia desde un paquete de calibración.
2. **Plugins de la config.** La config trae rutas absolutas de la laptop. Decisión: el paquete copia cada plugin y el manifiesto mapea la ruta original a la del paquete. `preparar-corre.sh` lo copia a la misma ruta absoluta en Colab. La config queda igual, byte a byte.
3. **Nombres de entorno.** El brief no los da. Decisión: `DEEPSEEK_API_KEY` para la clave y `ROOT_TOKEN_CUPO` para el token. El hijo recibe `ROOT_TOKEN_BUILDER` con la ruta.
4. **Qué entra de una vuelta cortada.** Decisión: solo los eventos de gasto y el registro. No entran la fila ni los transcripts. Una completa entra con todos sus eventos, marcados `origen: colab`. `corre` no marca ese origen, así que lo puse al importar.
5. **Clasificación.** Una vuelta es completa si hay fila con veredicto, no hay plazo y no hay `Gate` de cupo agotado. El código de salida de `r00t` no cuenta. Una vuelta con más de una fila de la tarea da error de uso.
6. **Atomicidad del registro.** Decisión: sin lock y sin deshacer. Si querés, una tarea posterior le agrega lock.
7. **`docs/interfaces.md`.** Decisión: lo edité aunque no está en la tabla. Dos filas nuevas.
8. **Plazo.** El corte de 7200 s es por tarea. El guion sale con 0 aunque una tarea falle. Los fallos quedan en `hechos/<t>.json`.
9. **Falsos positivos de `sk-`.** Los plugins de ECC pueden traer ejemplos con `sk-` y 16 caracteres. Si pasa, el empaquetado frena con la ruta. Decidí no relajar la regla. El orquestador lo ve en el paquete real de 1 tarea.
10. **`eventos.jsonl` entra completo** al paquete, como pide el brief. Puede traer datos de gasto de otras tareas.
