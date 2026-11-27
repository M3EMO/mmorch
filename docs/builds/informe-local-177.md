# Informe de la Tarea 177

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (ae50e34) | 4277 | 0 | 5 |
| Final (37ad08c) | 4287 | 0 | 5 |

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `/opt` en `RAIZ_SOLO_LECTURA` impedía crear `/opt/plugin` | `corre_bwrap_con_opt_no_monta_opt_y_el_plugin_va_a_opt_plugin`. Guardas: `corre_bwrap_con_opt_sigue_montando_usr_y_etc_en_solo_lectura`, `corre_args_claude_con_plugin_traen_plugin_dir_en_opt_plugin` | e8fb3fd |
| 2 | Nada revisaba los destinos de los montajes | `revisar_destinos_destino_dentro_de_carpeta_ro_falla_y_nombra_ambas`, `revisar_destinos_bind_dentro_de_usr_tras_ro_bind_de_usr_falla`, `corre_bwrap_conflicto_de_destinos_frena_antes_de_lanzar_bwrap`. Guardas: `revisar_destinos_args_reales_del_builder_pasan`, `revisar_destinos_ro_bind_de_usr_solo_no_choca`, `revisar_destinos_solo_cuenta_carpetas_montadas_antes_del_destino`, `corre_bwrap_guarda_montajes_de_trabajo_estado_credencial_y_plugin` | 37ad08c |

Reintento del hook: el primer commit de cada punto falló por formato (punto 1) y por clippy (punto 2, `type_complexity` y un lifetime). Los arreglé y reintenté. No hubo fallas de tiempo.

## 3. Qué falló o quedó incompleto

Nada quedó sin hacer. No corrí podman, Colab, `claude` ni `bwrap` reales. La prueba real de aislamiento la corre el orquestador.

## 4. Los montajes del builder

En el orden de `args_bwrap_builder`:

| Origen | Destino | Modo |
|---|---|---|
| `/usr` (si existe) | `/usr` | solo lectura |
| `/etc` (si existe) | `/etc` | solo lectura |
| `/bin`, `/lib`, `/lib64`, `/sbin` | igual | solo lectura si son carpeta; `--symlink` si son enlace |
| `ROOT_PROC_BUILDER` o procfs propio | `/proc` | solo lectura / `--proc` |
| devtmpfs | `/dev` | `--dev` |
| tmpfs | `/tmp` | `--tmpfs` |
| repo | `/trabajo` | escritura |
| estado | `/home/builder/.claude` | escritura |
| copia de la credencial (sin token) | `/home/builder/.claude/.credentials.json` | solo lectura |
| plugin (si hay) | `/opt/plugin` | solo lectura |

`/opt` ya no se monta. bwrap crea `/opt/plugin` sobre su raíz escribible.

## 5. Tests que cambiaron de valor

Ningún test cambió de valor. Los 4 tests que llaman a `armar_comando` cambiaron de firma: reciben los montajes armados y esperan un `Result`. Están en `src/corre/bwrap.rs`. No edité tests de otros archivos.

## 6. Llamadores revisados

Busqué con `grep -w`. No usé `codegraph`.

- `armar_comando` (firma cambiada): `lanzar_con` y 4 tests en `bwrap.rs`. Todos actualizados.
- `lanzar` ahora delega en `lanzar_con`. Su firma pública no cambió.
- `RAIZ_SOLO_LECTURA`: solo `args_bwrap_builder` la usa.

## 7. Preguntas para el planner

1. El brief define las carpetas de solo lectura como las de `RAIZ_SOLO_LECTURA` y `RAIZ_ENLACES`. Pero sus tests 1 y 5 usan `--ro-bind /opt /opt`, y `/opt` ya no está en esas listas. Decisión: `revisar_destinos` trata cualquier `--ro-bind X X` como carpeta de solo lectura. Los montajes reales no cambian. ¿Se prefiere la regla literal con las listas?
2. La revisión corre dentro de `armar_comando`, después del `chown` de repo y estado y de crear `cargo-home`. Un conflicto frena antes de bwrap, pero esos cambios ya ocurrieron. Decisión: se deja así, porque son idempotentes.
