# Informe de la Tarea 145

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (799e48c) | no medido | no medido | no medido |
| Final | 3757 | 0 | 5 |

- No medí la base: el informe de la 141 dice 3671 verdes sobre otra punta de la cadena.
- Los tests con podman quedan para el orquestador, al mezclar.
- Un commit tardó más de 120 segundos por el hook y pasó al fondo. Terminó bien. No hubo reintentos por tiempo.
- El primer intento del commit del punto 1 y del punto 2 falló por formato (`rustfmt`). Apliqué `rustfmt` y reintenté.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El runtime no tiene `/repo`, `/salida`, `/oraculo`, `/cache`, `/rustsec` ni `/trabajo`. | `carpetas_del_runtime_son_los_montajes_de_bwrap`, `seis_carpetas_dan_ok_cada_una`, `sin_repo_nombra_repo_y_sale_mal`, `verificacion_coincide` (15 líneas). | 28d6a31 |
| 2 (parcial) | `error_de_infra` dice "podman falló" en modo `bwrap`. | `error_de_infra_bwrap_no_nombra_podman`. Guarda: `error_de_infra_podman_conserva_su_mensaje`. | 1cea4ed |
| 3 | Un repo con `token.rs` pierde archivos en el paquete. | `repo_con_archivo_de_nombre_secreto_lo_lleva`, `repo_con_carpeta_evidencia_la_lleva`. Guardas: `repo_no_lleva_git_target_ni_env`, `token_de_root_home_no_sale_al_paquete`. | 1533795 |
| 4 (parcial) | El comando de un trabajo no lleva `--n` ni `--semilla`. | `comando_sin_n_explicita_lleva_n_4_y_semilla_42`, `trabajo_con_n_explicita_usa_esa_n_y_el_otro_la_por_defecto`, `n_que_no_es_entero_mayor_que_cero_da_uso_y_nombra_la_entrada`, `manifiesto_guarda_n_y_semilla_de_cada_trabajo`. Guarda: `trabajo_sin_tercer_campo_usa_la_n_por_defecto`. | c8215b4 |
| 5 | La vuelta no trae SHA-256 del runtime. Un `stop` fallido tras bajar la vuelta frena la importación. | `guion_de_cierre_escribe_el_sha_de_lo_que_empaqueta`, `vuelta_con_archivo_alterado_no_se_importa`, `vuelta_sin_manifiesto_no_se_importa`, `archivo_que_no_esta_en_el_manifiesto_no_se_importa`, `stop_que_falla_tras_bajar_la_vuelta_importa_y_avisa`. Guarda: `stop_que_falla_antes_de_la_vuelta_no_importa`. | d217a51 |

## 3. Qué falló o quedó incompleto

1. **Punto 2, el mensaje de plazo.** Sigue diciendo "contenedor bwrap eliminado". Su texto vive en el `#[error]` de `ErrorOraculo::PlazoVencido`, en `src/oracle/mod.rs`. Ese archivo es de la 143 y la 144, y está prohibido. Cambiarlo en `contenedor.rs` pide cambiar el tipo de error, y `calibra.rs` y `mod.rs` lo distinguen. El test "el error de plazo no contiene contenedor" no se escribió. Falta también `descarga.rs:756`, con el mismo texto.
2. **Punto 4, las banderas `--n` y `--semilla` de `colab empaquetar`.** No están en `cli.rs`. `src/lib.rs:985-1005` desestructura `ColabAccion::Empaquetar` sin `..` y arma `PedidoEmpaquetar` campo por campo. Agregar las banderas rompe `lib.rs`, que está prohibido. Quedó hecho: las constantes, `tarea:dimension:n`, `empaquetar_cli_con(pedido, n, semilla)` y el manifiesto. Hoy el comando usa 4 y 42, y `tarea:dimension:n` cambia la `n` de un trabajo. Falta el cambio de `cli.rs` y `lib.rs`: dos campos nuevos `n: u32` (default 4) y `semilla: u64` (default 42) en la variante, y `lib.rs` llama a `empaquetar_cli_con(pedido, *n, *semilla)`.
3. **Sin probar en Colab.** El guion de cierre en Python corrió solo con `python3` local. La prueba de la 141, vuelta (58), sección 5, queda para el orquestador.
4. **`mkdir` de `preparar-runtime.sh`.** Un test compara la lista con `argumentos_bwrap`, pero ningún test ejecuta `crear_carpetas`, porque el script instala paquetes.
5. **`empaquetar_cli_con`.** No tiene test directo: necesita `root_home()` y podman. Compila y delega.
6. **Compatibilidad.** `TrabajoManifiesto` exige `n` y `semilla`. Un paquete de ida armado antes de este cambio no se puede leer con `colab calibrar`. Hay que reempaquetar.
7. **`copiar_dir`.** Su parámetro `excluir` quedó sin uso real: todas sus llamadas pasan `&[]`.

