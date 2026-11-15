# Informe de la Tarea 155

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (0588837) | 3905 | 0 | 5 |
| Final (15eb6f1) | 3917 | 0 | 5 |

La base se suma de todos los binarios de test. El hook de commit no falló por tiempo en ningún punto.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La caché y los `.env*` o nombres secretos se saltaban en silencio | `cache_de_cargo_trae_fuentes_con_nombre_secreto`, `rustsec_trae_avisos_con_nombre_secreto`, `oraculo_con_nombre_secreto_falla_el_empaquetado`, `token_en_configs_de_root_home_falla_el_empaquetado` | ccc3c5f |
| 2 | Un crate con archivo ausente pasaba el empaquetado | `checksum_de_cargo_con_archivo_ausente_falla_el_empaquetado`, `checksum_de_cargo_mal_formado_falla_el_empaquetado` | e747753 |
| 3 | El runtime conservaba el uid de la laptop | `guion_preparar_extrae_con_no_same_owner` (en `colab_calibra` y en `oracle::colab`) | 15eb6f1 |

Guardas que pasaron desde el inicio:
- Punto 1: los tests de exclusión de la 141 y la 153.
- Punto 2: `checksum_de_cargo_con_archivo_presente_empaqueta`.
- Punto 3: `juez_abrir_paquete_usa_filter_data` y `guion_preparar_deja_los_archivos_con_el_uid_de_quien_extrae`.

Elección del punto 1: un parámetro con dos valores, `Nombres::Avisar` y `Nombres::Libres`. Es más simple que dos funciones que repiten el recorrido.

## 3. Qué falló o quedó incompleto

- El test de uid 4242 no se ve rojo sin root. GNU tar no cambia dueños cuando corre un usuario normal. Queda como guarda. Con root, sin `--no-same-owner`, fallaría.
- El test unitario de `copiar_dir` con `Avisar` no se vio rojo por separado. Usa la firma nueva y no compilaba antes del enum.
- `notebook-juez.py`: el test confirma el texto `filter="data"`. No prueba que el filtro ignore los dueños. Esa propiedad viene de la documentación de `tarfile`.
- Sin podman: ningún test nuevo lo necesita. Quedan para el orquestador la suite con podman y la prueba de la 141 en Colab (base 1,0; `find /content/root-paquete ! -uid 0 | head` vacío).
- Enlaces simbólicos en la caché: no se copian, como antes. Un enlace dentro de un crate listado en `files` hace fallar la guarda del punto 2 con el crate y el archivo. Ninguna caché de prueba trajo uno.

## 4. Las copias de `copiar_dir`

| Llamador | Origen | Regla |
|---|---|---|
| `armar_y_empaquetar` | `tools/colab` | `Avisar` |
| `armar_y_empaquetar` | `oraculos/<tarea>` | `Avisar` |
| `armar_y_empaquetar` | caché de cargo | `Libres` |
| `armar_y_empaquetar` | `cache/rustsec` | `Libres` |
| `armar_y_empaquetar` | `configs` | `Avisar` |

`EXCLUIDOS` aplica en las cinco, en cualquier nivel. `copiar_repo` no cambia.

## 5. Las extracciones de tar

| Extracción | Máquina | ¿Conserva dueños? |
|---|---|---|
| `guion_preparar_en` (`tar --no-same-owner -xzf`) | runtime de Colab, como root | No |
| `GUION_PREPARAR` en `src/oracle/colab.rs` | runtime de Colab, como root | No |
| `notebook-juez.py`, `tarfile` con `filter="data"` | Colab | No |
| `-xzf` dentro de tests de `colab_calibra.rs` | laptop, usuario normal | No cambian dueños sin root; no se tocaron |

Los comandos de las sondas no cambian: `COMANDO_GITLEAKS`, `COMANDO_CARGO_AUDIT` y `COMANDO_LOCK`.

## 6. Tests que cambiaron de valor

- `token_de_root_home_no_sale_al_paquete` pasó a `token_en_configs_de_root_home_falla_el_empaquetado`. Ahora `configs/token.json` da `Uso` con su ruta.
- `copiar_dir_copia_archivos_normales_y_filtra_secretos_y_excluidos` pasó a `copiar_dir_avisar_copia_normales_y_omite_excluidos`. Los secretos ya no se saltan, fallan. Cubre la copia normal y `EXCLUIDOS`.
- Tests de otros archivos editados: ninguno.

## 7. Llamadores revisados

- `copiar_dir` (firma nueva): codegraph da `armar_y_empaquetar` y un test. `grep -w` da cinco llamadas en `armar_y_empaquetar`, la recursión y los tests. `src/oracle/mod.rs` tiene otra `copiar_dir` privada con otra firma. No se tocó.
- `script_preparar` y `guion_preparar_en`: la firma no cambió. Solo cambió el texto del guion.
- `GUION_PREPARAR`: la firma no cambió. Un test lo lee en `oracle::colab` y otro en `colab_calibra`.

## 8. Preguntas para el planner

1. El mensaje de `Avisar` tutea: "Quítalo o renómbralo". Decisión: se deja, igual que otros mensajes de Root. Confirmar si el tono debe ser otro.
2. La guarda del punto 2 falla con `Infra`, como pide el brief. Un `.cargo-checksum.json` sin objeto `files` también falla. Decisión: se trata como error de formato.
