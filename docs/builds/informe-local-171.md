# Informe de la Tarea 171

Rama: `laptop/tarea-171`, sobre 5a4f295. Se construyó en la laptop, sin podman, con `ROOT_SIN_PODMAN=1`.
No usé `codegraph`: busqué los llamadores con `grep -w`.

## 1. Resultado de la suite

Comando: `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`.

| | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 5a4f295 | 4098 | 0 | 5 |
| Final | 4138 | 0 | 5 |

- Hay 40 tests nuevos: 29 de `corre_bwrap_` en `bwrap.rs`, 6 en `corrida.rs` y 5 en `tests/red_builder.rs`.
- **Se saltaron 2 tests de bwrap real.** Motivo: en este equipo no está `bwrap`. Corren con aviso `SALTEADO sin bwrap usable`. Cuentan como verdes. El orquestador los corre en Colab.
- Ningún test crea un espacio de red ni corre `iptables`, `ip netns` o `slirp4netns` reales.
- El hook de pre-commit pasó en los tres commits al primer intento. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1. Ejecutor con bwrap | No existía `EjecutorBwrapBuilder` | 66 errores de compilación por símbolos que faltaban. Después, 4 fallos de ejecución por falta de `serie_de_pruebas()`, arreglados en los tests. Tests: `corre_bwrap_args_no_montan_root_home_home_root_ni_content`, `..._token_no_va_en_argv_y_si_en_el_entorno`, `..._entorno_del_command_sin_claves_...`, `..._root_home_con_modo_0755_frena_el_lanzamiento`, `..._builder_con_el_uid_de_r00t_frena`, `..._fallo_de_bwrap_es_ejecutor_y_no_salida`, `..._orden_del_comando_es_ip_prlimit_setpriv_bwrap_claude` | 4dffbe0 |
| 2. Elección del ejecutor | `corre` solo armaba el de podman | 6 errores de compilación (`elegir_ejecutor`, `anotar_ejecutor`). Tests: `corre_elegir_ejecutor_con_bwrap_arma_el_de_bwrap`, `..._bwrap_sin_red_da_error_de_ejecutor`, `corre_anotar_ejecutor_con_bwrap_deja_el_aviso_una_sola_vez` | 7b0eae6 |
| 3. Red del builder | No existía `red-builder.sh` | 5 tests fallaron: el guion no existía | 7591448 |

Guardas que siguen verdes: todos los tests de `EjecutorContenedorBuilder`, de `contenedor.rs` y de `corre::`.

## 3. Qué falló o quedó incompleto

- **Los dos tests con bwrap real no corrieron aquí.** No hay `bwrap`. Quedan para Colab.
- **No probé `setpriv`, `ip netns` ni `slirp4netns` reales.** Lo pide el brief. Lo prueba el orquestador.
- **El test del aviso "builder en bwrap" no pasa por `correr` completo.** Con bwrap, `correr` arma el ejecutor real y no se puede inyectar uno falso. Probé `anotar_ejecutor` y `elegir_ejecutor` por separado. También probé que sin bwrap la fila no lleva el aviso.
- **Con `ROOT_TOKEN_BUILDER`, `lanzar` aún exige `.credentials.json`** y monta su copia. Está en la especificación, pero en Colab puede no haber ese archivo. Ver pregunta 2.
- **`verificar_antes` y `vida_restante` del ejecutor bwrap usan el valor por defecto:** no revisan la vigencia del login. Podman sí. Ver pregunta 3.
- **Sin cgroups:** la memoria no tiene tope. El tope de procesos es `RLIMIT_NPROC` de 4096, por `prlimit`. Cuenta los procesos del uid del builder en todo el sistema.
- **La limpieza por señal** sigue llamando a `podman ps`. Sin podman en Colab, falla y solo imprime un aviso. La copia de la credencial se borra igual.
- Un subagente escribió un `/tmp/x` por error y lo borró. Verifiqué que no existe.

## 4. Podman y bwrap, lado a lado

| Aspecto | Podman | Bwrap |
|---|---|---|
| Comando | `podman run ... imagen claude` | `ip netns exec <red> prlimit --nproc=4096 setpriv ... bwrap ... claude` |
| Usuario | `--userns=keep-id` | `setpriv --reuid/--regid` del usuario `ROOT_USUARIO_BUILDER` |
| Capacidades | `--cap-drop=all` | `--inh-caps=-all --bounding-set=-all` |
| Privilegios nuevos | `no-new-privileges` | `--no-new-privs` |
| Procesos | `--pids-limit=4096` | `RLIMIT_NPROC` 4096 por `prlimit` (por uid, no por grupo) |
| Memoria | `--memory=8g` | **Sin tope** |
| Red | `pasta:--no-map-gw` | netns `ROOT_RED_BUILDER` con `slirp4netns` e `iptables` |
| Proxy | `--http-proxy=false` | No aplica |
| Espacios | contenedor | `--unshare-user/pid/ipc/uts/cgroup-try`, `--new-session`, `--die-with-parent` |
| Sistema | imagen `root-builder:1` | `--ro-bind` de `/usr`, `/etc`, `/opt`; enlaces `/bin`, `/lib`, `/lib64`, `/sbin` |
| `/proc`, `/dev`, `/tmp` | de la imagen | `--proc`, `--dev`, `--tmpfs` |
| Repo | `-v repo:/trabajo:rw,Z` | `--bind repo /trabajo` |
| Estado | `-v estado:/home/builder/.claude:rw,Z` | `--bind estado /home/builder/.claude` |
| Credencial | copia en `.credentials.json`, `ro,Z` | igual, `--ro-bind`; o `CLAUDE_CODE_OAUTH_TOKEN` por entorno si hay `ROOT_TOKEN_BUILDER` |
| Plugin | `ro,z` en `PLUGIN_EN_CONTENEDOR` | `--ro-bind` en el mismo destino |
| Directorio | `-w /trabajo` | `--chdir /trabajo` |
| Entorno | `-e CLAVE=valor` en argv | `env_clear()` y entorno heredado por bwrap. Nada en argv |
| `HOME` y `PATH` | los fija la imagen | `HOME=/home/builder`, `PATH=/usr/local/bin:/usr/bin:/bin` |
| Variables prohibidas | `VARIABLES_PROHIBIDAS` | las mismas |
| Plazo | `min(plazo, TIMEOUT_BUILDER)`; borra el contenedor | igual; mata el grupo de procesos |
| Fallo de infraestructura | códigos 125, 126, 127 | 126, 127, o código ≠ 0 con stdout vacío y stderr con prefijo `bwrap:`, `setpriv:`, `prlimit:`, `ip:` |
| SIGKILL (137) | `Cortado` con stdout | igual |
| Chequeo previo | credencial, plugin, montajes | además: red, usuario, uid distinto, `ROOT_HOME` 0700, `$HOME` no legible |

