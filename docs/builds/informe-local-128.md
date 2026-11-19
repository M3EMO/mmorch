# Informe de la Tarea 128

## 1. Resultado de la suite

Todas las cifras salen de `tools/test-seguro.sh` con `ROOT_SIN_PODMAN=1`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (352fd99) | 3536 | 0 | 5 |
| Final (7089bab) | 3550 | 0 | 5 |

El hook de commit no falló por tiempo. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Los casos de la calibración evaluaban con todos los ítems inestables. | `calibra_casos_ven_los_inestables_de_la_base_y_la_base_no`, `oraculo_calibracion_cubre_6_de_8_de_peso_estable_con_los_inestables_fuera`. Guardas: `excluidos_fuera_de_calibracion_dan_los_de_la_fila`, `excluidos_en_calibracion_sin_valor_de_hilo_dan_vacio`. | cca15c2 |
| 2 | Un defecto sin veredicto contaba como no detectado. | `defecto_sin_veredicto_no_entra_a_la_sensibilidad`, `oraculo_tres_defectos_sin_veredicto_de_diez_no_calibra`, `version_suma_defectos_sin_veredicto_de_las_filas_que_cuentan`. Guardas: `fila_vieja_sin_defectos_sin_veredicto_se_lee_con_cero`, `oraculo_dos_defectos_sin_veredicto_de_diez_calibra`. | 2e3ef4c |
| 3 | La rotación ignoraba los defectos en piso. | `rotacion_prefiere_el_defecto_cuyo_item_puede_bajar`, `rotacion_defecto_sin_nivel_base_cuenta_como_apto`, `limpio_toma_el_item_del_defecto_que_le_toca_segun_la_rotacion`. Guardas: `rotacion_con_los_dos_items_en_piso_alterna_los_dos`, `rotacion_con_un_solo_defecto_es_ese_defecto`. | 7089bab |

Diseño de cada punto:

- Punto 1: un valor de hilo `INESTABLES_DE_CALIBRACION` en `src/oracle/mod.rs`. `evaluar_copia` lo fija para cada caso. La base no lo recibe.
- Punto 2: `Fila` y `Version` suman `defectos_sin_veredicto`. La constante pasa a `MAX_SIN_VEREDICTO_PCT`.
- Punto 3: `defectos_a_rotar` filtra con `en_piso`. `Ronda.defectos` pasa a `Ronda.rotacion`.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- Ningún test con podman corrió. Quedan para el orquestador.
- La repetición de P101 a para Diseño espera la respuesta a P107.
- El test rojo del punto 1, parte "6 de 8", prueba `excluidos_de`, `peso_total` y `puntaje_diseno`. No usa el evaluador falso completo.
- Las métricas por ítem de la sensibilidad cambian con el punto 2 y no tienen test propio.

## 4. Las filas viejas

- Cero tests cambian la sensibilidad de una fila vieja. La regla nueva actúa solo en `fila_de`, que crea filas nuevas.
- Una fila leída del JSON conserva su sensibilidad guardada. Su `defectos_sin_veredicto` vale 0.
- Esas filas contaban los defectos sin veredicto como no detectados. Por eso eran más estrictas, no más laxas.

## 5. Tests que cambiaron de valor

- `calibra_defecto_sin_veredicto_es_falso_negativo` pasa a `calibra_defecto_sin_veredicto_no_es_falso_negativo`. La sensibilidad pasa de `Some(0.0)` a `None`. La exactitud balanceada pasa de `Some(0.5)` a `None`. Sin defectos con veredicto no hay clase que medir.
- `calibra_defecto_sin_veredicto_no_suma_a_limpios_sin_veredicto`: la sensibilidad pasa de `Some(0.0)` a `None`. Suma una aserción sobre `defectos_sin_veredicto`.

Tests de otros archivos que edité. Solo sumé el campo nuevo a literales:

- `src/lib.rs`: un literal de `Version` en `version_de`.
- `src/oracle/mod.rs`: tres literales de `Fila` en tests, más tests nuevos del punto 1.
- `tests/calibra.rs`: tres literales de `Fila`.

## 6. Llamadores revisados

| Símbolo | Cambio | Codegraph | grep -w |
|---|---|---|---|
| `excluidos_de` | Lee el valor de hilo en calibración. | `evaluar` (dos usos) y los tests nuevos. | `src/oracle/mod.rs` (1240, 1256) y los tests. |
| `defecto_de` | Recibe `&[Defecto]` en vez de `&Ronda`. | `medir_caso`, `preelegir`, test `elegidos_en_pares`. | Solo `src/oracle/calibra.rs`. |
| `item_y_lugar_de` | Recibe `rotacion` en vez de `dimension`. | `finalizar_casos` y un test. | Solo `src/oracle/calibra.rs`. |
| `MAX_LIMPIOS_SIN_VEREDICTO_PCT` | Renombrada a `MAX_SIN_VEREDICTO_PCT`. | Sin símbolo viejo. | Solo `Version::calibrada`. Los informes en `docs/builds/` conservan el nombre viejo. |
| `Fila`, `Version` | Campo nuevo `defectos_sin_veredicto`. | Cada literal se halló por error de compilación. | Literales en `calibra.rs`, `lib.rs`, `oracle/mod.rs`, `tests/calibra.rs`. |

`item_y_lugar_de` no estaba en el brief. Asignaba el ítem a cada limpio con la lista completa de defectos. Con la rotación nueva, los limpios medían el ítem equivocado. El test `limpio_toma_el_item_del_defecto_que_le_toca_segun_la_rotacion` lo muestra.

## 7. Preguntas para el planner

1. **Tope de defectos sin veredicto.** Decisión: el total son `defectos + defectos_sin_veredicto` de la versión. El tope es el 20 % de ese total. Confirmar.
2. **Filas viejas en una versión.** Decisión: se suman con 0. El tope del 20 % de defectos ignora sus casos sin veredicto, y su sensibilidad sigue con la regla vieja. ¿Se recalibran?
3. **Identidad con copias.** `defectos_distintos` conserva el caso de menor índice por identidad. Si ese caso no tiene veredicto y otra copia sí, la identidad cuenta sin veredicto. Decisión: no se cambió.
4. **Filas ya guardadas.** Sus casos traían los ítems inestables. Decisión: no cambié `huella_de` ni invalidé filas. ¿Hace falta?
5. **Limpios compartidos.** Los limpios se comparten entre Estructura, Arquitectura y Diseño. Los excluidos solo tocan ids `diseno.*`. Decisión: no cambié `Compartidos`. Conviene que el orquestador lo confirme.
6. **Ítem en piso sin defectos.** Mientras otro ítem pueda bajar, un ítem en piso no recibe defectos. Su sensibilidad por ítem deja de medirse. Decisión: se aplica el brief tal cual.
7. **Varios ítems en piso.** Si ninguno puede bajar, la rotación usa todos, como pide el brief.
8. **Rotación y caché.** No revisé si algún caché guarda la rotación vieja. Decisión: sin cambios.
