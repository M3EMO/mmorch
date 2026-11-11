# Informe de la Tarea 85 (builder local)

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (1e68899) | 2922 | 0 | 4 |
| Final (punto 4) | 2933 | 0 | 4 |

La suite con podman la corre el orquestador al mezclar.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 5 | Prefijo "inestable en esta tarea" duplicado y comentario viejo de `MAX_ENTRADA_JUEZ_BYTES` | Ninguno. La guarda es la suite verde | a1733a9 |
| 1 | La señal borraba la copia antes de matar el contenedor | `senal_termina_el_hijo_quita_contenedores_y_borra_la_copia_en_ese_orden`, `quitar_builders_*` (3), `terminar_hijo_mata_el_hijo_en_curso_y_libera_el_registro`, `corre_sigterm` y `corre_sigint` de `tests/corre.rs` | 6085ba1 |
| 2 | Un `cmdline` ilegible contaba como vivo y un pid reusado no se barría | `cmdline_ilegible_es_de_otro_usuario_y_se_barre`, `pid_reusado_por_otro_r00t_que_empezo_despues_se_barre` | 31e6266 |
| 3 | Dos lectores de cola y una cola que empezaba a mitad de un carácter | `modo_cola_no_empieza_a_mitad_de_un_caracter`, `modo_cola_con_flujo_largo_...`, `cola_de_podman_no_empieza_...`, `leer_todo_con_tope_no_empieza_...` | 37beddb |
| 4 | El lector anteponía el `STEP` al stdout | `contenedor_correr_comando_guarda_el_step_que_la_cola_perdio_en_paso`, `entrega_paso_guardado_clasifica_si_la_cola_perdio_el_step` | 5a0bf15 |

Guardas verdes: el test de SIGTERM de la Tarea 84, `senal_*`, `lector_step_*`, los tests de 571d3e5 y `entrega_build_con_step_seguido_de_mas_de_1_mib_da_la_imagen_no_construye`.
El commit del punto 1 necesitó un reintento por `cargo fmt`, no por un test de tiempo. Ningún test de tiempo falló en el hook.

## 3. Qué falló o quedó incompleto

- Quedó una carrera en el punto 1. Si la señal llega antes de que el spawn registre el pid, el hijo sobrevive.
- Quitar contenedores por nombre cubre ese caso solo si podman ya registró el contenedor.
- No hay test dedicado de la carrera de código de salida entre el hilo principal y el hilo de señal. Lo cubre el `Drop` de `VigilaSenales`, por lectura de código.
- Nada más quedó sin hacer.

## 4. Tests que cambiaron de valor

- Se borró `motivo_inestable_empieza_con_el_prefijo_que_lee_las_filas_viejas`. El brief pidió borrar el test que fijaba la coincidencia.
- `quitar_builders_con_programa_inexistente_no_falla` pasó a `..._avisa_el_error`. Ahora `ps` o `rm` con error devuelven `Err`.
- `contenedor_correr_comando_antepone_el_step_que_la_cola_perdio` pasó a `..._guarda_el_step_..._en_paso`, con `STEP 3/5`.
- Se borraron los 3 tests de `anteponer_step`, porque la función ya no existe.
- Algunos tests de `ejecutor.rs` y `corrida.rs` toman un cerrojo `serie_de_pruebas`. Evita que una señal mate el hijo de otro test.

## 5. Llamadores revisados

- `quitar_builders_de_este_proceso` (ahora devuelve `Result`): `al_recibir_senal` en `corrida.rs` y los tests de `ejecutor.rs`.
- `SalidaPodman` (campo nuevo): 6 archivos, los mismos en grep y en codegraph. `calibra.rs` solo la usa en una firma.
- `leer_con_tope`, `leer_cola_en_hilo(_con)`, `leer_salida_con_step`, `leer_todo_con`: firmas sin cambio. Codegraph y grep dan la misma unión.
- `barrer` y `barrer_con_proc`: firma sin cambio. El único llamador de producción es `corrida.rs:170`.
- `recoger_salida` cambió de firma. Solo la llama `correr_comando`.

## 6. Preguntas para el planner

1. `ps` o `rm` de contenedores con código distinto de 0 ahora son error y salen por stderr. Antes se ignoraban. Decisión: se acepta, porque el brief pide avisar el fallo.
2. Si `stat` o `btime` no se pueden leer, el barrido conserva la corrida. Decisión: conservar, porque el `cmdline` ya dijo que es `r00t`. La alternativa es barrer.
3. La cola alineada a UTF-8 pierde hasta 3 bytes de inicio. El aviso "recortada a los últimos N bytes" sigue diciendo el tope. Decisión: no cambiar el aviso.
4. La cola de stdout de podman también queda alineada a UTF-8, por compartir el lector. Decisión: se acepta.
5. `paso` se llena para todo comando de podman, no solo el build. Decisión: se deja, porque el lector es el mismo.
6. `ENTRADA_JUEZ_BYTES` en `calibra.rs` parece no tener usos. No se tocó. Quedan literales "inestable en esta tarea" en aserciones de tests de `calibra.rs` y `mod.rs`.
7. Un subagente editó por error este worktree desde otro. Esos cambios eran el punto 5 y se revisaron. Otro subagente revirtió sus propios archivos en este árbol.
