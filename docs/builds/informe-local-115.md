# Informe de la Tarea 115

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base eee0aa0 (medida tras el punto 1, que solo cambia comentarios) | 3377 | 0 | 4 |
| Final 776e1ba | 3406 | 0 | 4 |

El hook de commit corre la suite. Ningún test de tiempo falló, así que no hubo reintento.
Hay tests con `saltar_sin_podman`: salen como "SALTEADO" y el orquestador los corre al mezclar.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Dos comentarios de `src/tipos.rs` y cuatro líneas de `docs/interfaces.md` nombraban reglas viejas. | Ninguno: el punto solo cambia comentarios. | 111d790 |
| 2 | Eficiencia no se medía contra el estado inicial. | `comparar_razon_y_ruido_sin_regresion`, `comparar_ruido_bajo_usa_el_piso_y_marca_regresion`, `medir_alterna_los_lados_en_tests_y_compilacion`, `test_que_pasa_solo_en_el_repo_entra_al_filtro_de_exclusion`, `sin_memory_peak_memoria_no_tiene_valor_y_tiempo_si`, `workspace_sin_binarios_deja_tamano_sin_valor`, `sin_tests_comunes_tiempo_y_memoria_no_tienen_valor` | 2e7135c |
| 3 | Sin `[eficiencia]`, la dimensión daba `no_aplica` aunque el cambio empeorara el producto. | `oraculo_eficiencia_rust_sin_tabla_con_estado_inicial_mide_sola`, `oraculo_eficiencia_auto_con_tests_1_3_veces_mas_lentos_da_077`, `oraculo_eficiencia_rust_sin_estado_inicial_sin_tabla_es_no_aplica_con_motivo`, `oraculo_eficiencia_banco_alto_deja_ganar_a_la_medicion`, `oraculo_eficiencia_banco_sin_valor_deja_decidir_a_la_medicion`, `oraculo_eficiencia_sin_medidas_da_sin_veredicto_con_motivos` | 26cb0f0 |
| 4 | Una regresión grande de eficiencia no daba "no listo". | `frase_de_regresion_da_las_cuatro_medidas_con_mib`, `informe_eficiencia_dos_regresiones_nombra_la_de_mayor_razon`, `informe_eficiencia_regresion_de_1_3_no_dispara_y_el_resumen_la_nombra`, `informe_eficiencia_regresion_de_1_6_sobre_el_ruido_da_no_listo_y_la_frase`, `informe_eficiencia_regresion_no_espera_la_calibracion`, `informe_eficiencia_sin_regresiones_lo_dice_en_el_resumen` | 776e1ba |

Guardas del punto 2: `comparar_vacios_o_minimo_no_positivo_dan_none`, `build_que_falla_deja_compilacion_y_tamano_sin_valor`, `medida_id_usa_los_nombres_estables`.
Guardas del punto 3: `oraculo_eficiencia_calibracion_de_diseno_no_llama_la_medicion`, `oraculo_eficiencia_no_rust_con_estado_inicial_no_mide_sola`, `oraculo_eficiencia_banco_bajo_gana_a_la_medicion`, `oraculo_eficiencia_sin_tabla_no_rust_es_no_aplica`, `veredicto_viejo_sin_eficiencia_auto_lee_vacio`. El último se escribió con el campo ya presente: no se vio en rojo.
Guardas del punto 4: `informe_eficiencia_regresion_dentro_del_ruido_no_dispara`, `informe_eficiencia_sin_medicion_no_suma_linea`, `informe_eficiencia_regresion_con_secreto_da_bloqueado`.

## 3. Qué falló o quedó incompleto

- **La memoria pico incluye la compilación.** Cada llamada a `ejecutar` usa un `/salida` nuevo. No hay `target` persistente entre llamadas.
  El comando de tests compila con `--no-run` en el mismo contenedor, y `memory.peak` incluye a `rustc`.
  El tiempo de tests no sufre: el reloj arranca después de la compilación.
  Arreglarlo pide un `/salida` fijo por ejecutor. Eso toca `EjecutorContenedor`, que se construye con literal en `src/oracle/calibra/defectos.rs` (archivo prohibido).
- **El `target` por lado no persiste.** Cada corrida de tiempo y de compilación parte de un `target` frío. El comando de compilación primero compila las dependencias y luego limpia solo los paquetes del workspace (`cargo clean -p`), así el reloj mide solo el workspace.
  El costo: cada corrida compila las dependencias de nuevo.
- **Nada con podman se probó.** Los comandos se revisaron con `bash -n` y con un ejecutor falso. Los corre el orquestador al mezclar.
- **No se probó que `memory.peak` exista dentro del contenedor.** Sin él, la memoria queda sin valor con el motivo "sin memory.peak".
- **`bc` no hace falta.** El tamaño se suma con `awk`.

## 4. Los comandos de cada medida

