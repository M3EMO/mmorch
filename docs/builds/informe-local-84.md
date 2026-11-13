# Informe de la Tarea 84 (builder local)

Base: 86f260c, la punta de `laptop/tarea-83`. Rama: `laptop/tarea-84`.
Toda la corrida usó `ROOT_SIN_PODMAN=1`. Ninguna prueba llamó a una API ni tocó `~/.local/share/root/`.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (86f260c) | 2884 | 0 | 4 |
| Final (rama completa) | 2922 | 0 | 4 |

La suite corrió con `tools/test-seguro.sh --no-fail-fast`. El hook de commit pasó en los cinco commits al primer intento de cada uno.
El punto 2 falló un intento de commit por un lint de clippy (`is_multiple_of`). Se corrigió y el segundo intento pasó. No fue un test de tiempo.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 2 | `leer_en_hilo` y `leer_todo` leían sin tope. | `leer_en_hilo_guarda_64_mib_y_drena_los_70_mib`, `leer_en_hilo_con_tope_chico_guarda_el_inicio_y_marca_recorte`, `modo_cola_conserva_los_ultimos_bytes_y_marca_recorte`, `corre_proceso_con_tope_falla_si_stdout_pasa_el_tope`, `corre_proceso_con_tope_stderr_recortado_no_falla`, `leer_todo_con_tope_chico_guarda_la_cola_y_avisa`. Rojo por falta de API (no compilaban). | f1b1f9e |
| 3 | Un `RUN` con más de 1 MiB de salida después de su `STEP` daba `Infra`. | `entrega_build_con_step_seguido_de_mas_de_1_mib_da_la_imagen_no_construye` (daba `Infra`). Los tests de `es_linea_step` y del lector no compilaban. | 8da92ce |
| 1 | Una señal dejaba la credencial en disco. El barrido tomaba un pid reciclado por vivo. | Barrido: `corrida_con_pid_del_proceso_de_test_no_r00t_se_barre`, `pid_reciclado_por_otro_proceso_se_barre`. Señales: `senal_sigterm_quita_la_copia_y_sale_con_143`, `senal_sigint_..._130`, `senal_quita_los_contenedores_de_este_proceso_por_nombre`, `quitar_builders_pide_por_nombre_solo_los_de_este_proceso`. Integración: `corre_sigterm_..._143` y `corre_sigint_..._130`. | 98e5164 |
| 4 | Cinco defectos de cierre de la Tarea 83 (preguntas 2, 3, 7 y 10). | `inestables_de_con_huella_vieja_no_los_da`, `calibra_fila_vieja_con_motivo_inestable_en_no_aplica_se_lee_como_inestable`, `calibra_dimension_inestable_va_a_inestable_y_no_a_no_aplica`, `calibra_base_sin_items_del_juez_se_juzga_una_sola_vez`, `oraculo_fila_con_huella_vieja_no_excluye_sus_inestables`. | c43acf5 |
| 5 | Código sin uso en producción. | Ninguno. Es un borrado puro. | 3f5b30c |

Guardas que quedaron verdes desde el inicio:
- Punto 2: `leer_en_hilo_salida_chica_se_lee_completa`, `corre_proceso_con_tope_amplio_y_salida_chica_igual_que_hoy`, `leer_todo_con_salida_chica_sale_igual_y_sin_aviso`.
- Punto 3: `entrega_ultimo_paso_*`, `entrega_build_con_step_y_8_kb_*` y `contenedor_correr_comando_conserva_solo_la_cola_de_1_mib` (571d3e5).
- Punto 1: `corre_credencial_*` y `corre_fallo_del_ejecutor_*` (camino normal y de error), `vigia_de_senales_se_suelta_al_salir_sin_colgarse`.
- Punto 4: `oraculo_diseno_con_estables_de_menos_de_la_mitad_queda_inestable`, `calibra_dimension_inestable_no_mide_casos_y_no_califica`, `inestables_de_da_los_de_la_fila_mas_reciente`.

## 3. Qué falló o quedó incompleto

