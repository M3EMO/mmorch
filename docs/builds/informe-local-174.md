# Informe de la Tarea 174

Rama: `laptop/tarea-174`, sobre `laptop/cadena-173b` (8eb25e1). Sin push. No usé `codegraph`: busqué llamadores con `grep -w`.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`, sumando todos los binarios de test.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (8eb25e1) | 4219 | 0 | 5 |
| Final (punto 4) | 4243 | 0 | 5 |

Los tests con podman quedaron saltados (`saltar_sin_podman`). Los corre el orquestador.
El hook de pre-commit pasó en los cuatro commits al primer intento. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `preparar-corre.sh` no ve `rustc` | `preparar_corre_versiones_encuentra_rustc_bajo_cargo_home`, `preparar_corre_rust_igual_que_preparar_runtime` | 283443f |
| 2 | El builder no ve Rust | `corre_bwrap_entorno_pone_cargo_primero_y_cargo_home_en_el_estado`, `corre_bwrap_crea_cargo_home_en_el_estado_antes_de_lanzar`, `preparar_corre_versiones_corre_el_entorno_del_builder_con_setpriv`, `preparar_corre_frena_si_cargo_falla_con_el_entorno_del_builder`, y el de entorno ajustado. Guardas: `corre_bwrap_montajes_son_los_de_la_base`, variables prohibidas | cb186a7 |
| 3 | `node` de apt es v18 | `corre_versiones_builder_env_trae_las_tres_claves`, `corre_node_sin_version_da_infra`, `corre_node_raro_da_infra`, `preparar_corre_apt_no_trae_nodejs_ni_npm`, `preparar_corre_sin_node_version_frena_y_nombra_la_clave`, `preparar_corre_baja_node_con_sha_correcto_y_lo_extrae`, `preparar_corre_frena_si_sha_de_node_no_coincide`, `preparar_corre_frena_si_node_no_coincide_con_versiones` | 194f32e |
| 4 | El builder no entra a su carpeta ni la posee | `corre_bwrap_chown_da_repo_y_estado_al_builder_antes_de_lanzar`, `corre_bwrap_chown_que_falla_frena_con_su_stderr`, `corre_bwrap_ancestro_0700_frena_y_nombra_la_carpeta`, `corre_bwrap_ancestros_el_bit_que_cuenta_es_el_de_quien_coincide`, `corre_bwrap_home_0711_pasa_y_0704_frena`, `corre_runtime_pasa_al_r00t_home_de_usuario_en_srv`, `preparar_corre_crea_raiz_de_usuario_root_con_modo_0711`, `preparar_corre_root_usuario_por_omision_es_srv`, `preparar_corre_fija_safe_directory_en_git_del_sistema`. Guardas: tests de la 171 sobre `$HOME` legible | 801c8ea |

Nota del punto 2: `preparar_corre_path_builder_por_omision_es_path_fijo_de_bwrap` nació verde, porque los dos lados tenían el valor viejo. No lo vi en rojo.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- **No probé nada contra podman, Colab ni `rustup` real.** `version_node()` de `PodmanBuilder` no tiene test real. Lo verifica el orquestador (punto 4 de su lista).
- El orden de pasos cambió. `crear_builder` corre antes de `versiones`, porque la revisión del punto 2 corre como `builder`. El subagente del punto 2 lo detectó. Yo moví el paso.
- Agregué `xz-utils` a la lista de apt. `tar -xJf` lo necesita para el tarball de node. Está fuera de lo literal del brief, pero dentro de `preparar-corre.sh`.
- El paso `node` baja de `nodejs.org`. Si Colab no tiene red a ese host, el paso falla. Antes lo hacía apt.

## 4. El entorno y las carpetas del builder

**Entorno del builder dentro de bwrap** (laptop y Colab, igual):

| Variable | Valor |
|---|---|
| `HOME` | `/home/builder` |
| `PATH` | `/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin` |
| `RUSTUP_HOME` | `/usr/local/rustup` (solo lectura, dentro de `/usr`) |
| `CARGO_HOME` | `/home/builder/.claude/cargo-home` (dentro del estado, con escritura) |

**Carpetas en Colab:**

| Carpeta | Dueño | Modo |
|---|---|---|
| `/srv/root-usuario`, `.cache`, `.cache/root`, `.cache/root/corridas` | root | 0711 cada nivel |
| `<corridas>/<id>/repo/` y `claude-estado/` (recursivo) | `builder` (por `chown -R -h` antes de cada lanzamiento) | los que traiga el contenido |
| `claude-estado/cargo-home/` | el dueño del estado | los de `create_dir_all` |
| `/root`, `ROOT_HOME` (`/root/root-home`) | root | 0700 |

`r00t` hijo corre con `HOME=/srv/root-usuario` (`corre-runtime.py`).
**En la laptop** no cambia nada de carpetas: `corre` usa podman y su `$HOME`. El `chown` y la revisión de ancestros solo están en el ejecutor bwrap.
Los montajes de bwrap son los de la base: ningún `--bind` ni `--ro-bind` nuevo. Un test lo guarda.

## 5. Tests que cambiaron de valor

- `corre_versiones_builder_env_trae_las_dos_claves` → `..._las_tres_claves`: ahora trae `NODE_VERSION=22.11.0`.
- `VERSIONES_OK` y el helper `Prep` (módulo de guiones): suman `NODE_VERSION`, `curl` y `tar` falsos, y el `node` falso pasa de v22.0.0 a v22.11.0.
- `preparar_corre_root_npm_por_omision_es_usr_bin_npm` → `..._es_usr_local_bin_npm`: el npm es el del node instalado.
- `corre_bwrap_entorno_del_command_sin_claves_ni_root_home_y_con_home_y_path_fijos`: suma `RUSTUP_HOME` y `CARGO_HOME`, y el `PATH` nuevo.
- `corre_bwrap_el_hijo_no_hereda_claves_ni_root_home`: la lista blanca suma `RUSTUP_HOME` y `CARGO_HOME`.
- `preparar_corre_frena_si_cargo_falla_con_el_entorno_del_builder`: ahora comprueba que no llega a `paso root-home`.
- Fixture `disposicion()` de bwrap: `$HOME` y la cadena hasta el repo pasan a 0711. Antes `$HOME` era 0700.
- `cadena_falsa` y `ejecutor_de` de bwrap: suman un `chown` falso (5 programas).

Todos están en `src/colab_corre.rs` y `src/corre/bwrap.rs`, archivos de la tabla. No edité tests de otros archivos.

## 6. Llamadores revisados

- `ImagenBuilder::version_node` (trait nuevo): `PodmanBuilder`, los dos dobles de test (`Falso`, `NodeFijo`) y `armar` en `colab_corre.rs`. Solo `colab_corre.rs` usa el trait.
- `chequeo_previo`: solo `lanzar`.
- `revisar_permisos`: solo `chequeo_previo` y sus tests. Cambió la semántica: mira lectura, no búsqueda.
- `EjecutorBwrapBuilder` suma los campos públicos `chown` y `tope_ancestros`. Los inicializa `desde_entorno`. Los tests usan `ejecutor_de`.
- `cadena_falsa` y `ejecutor_de` (tests): todos los llamadores de bwrap.rs se ajustaron.

## 7. Preguntas para el planner

1. **¿`NODE_VERSION` sin la `v` inicial?** Decisión: sí. Guarda `22.11.0`. La URL de nodejs.org suma la `v`, y el guion compara sin ella.
2. **¿`tope_ancestros` como campo público?** El chequeo de ancestros sube hasta `/`. En tests, `TMPDIR` cae bajo `/root` (0700) y fallaría. Decisión: campo con default `/`. Los tests lo bajan a su carpeta temporal.
3. **¿`safe.directory '*'` es demasiado amplio?** Decisión: el brief lo pide así, igual que `builder-colab.sh`. Lo dejé tal cual.
4. **¿`chown -R` en cada lanzamiento es caro con repos grandes?** Decisión: lo dejé, como pide el brief. Tras la corrida, `repo/` queda del builder. `r00t` lo lee con `git` gracias a `safe.directory`.
5. **¿El SHA-256 de node sale de `SHASUMS256.txt` del mismo host?** Valida la descarga, no la autenticidad ante un host comprometido. Decisión: como pide el brief. Una firma GPG sería otra tarea.
6. **`RUSTUP_HOME` en solo lectura.** Si `rustup` necesita escribir (por ejemplo, locks o `settings.toml`), podría fallar. Los tests usan falsos. La prueba de aislamiento lo mostrará. Decisión: sin cambio, por el brief.
