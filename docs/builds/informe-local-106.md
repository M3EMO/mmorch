# Informe de la Tarea 106

Rama `laptop/tarea-106`, base 3a1ed58. Sonnet organizó y revisó. Haiku construyó cada punto.

## 1. Resultado de la suite

Corrida con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (3a1ed58) | 3255 | 0 | 4 |
| Final | 3286 | 0 | 4 |

`cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` pasan. Los commits no tuvieron fallas de tiempo en el hook.

## 2. Commits

El punto 4 se construyó en un worktree aparte (`local106-p4`) y entró por `cherry-pick`. El hash original es ea755bb.

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 4, duración por mutante | `MutanteEstado` no traía la duración | `mutante_estado_trae_la_duracion_de_cada_mutante`. Guarda: `reporte_sin_duraciones_da_el_estado_de_hoy` | 7a9331e |
| 1, elección sin reposición | Varios casos medían la misma inyección | `un_solo_candidato_deja_los_casos_con_defecto_repetidos_como_agotados`, `casos_con_defecto_inyectan_identidades_distintas`, `reemplazo_no_elige_ni_una_identidad_usada_ni_una_invalida`, `elegir_libre_no_elige_un_excluido_y_sin_restantes_no_elige_nada`, `excluir_la_elegida_no_la_repite_en_cada_defecto_con_identidad_de_linea`, `dockerfile_excluido_no_es_aplicable_y_no_toca_el_repo`, `assert_borrado_salta_el_mutante_excluido`, dos de identidad. Guardas: `elegir_libre_sin_exclusiones_elige_como_elegir`, `misma_semilla_inyecta_lo_mismo_en_cada_caso` | 602fe0b |
| 2, `inyecciones_distintas` | La fila contaba casos, no inyecciones distintas | `fila_nueva_cuenta_inyecciones_distintas_igual_a_sus_defectos`, `identidad_repetida_cuenta_una_vez_en_sensibilidad_y_en_distintas`, `muestra_suficiente_cuenta_inyecciones_distintas_no_defectos`, dos de `texto_inyecciones`. Guarda: `fila_vieja_sin_inyecciones_distintas_se_lee_none_y_calibra_como_hoy` | aad09f0 |
| 3, equivalentes curados | Un equivalente conocido se podía inyectar | `equivalente_declarado_no_se_elige_nunca`, `equivalente_sin_motivo_es_error_de_carga_con_archivo_y_linea`, `fila_cuenta_los_equivalentes_declarados`, `equivalente_declarado_nunca_se_inyecta_y_el_sobrante_queda_agotado`. Guardas: `calibracion_sin_tabla_queda_none`, `calibracion_sin_equivalentes_trae_lista_vacia`, `sin_la_lista_la_misma_linea_si_se_elige` | 658e8d7 |

## 3. Qué falló o quedó incompleto

- Todos los puntos están hechos y probados con ejecutor falso.
- No se corrió la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.
- Ningún test con podman corrió aquí.
- Falta, y es del orquestador: agregar a `oraculos/s2-t1/tarea.toml` el equivalente `composite_pattern.rs:113`, `true -> false`, con su motivo.
- `docs/interfaces.md` se regeneró tres veces, no una. El hook compara el esquema en cada commit. Los puntos 4, 2 y 3 cambian el esquema.
- Archivos fuera de la tabla del brief, todos para compilar o cablear:
  - `src/oracle/calibra/defectos.rs`: una línea, el literal de `Contexto`.
  - `src/oracle/mod.rs` y `tests/calibra.rs`: literales de `Fila`.
  - `src/oracle/calibra.rs` en el punto 3: cablea la lista curada y el contador.
  - `src/oracle/calibra.rs` en el punto 4: una línea, el helper de tests `mutante_estado`.
- `MutanteEstado` perdió el derive `Eq`, porque `f64` no lo admite. Nada lo usaba.

## 4. Identidad de una inyección

La identidad es `(defecto, archivo, línea, edición)`. `Inyeccion::identidad()` la arma después de aplicar. Cada función del inyector arma la misma identidad antes de elegir.

| Defecto | Archivo | Línea | Edición |
|---|---|---|---|
| `ComportamientoRoto` con estado inicial | el del candidato | línea de la edición | operador de la Tarea 98, p. ej. `true -> false` |
| `ComportamientoRoto` sin estado inicial | el del candidato | línea de la igualdad | vacía |
| `CuerpoVaciado` | el del candidato | línea de la firma | vacía |
| `PanicoEnEntrada`, `ErrorTragado` | el del candidato | línea cambiada | vacía |
| `BucleCuadratico` | el del candidato | línea del bucle insertado | vacía |
| `AssertBorrado` | archivo del mutante muerto | 0 | nombre del mutante |
| `SecretoEnCodigo`, `CicloEntreCapas`, `ArchivoDios`, `ModuloBorrado`, `DockerfileRoot` | el afectado | 0 | vacía |

Los defectos de archivo entero normalizan la línea a 0. Dos ciclos en el mismo archivo con capas distintas comparten identidad.

## 5. Tests que cambiaron de valor

Cada cambio sale de una regla nueva: un caso no repite inyección. Los casos que antes repetían ahora quedan "candidatos distintos agotados".