- **Punto 1, archivo fuera de la lista.** El brief no incluye `src/corre/barrido.rs`. El brief no sabía que el barrido ya existía. El chequeo de `/proc/<pid>/cmdline` solo cabe ahí. Lo toqué y lo dejé en el mismo commit 98e5164. El orquestador puede descartar ese archivo si lo prefiere.
- **Punto 1, hueco de contenedor.** `r00t` no mata el proceso `podman run`. Quita los contenedores por nombre. Si la señal llega antes de que podman registre el contenedor, el contenedor sigue vivo con la credencial montada.
- **Punto 1, SIGKILL.** No se atrapa. Lo cubre el barrido del próximo arranque.
- **Punto 1, pid reusado por otro `r00t`.** El cmdline no distingue dos corridas de `r00t`. Una corrida huérfana cuyo pid lo reusa otro `r00t` vivo no se barre.
- **Punto 1, permisos.** Un `cmdline` ilegible por permisos cuenta como vivo. No hay test de ese caso: como root el archivo siempre se lee.
- **Punto 4, no verificado de punta a punta.** Ninguna prueba cubre una tarea real sin modelo de juez. Solo hay pruebas con un juez falso.
- Cada corrida de integración del punto 1 deja un `sleep 30` huérfano del `podman` falso durante 30 segundos. Es inofensivo.

## 4. Tests que cambiaron de valor

- `corrida_con_pid_vivo_no_se_toca` (barrido.rs) pasó a `corrida_con_pid_del_proceso_de_test_no_r00t_se_barre`. El pid propio ya no es excepción. Manda el cmdline.
- `inestables_de_da_los_de_la_fila_mas_reciente`: sus filas ahora llevan huella vigente. Sin ella devolvería vacío.
- `calibra_dimension_inestable_no_mide_casos_y_no_califica`: lee el motivo de `inestable`, no de `no_aplica`.
- `calibra_un_caso_de_diseno_toma_la_mutacion_de_la_base`, `calibra_un_caso_limpio_toma_de_la_base_las_sondas_deterministas_y_corre_el_juez_y_los_tests` y `base_con_juez_guarda_una_lista_de_extractos_por_juicio`: la base ahora trae un ítem del juez. Sin juez, la base se juzga una vez. Las expectativas no cambiaron.
- El helper `calibracion_con_inestables` (oracle/mod.rs) usa la huella vigente.

## 5. Funciones borradas

| Función | Llamadores revisados | Producción |
|---|---|---|
| `cota_bytes_de_prompt` | grep -w: 5 usos, todos en tests de `juez.rs`. codegraph: 4 tests. | Ninguno |
| `casos_con_reserva_abierta` | grep -w: el wrapper `_en` y 4 tests. codegraph: el wrapper y un test. | Ninguno |
| `casos_con_reserva_abierta_en` | grep -w: solo la definición. codegraph: 0. | Ninguno |

Se borraron también 8 tests, los helpers `limites_chicos` y `reserva_de_caso`, y las constantes huérfanas `BYTES_POR_CARACTER`, `HALLAZGO_CHARS_MAX`, `RESERVA_ENCABEZADOS_BYTES` y `RESERVA_REINTENTO_BYTES`.
Se borró `Presupuesto.gasto_desde`, su armado en `src/lib.rs` y las 7 líneas de `tests/calibra.rs`. Ningún código de producción lo leía.

Quedan menciones en documentos que no toqué:
- `docs/builds/informe-local-83.md` (líneas 60 y 61) llama "candidatas a borrar" a estas funciones.
- `docs/builds/informe-local-82.md:50` es histórico.
- El comentario de `MAX_ENTRADA_JUEZ_BYTES` en `juez.rs` (líneas 45-46) habla de la cota borrada. Conviene revisarlo.

## 6. Llamadores revisados