## 5. Tests que cambiaron de valor

- Ningún test existente cambió de valor.
- No edité tests de otros archivos.
- **Archivo nuevo fuera de la tabla:** `tests/red_builder.rs`. La tabla del brief solo nombra `tools/colab/red-builder.sh` para el punto 3, pero los tests del guion necesitan un archivo. Ver pregunta 5.

## 6. Llamadores revisados

Cambié la visibilidad (`pub(super)`) de `revisar_credencial`, `revisar_plugin`, `senal_termino_el_ejecutor`, `CODIGO_SIGKILL`, `CREDENCIAL_COPIA`, `ESTADO_EN_CONTENEDOR`, `CREDENCIAL_EN_CONTENEDOR` y `VARIABLES_PROHIBIDAS`. Ninguna firma pública cambió.

Funciones internas con cambio de estructura, con sus llamadores por `grep -w`:
- `correr_registrado` (mismo contrato): ahora delega en `correr_registrado_con`. Llamadores: `ejecutor.rs:241` (lanzar), `:591` y dos tests.
- `spawn_hijo` (mismo contrato): ahora delega en `spawn_con`. Llamador: `ejecutor.rs:593`.
- `correr_comando_registrado` y `spawn_con`: nuevas. Solo las usa `bwrap.rs`.
- `correr_interno` (`corrida.rs`): arma un `DepsCorre` local. Sus dos llamadores, `correr` y `correr_reanudando`, no cambian.

`src/lib.rs` sigue armando `EjecutorContenedorBuilder` y no se tocó. La elección vive en `correr_interno`.

## 7. Preguntas para el planner

1. **¿Son 7 reglas de corte o 6?** El brief dice 7 y lista 6 rangos. *Decisión:* implementé 2 reglas ACCEPT de DNS (UDP y TCP 53) y 6 reglas DROP, en ese orden. El test pide ese orden.
2. **¿Con `ROOT_TOKEN_BUILDER`, se sigue montando `.credentials.json`?** *Decisión:* sí, como pide el brief, y `lanzar` exige que exista. En Colab puede no existir. Si no debe exigirse, hay que cambiarlo.
3. **¿El ejecutor bwrap debe revisar la vigencia del login?** Podman la revisa con `verificar_antes`. *Decisión:* no la revisa. Con token por archivo no hay vencimiento que leer.
4. **¿Dónde está `claude` en Colab?** El `PATH` fijo es `/usr/local/bin:/usr/bin:/bin`, y bwrap solo monta `/usr`, `/etc`, `/opt` y los enlaces. Un `claude` fuera de esas rutas no se encuentra. *Decisión:* dejé el `PATH` fijo del brief.
5. **¿Vale `tests/red_builder.rs` como archivo nuevo?** *Decisión:* lo creé, porque el guion necesita sus tests. Se puede mover.
6. **Chequeo de `$HOME`.** *Decisión:* es más estricto que el brief. Falla si `$HOME` es legible por el builder, también con modo 0755. En Colab hay que cerrar `/root` con 0700, y `ROOT_HOME` también.
7. **El uid del builder sale de `/etc/passwd`.** Un usuario de LDAP no aparece. *Decisión:* lo dejé así, porque en Colab el usuario `builder` es local.
8. **¿El oráculo debe seguir el modo del entorno de `corre`?** El modo del builder sale de `deps.entorno`. El del oráculo sale de `std::env`. En producción es el mismo proceso. *Decisión:* no lo cambié.
9. **¿Un `ROOT_CONTENEDOR_MODO=bwrap` sin red o sin usuario debe dar código 2 (`Uso`) o 3 (`Ejecutor`)?** *Decisión:* da 3.
10. **¿Dónde van los logs de `slirp4netns`?** Hoy van a `/dev/null`. Si falla en Colab, no queda rastro. *Decisión:* los dejé en `/dev/null`. El guion reintenta `getent` 10 veces (`ROOT_RED_INTENTOS`) antes de fallar.
11. **El guion reusa el proceso de sostén del netns** si sigue vivo y slirp murió. Es decisión mía, sin prueba con `ip` real.