## 4. El comando de un trabajo

```
r00t oraculo calibrar --tarea s2-t1 --dimension pruebas --repo repos/s2-t1 --n 4 --semilla 42 --json
```

Con `s2-t1:pruebas:8`, el comando lleva `--n 8`.

## 5. Tests que cambiaron de valor

- `verificacion_coincide` (`tests/colab_preparar.rs`): espera 15 líneas `ok` en vez de 9, por las seis carpetas. El `escenario()` crea las carpetas y `correr()` pasa `ROOT_RAIZ_DIR`.
- `manifiesto_trae_sha256...` (`colab_calibra`): el comando esperado lleva `--n 4 --semilla 42`.
- `parseo_de_trabajos_repos_y_sha_de_entrada`: usa `TrabajoPedido` y la firma nueva.
- `Escenario::paquete` y `paquete_de_ida` (helpers): usan `TrabajoPedido`, `n` y `semilla`.
- `vuelta_minima` y dos tests de armado de vuelta: llaman a `con_manifiesto_de_vuelta`, porque la vuelta sin manifiesto del runtime se rechaza ahora. Sus aserciones no cambiaron.
- Los tests de `infra_de` en `contenedor.rs`: pasan el motor `"podman"`.
- Ningún test de otro archivo de producción fuera de los de arriba. Todos los cambios caen en archivos de la tabla del brief.

`docs/interfaces.md` (línea 61) lista `colab empaquetar` sin `--n` ni `--semilla`. No lo toqué. Cuando se agreguen las banderas, el orquestador lo regenera.

## 6. Llamadores revisados

| Firma | codegraph | grep -w |
|---|---|---|
| `error_de_infra(motor, codigo, stderr)` | `resultado_de` | `resultado_de` y los tests de `contenedor.rs` |
| `comando_de_trabajo(.., n, semilla)` | `armar_manifiesto` y un test | los mismos |
| `parsear_trabajos(entradas, n_defecto)` | `empaquetar_cli_con` y tests | los mismos |
| `Paquete.trabajos` (`TrabajoPedido`), `DatoTrabajo`, `TrabajoManifiesto` | solo `colab_calibra.rs` | `lib.rs` solo usa `PedidoEmpaquetar`, sin cambio |
| `empaquetar_cli(pedido)` | `lib.rs:627`; firma igual | igual |
| `script_cierre()` | `correr_runtime`; firma igual | igual |

Funciones nuevas: `copiar_repo`, `empaquetar_cli_con`, `script_cierre_en`, `verificar_vuelta_del_runtime`, `aviso_de_liberacion_pendiente`.

## 7. Preguntas para el planner

1. **¿Se puede editar `src/oracle/mod.rs` para el mensaje de plazo?** Decidí no tocarlo. El texto "contenedor {c} eliminado" vive allí. Propongo un campo `motor` en `PlazoVencido`, o un texto sin la palabra "contenedor".
2. **¿Se puede editar `src/lib.rs` para las banderas `--n` y `--semilla`?** Decidí no tocarlo. Propongo el cambio descrito en la sección 3, punto 2.
3. **¿Los repos entran con `target` y `.env*` omitidos?** El brief dice "salvo `.git`". Mantuve la omisión de `target` (como hoy) y de los archivos `.env*`, por la regla de "ninguna clave va en el paquete". Un `.env` de repo con una clave real no sale.
4. **¿Qué pasa con los enlaces simbólicos de un repo?** Siguen sin copiarse, como hoy. Un repo con enlaces relativos no queda igual en Colab.
5. **¿La vuelta sin `manifiesto-vuelta.json` se rechaza?** Decidí que sí: la regla es "llega verificada desde el runtime". Un paquete de vuelta viejo, hecho a mano, ya no entra por `colab calibrar`. `colab importar` no cambió.
6. **¿El SHA del tar completo también debe viajar por `exec`?** Hoy el manifiesto va dentro del mismo tar. Cubre un archivo cambiado, pero no un tar entero reemplazado con su manifiesto. No lo agregué. Se puede sumar un marcador `ROOT_SHA_VUELTA=` en el guion de cierre.
7. **¿Falta un test de `crear_carpetas` con ejecución real?** Decidí no ejecutar `preparar-runtime.sh` en la laptop, como pide el brief.