1. `calibra_camino_caliente_con_funcion_existente_aplica_el_bucle`: defectos 5 a 1, no aplicables 0 a 4. Hay un solo candidato.
2. `calibra_cinco_defectos_y_cinco_limpios_acertados_si_califica`: n pasa de 10 a 20. Con 10 casos quedan 4 defectos, bajo el mínimo de 5.
3. `calibra_completitud_con_no_compila_reemplaza_y_no_mide`: reemplazos 6 a 5. El fixture tiene 5 funciones.
4. `calibra_con_bootstrap_los_defectos_de_linea_caen_en_el_cambio`: casos con defecto de 4 o más a exactamente 1. El cambio tiene un solo `==`.
5. `calibra_detector_perfecto_da_1_en_sensibilidad_especificidad_y_exactitud`: (10, 10, 0) a (5, 10, 5). `PanicoEnEntrada` tiene un candidato.
6. `calibra_inyeccion_invalida_agotada_es_no_aplicable_y_no_mide`: reemplazos 6 a 3. `ArchivoDios` tiene tres módulos.
7. `calibra_inyeccion_invalida_agotada_guarda_el_motivo_del_no_aplicable`: el motivo pasa a "no aplica: candidatos distintos agotados".
8. `calibra_diseno_tras_arquitectura_con_evaluador_real_no_llama_al_juez_en_limpios`: defectos evaluados de 2 a 1. El renglón agotado no tiene archivo.
9. Dos tests de conteo de llamadas al evaluador (mutación de la base y sondas deterministas de un limpio): la fórmula pasa de `medidos + 1 + extra` a `medidos + extra`.
10. `calibra_por_item_mide_cada_item_y_omite_lo_no_medido`: sensibilidad 0,5 a 0,8 y exactitud 0,75 a 0,9. Hay 4 defectos medidos.
11. `sonda_sin_candidatos_ejecutados_es_no_aplicable_con_motivo_de_sonda`: reemplazos 6 a 1 y motivo nuevo. Se borró la constante `MOTIVO_EXACTO`.

## 6. Llamadores revisados

Se unieron los llamadores de `codegraph callers` con los de `grep -w`.

| Símbolo | Cambio | Llamadores |
|---|---|---|
| `Contexto` de `inyectar.rs` | campo nuevo `excluidos` | grep: `inyectar.rs` (función `inyectar` y 7 tests), `calibra.rs` (`inyectar_en`), `defectos.rs:192`. Todos pasan el campo. |
| `inyectar_con` | no cambia la firma, cambia su `Contexto` | codegraph: 17 llamadores, `inyectar_en` y tests. grep: 26 líneas, mismos sitios. |
| `Fila` | dos campos nuevos con `serde(default)` | literales en `calibra.rs`, `oracle/mod.rs`, `tests/calibra.rs`. Todos pasan el valor. |
| `muestra_suficiente` | usa `inyecciones_distintas` | codegraph: `aplicar` y `motivo_sin_calibrar`. grep: igual más tests. |
| `MutanteEstado` | campo nuevo, sin `Eq` | un literal fuera de `mutacion.rs`, el helper de tests de `calibra.rs`. |
| `parsear_outcomes` | firma igual | 20 llamadores de codegraph, todos tests de `mutacion.rs`. No cambian. |

## 7. Preguntas para el planner

1. **Casos agotados bajan el mínimo.** Un defecto con pocos candidatos deja casos agotados. Con Seguridad del fixture y n = 10 salen 4 defectos, y el mínimo es 5. Decisión: se acepta. Para repos chicos, el planner decide si sube `n`.
2. **Motivo de un reemplazo sin candidatos.** Un reemplazo sin candidato distinto da siempre "candidatos distintos agotados". Se pierden el motivo de sonda y "inyecciones inválidas agotadas" en ese caso. Decisión: se conserva el nuevo, porque la regla del brief lo pide.
3. **Identidad de archivo entero.** Su línea vale 0, y dos ciclos en el mismo archivo colapsan. Decisión: se acepta. El brief pide la línea, y estos defectos no tienen una antes de aplicarse.
4. **Equivalente sin estado inicial.** Un equivalente con operador (`true -> false`) solo aplica con `bootstrap/`. Sin él, `ComportamientoRoto` usa edición vacía y la identidad no coincide. Una edición vacía excluye el `==` a `!=` de esa línea. Decisión: se declara siempre la edición.
5. **Qué cuenta `equivalentes_excluidos`.** Cuenta las entradas declaradas sin duplicados, en toda dimensión. No cuenta los candidatos que realmente se quitaron. Decisión: es lo más simple y lo único que se sabe sin aplicar.
6. **Entradas duplicadas.** Dos entradas iguales cuentan una vez y la carga no las rechaza. El planner decide si deben dar error.
7. **Equivalente de `CuerpoVaciado`.** Con edición vacía o `cuerpo vaciado`, la entrada excluye también el cuerpo vaciado cuya firma está en esa línea. El brief define la edición como "operador o tipo de cuerpo vaciado" sin dar el texto exacto. Decisión: `cuerpo vaciado`.
8. **Sensibilidad por ítem.** La deduplicación por identidad también cubre la sensibilidad por ítem. Con identidades distintas no cambia nada.
9. **Elección en serie con copia.** La preelección aplica cada defecto sobre una copia del repo, y el caso lo aplica de nuevo. Cuesta una copia más por caso con defecto. Decisión: se acepta por simplicidad.
10. **Cerrojo de reemplazos.** El cerrojo cubre la elección y la suma de un reemplazo. Un reemplazo concurrente no puede repetir la misma inyección. El orden entre reemplazos de hilos distintos no es determinista. La primera elección de cada caso sí lo es.
