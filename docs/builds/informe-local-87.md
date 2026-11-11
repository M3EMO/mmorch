# Informe de la Tarea 87

Base: 70788ac. Rama: `laptop/tarea-87`. Los tres puntos los construyó un subagente Haiku en su propio worktree. El organizador revisó cada diff y lo aplicó a la rama.

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1`.

- Base (informe de la Tarea 86): la biblioteca da 2368 verdes, 0 fallas, 0 ignorados.
- Final: la biblioteca da 2386 verdes, 0 fallas, 0 ignorados.
- Final, todos los binarios juntos: 2957 verdes, 0 fallas, 4 ignorados.
- `cargo clippy --all-targets` y `cargo fmt --check` quedaron limpios en los worktrees de cada punto.
- Ningún test de tiempo falló en el hook. No hubo reintentos de commit.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Un `kill` podía caer en un pid ya esperado, un error de espera dejaba al hijo vivo, un segundo registro pisaba al primero, y la copia se creaba tras la señal | `senal_tras_el_try_wait_no_mata_el_pid_ya_esperado`, `error_de_la_espera_termina_al_hijo_antes_de_devolver_el_error`, `segundo_registro_con_hijo_en_curso_da_error_y_no_lanza`, `senal_en_la_ventana_entre_terminando_y_la_copia_no_deja_copia` | 3568812 |
| 2 | Tolerancia de 2 s y un error aborta el barrido | `r00t_vivo_que_empezo_60_s_despues_no_se_barre`, `error_en_una_corrida_del_medio_no_corta_el_barrido`, `error_al_registrar_el_gate_no_corta_el_barrido`, `r00t_vivo_que_empezo_antes_o_en_el_margen_no_se_barre` | 5bbcab6 |
| 3 | Nada marcaba el defecto en el piso ni los ítems omitidos | `piso_defecto_en_diseno_con_nivel_base_0_guarda_true`, `piso_sin_juez_usa_la_baja_de_un_nivel`, `omitidos_lista_el_item_estable_que_el_juez_dejo_sin_nivel`, `fila_cuenta_defectos_en_piso_y_omisiones_y_el_texto_los_muestra` | d0d9aa1 |

Guardas que no estuvieron en rojo:
- Punto 1: `limpieza_de_la_senal_corre_sin_tomar_el_cerrojo` (confirma el MEDIUM 1: la limpieza no pisa el registro) y los tests de señal de las Tareas 84 a 86.
- Punto 2: `r00t_vivo_que_empezo_400_s_despues_se_barre` ya pasaba con el margen viejo.
- Punto 3: `piso_defecto_en_diseno_con_nivel_base_3_guarda_false`, `piso_sin_nivel_base_del_item_guarda_false`, `piso_no_cambia_la_sensibilidad_de_un_defecto_detectado_por_cita`, `caso_viejo_sin_piso_ni_omitidos_se_lee_con_cero` y `fila_vieja_sin_conteos_de_piso_se_lee_con_cero`.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- El test de error de espera (punto 1) usa una costura que fuerza el fallo de `try_wait`. Su rojo vino de un `Vencio` y no del hijo vivo.
- El test de error de registro del `gate` (punto 2) hace fallar todas las corridas. Fallar solo una pediría otra costura, y no se agregó.
- No se corrieron `corre`, `calibrar` ni la validez reales, como pide el brief.
- La suite con podman la corre el orquestador.

## 4. Tests que cambiaron de valor

- `registro_del_hijo_se_borra_al_recoger_el_wait`: la costura corre bajo el cerrojo, y `pid_en_curso()` allí daría un bloqueo. El test mira el registro al volver `correr_proceso`.
- Seis tests de señal de `ejecutor.rs` cambiaron la llamada de `terminar_hijo_en_curso` a `terminar_por_senal(|_| {})`. Sus aserciones siguen iguales.
- `r00t_vivo_que_empezo_antes_o_en_el_margen_no_se_barre`: el caso "2 s después" pasa a "300 s después", el borde del margen nuevo.
- `pid_reusado_por_otro_r00t_que_empezo_despues_se_barre`: usa 400 s en vez de 10 s, para pasar el margen de 300 s.
- Literales de `Fila` en `tests/calibra.rs` (3) y `src/oracle/mod.rs` (2) suman los dos campos nuevos. Solo compilan: no cambia ninguna aserción.

## 5. Llamadores revisados

- `terminar_hijo_en_curso` (borrado): lo llamaban `corrida.rs` (`al_recibir_senal`) y los tests de `ejecutor.rs`. Todos pasaron a `terminar_por_senal`.
- `terminando()` y `HijoRegistrado` (borrados): solo los usaba `ejecutor.rs`. `lanzar` lee ahora `terminando` del cerrojo.
- `lanzar_hijo` (borrado): solo lo llamaba `correr_hijo`.
- `correr_proceso` y `correr_proceso_con_tope`: quedan solo en `cfg(test)`. El builder usa `correr_registrado`.
- `barrer` y `barrer_con_proc`: cambia el retorno a `ResultadoBarrido`. El único llamador es `corrida.rs:170` (`barrer(...)?;`) y compila sin cambio.
- `Fila`: sus literales están en `calibra.rs`, `src/oracle/mod.rs` y `tests/calibra.rs`. Todos actualizados. Los `Fila` de `tipos.rs`, `aprende.rs` y `track/` son otro struct.
- `CasoGuardado`: dos constructores en `calibra.rs`, ambos actualizados.
- `finalizar_casos`, `fila_de`, `detecta_item`: la firma no cambia.
- Fuentes: `codegraph callers` y `grep -rnw` unidos en el punto 3. En los puntos 1 y 2 se usó `grep -w`.

## 6. Preguntas para el planner

1. **Un hijo matado por la señal pierde su stdout parcial.** Decisión: la espera da un error genérico. El proceso sale justo después, así que el stdout no sirve.
2. **La limpieza de la señal corre bajo el cerrojo y puede tardar hasta 10 s.** Decisión: se acepta. Es lo que pide el brief, y un lanzamiento nuevo no debe entrar durante la limpieza.
3. **Si el borrado del barrido sale bien y el `gate` falla, la corrida queda en `barridas` y en `con_error`.** Decisión: se deja así. Informa las dos verdades.
4. **Un margen de 300 s no separa un `r00t` ajeno que arrancó dentro de esa ventana.** Decisión: se acepta. El plan lo declara casi imposible.
5. **`omisiones` suma sobre defectos y limpios.** Decisión: suma sobre todos los casos. El plan habla de "los casos".
6. **Un caso sin veredicto de la dimensión no lista omitidos.** Decisión: ese caso ya cuenta aparte como sin veredicto.
7. **`piso` solo existe en casos con defecto.** Decisión: un limpio no tiene ítem apuntado.
8. **El nivel base de `piso` es el de la base efectiva** (sin ítems inestables). Decisión: es el nivel que usa `detecta_item`.
9. **Las líneas de `stderr` del barrido no tienen tope de volumen.** Decisión: no se limita. Una raíz con muchas entradas rotas es un caso raro.