| Firma | codegraph | grep -w |
|---|---|---|
| `leer_en_hilo` (ahora `JoinHandle<Lectura>`; en producción se usa `leer_en_hilo_con`) | sin índice en el worktree | solo `ejecutor.rs` |
| `leer_todo` (ahora llama a `leer_todo_con`) | sin índice | solo `colab.rs:463-464` |
| `correr_proceso` (firma igual; delega en `correr_proceso_con_tope`) | sin índice | `ejecutor.rs` y sus tests |
| `texto_paso_fallido` (comportamiento igual) | sin índice | `clasificar_fallo` y 3 tests |
| `barrer` (cambió el comportamiento) | 7 tests; **no vio** el llamador de producción | `corrida.rs:170` |
| `al_recibir_senal`, `quitar_builders_de_este_proceso` (nuevas) | no vio el closure del vigía | `corrida.rs`, `ejecutor.rs`, tests |
| `Presupuesto` (sin `gasto_desde` ni lifetime) | 10 llamadores, todos tests o `lib.rs` | los mismos |
| `gasto_desde` | codegraph no indexa campos | 10 sitios, ninguno de producción |
| `inestables_de` (firma igual, filtra por huella) | — | único lector: `excluidos_de` en `oracle/mod.rs` |
| `Fila.inestable` (campo nuevo) | — | lectores de `no_aplica`: `lib.rs` y `calibra.rs`, revisados |

El caso de `barrer` confirma la regla: codegraph solo no basta, y hay que unirlo con grep.

## 7. Preguntas para el planner

1. **Punto 1: orden de la señal.** El brief pide borrar la copia y después quitar el contenedor. Seguí ese orden. Un bind mount conserva el inodo tras el borrado, así que el builder podría leer el token hasta que el contenedor muera. Decisión: orden del brief. ¿Se prefiere quitar el contenedor primero?
2. **Punto 1: salida durante la evaluación.** El código 130 o 143 también sale si la señal llega durante la evaluación, no solo durante los roles. Decisión: se acepta.
3. **Punto 1: `barrido.rs` fuera de la lista.** Lo toqué sin parar, porque el brief ignoraba el barrido existente. Decisión: tocarlo en el mismo commit. Ver la sección 3.
4. **Punto 2: tope de colab y UTF-8.** La cola de colab puede empezar a mitad de un carácter. `from_utf8_lossy` pone un carácter de reemplazo después del aviso. Decisión: no alinear. ¿Alcanza?
5. **Punto 2: modo cola duplicado.** `leer_con_tope` en modo cola repite la lógica de `leer_cola_en_hilo_con`. Decisión: dejarlo, porque el brief prohíbe tocar `leer_cola_en_hilo*`. Unificar queda para otra tarea.
6. **Punto 2: `leer_en_hilo` sin llamador de producción.** Quedó con `#[cfg(test)]`. Producción usa `leer_en_hilo_con`. Decisión: dejarlo así para evitar código muerto.
7. **Punto 3: forma de guardar el `STEP`.** El brief dice "guarda aparte" y que `clasificar_fallo` use esa línea. `SalidaPodman` tiene literales en archivos fuera de la lista, así que no le agregué un campo. Decisión: el lector antepone la última línea `STEP` al stdout solo si la cola ya no trae ninguno. `clasificar_fallo` no cambió y el resultado es el mismo. ¿Se prefiere un campo explícito en una tarea que pueda tocar esos archivos?
8. **Punto 3: tope de la línea `STEP`.** La línea guardada tiene un tope de 64 KiB, no 4 KiB. Con 4 KiB un paso largo no coincidiría con el Dockerfile y daría "sin veredicto". Un paso de más de 64 KiB sigue dando "sin veredicto".
9. **Punto 4: cómo se sabe que el juez no corre.** Calibra no ve el modelo del juez. Decisión: la base se repite solo si trae ítems `Fuente::Juez` en la dimensión. La alternativa es pasar un `hay_juez` desde `evaluar`, que cambia la firma de `calibrar_tarea_con`.
10. **Punto 4: forma de `inestable`.** Guarda el motivo completo como texto. La lista sigue en `inestables`. Una fila vieja se normaliza al cargar `Calibraciones`.
11. **Punto 4: prefijo duplicado.** "inestable en esta tarea" vive como constante en `calibra.rs` y como literal en `calibra/inestables.rs`. Un test fija que coinciden. Conviene unificarlo cuando se pueda tocar `inestables.rs`.
12. **Punto 4: `Presupuesto` con un solo campo.** Queda solo `usd`. Podría ser un `Option<f64>`. No lo cambié.
