# Informe de la Tarea 152

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (fb9baf6) | 3831 | 0 | 5 |
| Final (26090da) | 3836 | 0 | 5 |

Los 5 tests nuevos suman 4 en `contenedor.rs` y 1 en `colab_preparar.rs`. Un test viejo de `colab_preparar.rs` se reescribió.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | bwrap no crea `/oraculo/contrato` en una raíz de solo lectura | `bwrap_destinos_de_montaje_salen_de_la_raiz_runtime_o_de_un_tmpfs` (no compilaba), `bwrap_oraculo_destino_fuera_del_runtime_da_error_que_lo_nombra`, `bwrap_tmpfs_oraculo_va_antes_del_ro_bind_del_oraculo`. Guarda: `bwrap_guarda_argumentos_de_podman_literales_no_cambian` | f41feb3 |
| 2 | La lista de carpetas tenía una copia en el test | `comparar_lista_nombra_el_script_si_la_lista_difiere` (no compilaba). Guarda: `carpetas_del_runtime_son_los_montajes_de_bwrap` | 26090da |

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- Punto 2: la comparación de los guiones con la constante del crate no tuvo rojo natural. Las listas de hoy coinciden. La guarda con texto falso sí tuvo rojo.
- No se probó nada con podman ni en Colab. Lo corre el orquestador al mezclar.
- Ningún test usa `saltar_sin_podman`: los tests nuevos no necesitan podman.
- Los dos commits pasaron el hook sin reintento.

## 4. Los destinos de montaje

Valores de `oraculo_destino` en código de producción:
- `src/oracle/mod.rs:3244`: `/oraculo/contrato` o `/oraculo/oculto` (`DIR_CONTRATO`, `DIR_OCULTO`), o `None` si hay copia.
- `src/oracle/entrega.rs:1878`: `/oraculo/escenarios`.
- Los demás sitios dan `None`, que equivale a `/oraculo`.

Llamadas a `montaje_bwrap` en `argumentos_bwrap`: `/repo`, `/trabajo`, el oráculo (su destino o `/oraculo`), `/cache`, `/rustsec` y `/salida`. Antes del oráculo va `--tmpfs /oraculo`.

## 5. Tests que cambiaron de valor

- Ningún test bwrap de la 139 cambió. El `tmpfs` entra después de los primeros 9 argumentos que se comprueban.
- `carpetas_del_runtime_son_los_montajes_de_bwrap` (`tests/colab_preparar.rs`) lee ahora la constante del crate. Se quitó su chequeo de literales en `contenedor.rs`, porque la constante vive allí.
- Esa edición cae dentro de la tabla de archivos del punto 2. No se editó ningún test de otros archivos.
- `DIR_CONTRATO` y `DIR_OCULTO` se alcanzan desde los tests como `super::super::DIR_*`. No cambió su visibilidad.

## 6. Llamadores revisados

La firma de `argumentos_bwrap` no cambió. Gana un error nuevo (`Infra`) para destinos fuera de la lista.
- codegraph: 10 llamadores, todos en `contenedor.rs` (`ejecutar_bwrap` y los tests `bwrap_*`).
- grep -w: sin llamadores fuera de `contenedor.rs`.
- `CARPETAS_RUNTIME` es pública y nueva. Solo la usan `contenedor.rs` y `tests/colab_preparar.rs`.

## 7. Preguntas para el planner

1. `revisar_destinos_bwrap` acepta `/oraculo/../x`, porque empieza con `/oraculo/`. Decisión: no se cambió, porque el brief fija la regla. Rechazar componentes `..` cuesta una línea. ¿Se agrega?
2. Los comentarios de los guiones dicen que la lista debe coincidir con `contenedor.rs`. Siguen ciertos. Ninguno quedó falso.
3. `lista_de_carpetas` entra en pánico sin nombrar el guion si falta la línea `CARPETAS_RUNTIME=(`. Decisión: no se cambió, está fuera del pedido.