`PRELUDIO` es el prefijo de `src/oracle/descarga.rs`: fija `CARGO_HOME` y copia la config de `/cache`. Las funciones están en `src/oracle/eficiencia.rs`: `cmd_metadata`, `cmd_lista`, `cmd_tests` y `cmd_compilacion`. Su texto exacto sale de ellas, y sus doc-comments dicen cómo se lee cada salida.

| Comando | Qué hace | Cómo se lee la salida |
|---|---|---|
| `cmd_metadata` | `cargo metadata --no-deps --format-version 1 --offline > /salida/metadata.json` | `archivos["metadata.json"]`: paquetes y binarios (`kind` con "bin"). |
| `cmd_lista` | `cargo nextest run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml`, y copia el junit a `/salida/lista.xml` | `archivos["lista.xml"]`: los `testcase` y los que fallan. El código de salida se ignora. |
| `cmd_tests` | `cargo nextest run ... --no-run`, luego `date +%s%N`, la corrida con filtro `-E 'not (test(=a) \| ...)'`, otra `date` | Última línea `ROOT_NS <ns>` y última `ROOT_MEM <bytes>` (de `/sys/fs/cgroup/memory.peak`). |
| `cmd_compilacion` | `cargo build --release --offline`, `cargo clean --release -p <paquetes>`, reloj alrededor de un segundo `cargo build --release --offline` | `ROOT_NS <ns>`, `ROOT_BUILD <código>` y `ROOT_SIZE <bytes>` (suma de binarios con `stat -c %s`). |

Las corridas alternan lados: inicial, repo, inicial, repo, inicial, repo. Son 3 por medida, y se compara el mínimo. El tamaño usa 1 corrida.
El filtro excluye los tests que no pasan en los dos lados. Sin tests comunes, el tiempo y la memoria quedan sin valor.

## 5. Tests que cambiaron de valor

- `oraculo_eficiencia_sin_tabla_rust_es_no_aplica` (`src/oracle/mod.rs`): el motivo esperado pasó de "la tarea no define [eficiencia]" a "sin estado inicial". La tarea del test no tiene `bootstrap/`.

Ningún test de otro archivo se editó. `src/tipos.rs` y `src/oracle/mod.rs` sumaron el campo `eficiencia_auto` en sus propios literales de test.

## 6. Llamadores revisados

- `medir` (`src/oracle/mod.rs`, privada, suma el parámetro `auto`): codegraph da el llamador `evaluar_con`. Grep -w da la única llamada en `mod.rs:887`. Los otros `medir` del crate son otras funciones, como `auditar::medir`.
- `puntaje_de_eficiencia` (nueva): su único llamador es `medir`.
- `es_no_aplica` suma `MOTIVO_SIN_ESTADO_INICIAL`: lo leen `juntar_partes` y sus tests.
- `Veredicto` suma `eficiencia_auto`: los literales de producción están solo en `src/oracle/mod.rs`. Los de test se corrigieron.
- `comparar` (nueva, `eficiencia.rs`): su único llamador es `combinar`. Otros `comparar` del grep (`track::comparar`) son otras funciones.

## 7. Preguntas para el planner

1. **Memoria pico con `rustc` dentro.** Decisión: se midió igual y se anotó en el comando. Falta un `/salida` persistente por ejecutor para medir solo los tests.
2. **El global cambia en toda tarea Rust con `bootstrap/` sin `[eficiencia]`.** Antes la dimensión era `no_aplica`. Decisión: cuenta, como pide el brief.
3. **Calibrar Eficiencia corre la medición automática.** `partes.mide(Eficiencia)` es verdadero. Decisión: corre, como pide el brief. ¿Debe entrar a la base de calibración?
4. **Costo de tiempo.** Cada evaluación Rust con `bootstrap/` suma 2 listas, 6 corridas de tests y 6 de compilación. Decisión: se acepta. ¿Es aceptable para calibraciones largas?
5. **Piso de ruido de 2 %.** En una máquina cargada puede marcar regresión por ruido y bajar el valor. Decisión: se dejó el 2 % de Perfherder. El "no listo" exige además más de 1,5 veces.
6. **Escala 1 / razón.** Un repo 1,6 veces más lento vale 0,625. Decisión: la del brief (referencia / medida).
7. **Regresión sin frase.** Una medida con id desconocido y razón mayor que 1,5 no dispara. Hoy no ocurre. Decisión: no disparar.
8. **Empate de razones en el resumen.** Con dos regresiones de igual razón, el resumen nombra la última del vector. Decisión: sin desempate extra.
9. **El resumen no muestra el ruido.** Solo razón y los dos valores, como el ejemplo del brief.
10. **Nombres de test con símbolos raros.** Un nombre fuera de `[A-Za-z0-9_:]` deja tiempo y memoria sin valor con el motivo "nombre de test no apto para el filtro". Decisión: preferir sin valor antes que un filtro inseguro.
