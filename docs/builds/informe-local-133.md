# Informe de la Tarea 133

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (56fd3eb) | 3586 | 0 | 5 |
| Final (punto 3) | 3605 | 0 | 5 |

El hook de los commits corrió la suite en cada commit. Ningún test de tiempo falló. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Los renglones de `.casos.jsonl` y la fila de la base no guardaban las medidas de Eficiencia. | `caso_de_eficiencia_guarda_sus_medidas_en_el_renglon`, `la_base_guarda_sus_medidas_en_la_fila`. Guardas: `renglon_viejo_sin_eficiencia_se_lee`, `fila_vieja_sin_eficiencia_base_se_lee`. | 399ff42 |
| 2 | El caso comparaba contra la referencia de la base sin ver el cambio de la máquina. | `ancla_sube_el_ruido_y_quita_la_falsa_regresion`, `con_referencia_el_inicial_recibe_una_llamada_por_medida_de_tiempo`, `con_referencia_el_orden_de_las_corridas_es_repo_inicial_repo_repo`, `con_referencia_el_inicial_solo_recibe_su_ancla`. Guardas: `ancla_no_tapa_una_regresion_real`, `sin_referencia_el_orden_alterna_como_hoy`, `con_referencia_un_ancla_con_error_se_ignora`. | 1360ddc |
| 3 | Cada caso creaba su carpeta y recompilaba las dependencias. | `con_carpeta_de_eficiencia_*` (no compilaban), `calibra_eficiencia_la_base_y_dos_casos_reciben_la_misma_carpeta_del_repo`, `calibra_eficiencia_la_carpeta_no_existe_despues_de_calibrar`, `calibra_eficiencia_con_carpeta_fija_corre_los_casos_en_serie`. Guardas: `calibra_eficiencia_la_carpeta_se_borra_tambien_si_un_caso_da_error`, `sin_carpeta_de_eficiencia_crea_y_borra_su_carpeta_como_hoy`. | 5876e8d |

Notas:
- El test `con_carpeta_de_eficiencia_que_no_se_puede_crear_da_cuatro_medidas_sin_valor` quedó como guarda.
- El primer intento del commit 2 falló en clippy (`% 3 == 0`). Se corrigió con `is_multiple_of` y el commit pasó.

## 3. Qué falló o quedó incompleto

- **Nada con podman se probó.** El reparto del tiempo y la calibración de t1 a t3 quedan para el orquestador, como pide el brief.
- **El ancla no tiene test con el contenedor real.** Solo se probó con ejecutores falsos.
- **Sin probar:** la carpeta de Eficiencia con `home = Some(...)`, la limpieza ante pánico dentro de una calibración, y `BaseRepetida` con carpeta fija. Eficiencia no usa juez, así que esa rama no corre.
- **Punto 3, paso 6 (copia del repo):** los comandos de Eficiencia no copian el repo. `/repo` se monta en solo lectura y los comandos hacen `cd /repo`. No hay copia que borrar, y no agregué ningún `rm`.
- **Riesgo de código viejo en el `target` fijo:** cada caso se copia con `fs::copy`, que da mtime nuevo. Cargo ve las fuentes más nuevas que el último build y recompila los paquetes del workspace. La medida de compilación limpia el paquete antes de medir. No lo probé con podman.
- **Codegraph no se corrió.** El worktree no tiene `.codegraph/` y no corrí `init`. Los llamadores salen solo de `grep -w`.

## 4. El orden de las llamadas de un caso

Con la referencia fijada y la carpeta fija. Leído del código y de los tests de orden, no de una corrida real:

