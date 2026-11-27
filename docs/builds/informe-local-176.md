# Informe de la Tarea 176

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (`laptop/cadena-175`, 23e80ce) | 4258 | 0 | 5 |
| Final (`laptop/tarea-176`) | 4271 | 0 | 5 |

Los 13 tests nuevos son 11 en `src/corre/bwrap.rs` y 2 en `src/colab_corre.rs`. Ningún test de podman corrió.
El commit del informe falló una vez en el hook por `oracle::contenedor::tests::bwrap_ejecutar_lanza_el_programa_con_los_argumentos_de_bwrap` (test ajeno, de tiempo). Reintenté una vez y pasó.
El primer commit del punto 1 falló por un lint de clippy (`double_ended_iterator_last`). Cambié `.last()` por `.next_back()` y el commit pasó.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | bwrap monta siempre `--proc /proc`, que falla en Colab | No compilaban (E0061, E0425, E0599, E0560): `proc_builder_monta_el_procfs_en_solo_lectura`, `tipo_de_montaje_*`, `revisar_proc_builder_*`, `proc_builder_con_mountinfo_malo_frena_el_chequeo`, `aviso_proc_solo_con_proc_builder`, `desde_entorno_lee_proc_builder`, `desde_entorno_proc_builder_vacia_es_error`. Guardas: `sin_proc_builder_monta_proc_propio`, `unshare_pid_con_y_sin_proc_builder`, `montajes_son_los_de_la_base`, `montajes_con_proc_builder_son_los_de_la_base` | fd0ae0f |
| 2 | `corre-runtime.py` no fija `ROOT_PROC_BUILDER` | `corre_runtime_pasa_al_r00t_proc_builder_en_proc` (left `""`, right `"/proc"`). Guarda: `corre_runtime_el_resto_del_entorno_del_hijo_es_el_de_la_174` | a36d3d0 |

Nota: en el punto 1 los tests nuevos usan funciones y un campo que no existían. Por eso el rojo fue de compilación y no de aserción.

## 3. Qué falló o quedó incompleto

- **El aviso en el veredicto no está cableado.** El brief pide "con la variable, el veredicto trae el aviso". El veredicto se arma en `src/corre/corrida.rs` (`anotar_ejecutor`, `AVISO_BWRAP`). Ese archivo no está en la tabla. Me detuve y lo anoto.
  - Lo que sí hice en `bwrap.rs`: `AVISO_PROC_BUILDER` (`pub(crate)`) y `EjecutorBwrapBuilder::aviso_proc()`. Un test prueba que dan el texto con la variable y `None` sin ella.
  - Falta: que `corrida.rs` llame a `aviso_proc()` y empuje el aviso al veredicto. El test "el veredicto trae el aviso" no existe todavía.
- `tipo_de_montaje` no decodifica escapes octales de `mountinfo` (`\040`). Una ruta con espacios no coincidiría. `/proc` no lo necesita.
- Ningún test monta nada ni corre `bwrap` real con `--ro-bind /proc`. La prueba real es la de aislamiento en Colab, que corre el orquestador.
- No corrí la suite con podman, ni la validez, ni `corre`, ni `calibrar`.

## 4. El `/proc` del builder

- **Laptop con podman:** no cambia. Esta tarea no toca `EjecutorContenedorBuilder`. El contenedor sigue con su procfs propio.
- **Colab con bwrap y `ROOT_PROC_BUILDER=/proc`:** el builder tiene su espacio de pids (`--unshare-pid`) y ve el `/proc` del runtime en solo lectura (`--ro-bind /proc /proc`).
  - Por P133 b ve la lista de procesos y sus `cmdline`.
  - Por diseño del kernel (según el brief) no lee `environ`, `fd/`, raíz ni memoria de procesos de root, ni les manda señales.
  - Esto no lo probé aquí: lo mide `revisa.py` en modo `b`.
- **Sin la variable:** los argumentos quedan como antes, con `--proc /proc`.

## 5. Tests que cambiaron de valor

- Ningún test cambió de valor esperado.
- Los 8 tests existentes de `bwrap.rs` que llaman a `args_bwrap_builder` ganaron el argumento `None`. El literal de `ejecutor_de` ganó `proc_builder: None`. Son cambios de firma, no de valor.
- No edité tests de otros archivos.

## 6. Llamadores revisados

No usé `codegraph`: en Colab no está instalado. Usé `grep -w`.

- `args_bwrap_builder` (firma nueva con `proc_builder: Option<&Path>`): es privada. Llamadores: `armar_comando` y los tests de `bwrap.rs`. Todos actualizados.
- `EjecutorBwrapBuilder` (campo nuevo `proc_builder`): los literales del struct están solo en `desde_entorno` y en `ejecutor_de` de los tests. Los demás usos (`src/corre/corrida.rs`, `src/corre/mod.rs`) lo arman por `desde_entorno` o lo nombran sin literal.

## 7. Preguntas para el planner

1. **¿Se autoriza tocar `src/corre/corrida.rs` para cablear el aviso?** Decisión del organizador: no lo toqué, por la tabla del brief. Dejé `aviso_proc()` listo. El cableado son unas 3 líneas en `elegir_ejecutor` o `anotar_ejecutor`, más un test. Hace falta una tarea corta o una ampliación de la tabla.
2. **¿Qué hace el aviso si el ejecutor se elige con un `ROOT_PROC_BUILDER` vacío?** Decisión: `desde_entorno` da error de infraestructura que nombra la variable, como con `ROOT_TOKEN_BUILDER` vacío.