| # | Lado | Llamada | Carpeta |
|---|---|---|---|
| 1 | repo | metadata | `<carpeta>/repo` |
| 2 | repo | lista | `<carpeta>/repo` |
| 3 | inicial | compilar tests (ancla) | `<carpeta>/inicial` |
| 4 | repo | compilar tests | `<carpeta>/repo` |
| 5 | repo | tests, corrida 0 | `<carpeta>/repo` |
| 6 | inicial | tests (ancla) | `<carpeta>/inicial` |
| 7 | repo | tests, corrida 1 | `<carpeta>/repo` |
| 8 | repo | tests, corrida 2 | `<carpeta>/repo` |
| 9 | repo | build, corrida 0 | `<carpeta>/repo` |
| 10 | inicial | build (ancla) | `<carpeta>/inicial` |
| 11 | repo | build, corrida 1 | `<carpeta>/repo` |
| 12 | repo | build, corrida 2 | `<carpeta>/repo` |

`<carpeta>` es `<tmp de la calibración>/eficiencia`. La base y todos los casos usan la misma.

## 5. Tests que cambiaron de valor

- `con_referencia_el_lado_inicial_no_recibe_llamadas` (eficiencia.rs) pasó a `con_referencia_el_inicial_solo_recibe_su_ancla`. El inicial ahora recibe `compilar_tests`, `tests` y `build`.
- `con_referencia_la_razon_usa_la_serie_guardada` (eficiencia.rs): la última aserción espera esas tres llamadas del inicial. Los valores y la razón 2,0 no cambian.
- `calibra_eficiencia_la_base_mide_los_dos_lados_y_cada_caso_mide_uno` (calibra.rs): esperaba 0 llamadas del inicial en los casos. Ahora espera un múltiplo de 3 mayor que 0, una por cada llamada del ancla. Es un test del mismo archivo de la tabla.

Tests de otros archivos editados (solo literales de `Fila`, que gana el campo `eficiencia_base`):
- `src/oracle/mod.rs`: tres literales de test, con `eficiencia_base: Vec::new()`.
- `tests/calibra.rs`: tres literales de test, con `eficiencia_base: Vec::new()`.

## 6. Llamadores revisados

- **`Fila` gana `eficiencia_base`** y **`CasoGuardado` gana `eficiencia`.** Son campos con `#[serde(default)]`. Los literales que fallaron al compilar son los de las tablas de arriba. `docs/interfaces.md` se regeneró y `tests/interfaces.rs` pasa.
- **`medir_contra_referencia`, `medir_tests`, `medir_compilacion`, `combinar` y `combinar_medidas`** son privadas de `eficiencia.rs`. `grep -w` no halla llamadores fuera de ese archivo. El `combinar` de `perfiles/mod.rs` es otra función.
- **`con_carpeta_de_eficiencia` y `comparar_con_ancla`** son nuevas. No tienen llamadores previos.
- **Codegraph:** no se corrió (ver sección 3).

## 7. Preguntas para el planner

1. **¿Dónde van las medidas de la base?** Decisión: en `Fila.eficiencia_base`, con el veredicto crudo de la base (`juicios[0].eficiencia_auto`). Es el lugar que ya guarda datos de la base, como `juicios_base`.
2. **¿Qué pasa si el ancla falla?** Decisión: se ignora y la referencia queda con la serie de la base. Una falla del ancla no debe dar una medida sin valor. Esto oculta una máquina rota en el ancla. Conviene decidir si debe avisar.
3. **¿Los casos de Eficiencia corren en serie?** Decisión: sí, `hilos = 1` con carpeta fija. Cuatro casos a la vez compartirían un `target` y solaparían las corridas de tiempo. Esto no cuesta casi nada, porque Eficiencia no usa juez. El brief no lo pedía.
4. **El ancla sube el costo de cada caso.** Cada caso corre el lado inicial una vez más por medida: compilar tests, una corrida de tests y una compilación. Decisión: aceptado. Es lo que pide el brief.
5. **Caso límite del test del ancla.** Con los valores del brief (base 1,00 1,01 1,00, ancla 1,05, repo 1,05), la razón queda justo en `1 + ruido`. El test pasa por igualdad exacta de flotante. Decisión: dejé los valores del brief. Un repo de 1,04 daría margen.
6. **`mod.rs` (~línea 841) llama a `medir` sin cambios.** Dentro de un caso, el valor de hilo hace que use la carpeta fija sin tocar ese archivo. Decisión: no toqué `mod.rs`.
