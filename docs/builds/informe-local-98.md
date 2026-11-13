# Informe de la Tarea 98

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (7af9a54): 3135 verdes, 0 fallas, 4 ignorados.
- Final: 3151 verdes, 0 fallas, 4 ignorados. Son 16 tests nuevos.
- Ninguna falla de tiempo en el hook. No hubo reintentos por tiempo.
- El primer intento del commit 1 falló por un aviso de clippy (`collapsible_if`). Se corrigió y se reintentó.
- El segundo intento falló en `tests/interfaces.rs`: faltaba el campo `operador` en `docs/interfaces.md`. Ver sección 3.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `comportamiento_roto` solo mutaba `==` | `comportamiento_con_inicial_cambia_mayor_por_mayor_o_igual_dentro_del_cambio`, `comportamiento_con_inicial_if_ofrece_or_y_negacion_de_la_condicion`, `comportamiento_con_inicial_cambia_true_y_mas_uno_por_su_par`, `comportamiento_con_inicial_sin_el_archivo_no_descarta_nada`. Guardas: `comportamiento_con_inicial_ignora_operadores_en_textos_y_comentarios`, `comportamiento_con_inicial_descarta_el_cambio_que_el_inicial_ya_tenia`, `comportamiento_sin_inicial_elige_con_la_semilla_como_antes`, `caso_guardado_lleva_el_operador_y_los_viejos_quedan_en_none` | 049b148 |
| 2 | Completitud solo tenía `modulo_borrado`, que da detección segura con estado inicial | `cuerpo_vaciado_cambia_el_cuerpo_de_una_funcion_que_existia_y_el_cambio_toco`, `cuerpo_vaciado_cuenta_llaves_fuera_de_cadenas_y_firma_multilinea`, `cuerpo_vaciado_la_copia_del_fixture_compila`, `modulo_borrado_con_inicial_no_aplica_aunque_haya_modulos_tocados`, `motivo_de_defecto_de_linea_sin_candidato_dentro_del_cambio_lo_dice`. Guardas: `cuerpo_vaciado_no_elige_una_funcion_nueva_del_cambio`, `cuerpo_vaciado_sin_inicial_no_aplica`, `cuerpo_vaciado_salta_declaraciones_cuerpos_de_una_linea_y_todo`, `modulo_borrado_sin_inicial_elige_con_la_semilla_como_antes` | 63b7adc |

## 3. Qué falló o quedó incompleto

- **Cambio de esquema.** `CasoGuardado` suma `operador: Option<String>` con `#[serde(default)]`. `tests/interfaces.rs` exige que `docs/interfaces.md` lo liste. Agregué su entrada, entre `omitidos` y `piso`. Toqué ese archivo de la Tarea 97 solo por eso.
- **`calibra.rs` tocado.** Cambios mínimos: `Ronda.inicial`, `Contexto.inicial`, `CasoGuardado.operador`, `MOTIVO_MODULO_CON_INICIAL`, `motivo_sin_inyeccion` y literales de tests.
- **`defectos.rs` tocado.** Una línea: `inicial: None` en un literal de `Contexto`.
- **`en_cambio` en `docs/interfaces.md`.** Su descripción nombra solo `ComportamientoRoto` y `ModuloBorrado`. Ahora incluye `CuerpoVaciado`. No la cambié: el planner decide.
- **Una guarda no se vio en rojo.** `comportamiento_con_inicial_descarta_el_cambio_que_el_inicial_ya_tenia` pasó en su primera versión. La versión final suma una aserción previa y no se corrió en rojo.
- **`CuerpoVaciado` sin comprobar que la suite lo ejecute.** `calibra.rs` invalida un `ComportamientoRoto` que la suite no ejecuta. Un `CuerpoVaciado` en una función sin tests no se invalida.
- **Funciones no candidatas.** `cuerpo_de` reconoce `fn`, `pub fn` y `pub(crate) fn`. `pub(super) fn`, `async fn` y `unsafe fn` no son candidatas.
- **Condición con `!=` al inicio.** Puede entrar como "agregar !". No es Rust válido y no aparece en código real.
- **No corrí** la validez, `corre`, `calibrar` ni la pasada reales. Ningún test nuevo necesita podman.

## 4. Candidatos en S2, sin API

Conté con el código real, sobre copias de `Canop_broot` (`git archive` del commit y de su padre) dentro de mi worktree. Conté solo líneas de producción dentro de los rangos del cambio. Borré las copias y el test temporal.

| Tarea | Commit | Operador | Candidatos | Tras anti revertir |
|---|---|---|---|---|
| t1 | 1f77962e6f | `true -> false` | 2 | 2 |
| t1 | | total | 2 | 2 |
| t2 | f8203f9eaa | `> -> >=` | 1 | 0 |
| t2 | | `if: agregar !` | 1 | 1 |
| t2 | | total | 2 | 1 |
| t3 | dfd25e0c22 | `&& -> \|\|` | 1 | 1 |
| t3 | | `< -> <=` | 1 | 1 |
| t3 | | `== -> !=` | 1 | 1 |
| t3 | | `if: agregar !` | 1 | 1 |
| t3 | | total | 4 | 4 |

- Hoy, con solo `==`: t1 da 0, t2 da 0 y t3 da 1.
- En t2, la regla anti revertir descarta `> -> >=`. Esa mutación recrearía el estado inicial.
- Los demás operadores dan 0 en los tres cambios.
- No conté los candidatos de `cuerpo_vaciado` en S2. El brief pide solo operadores.

## 5. Tests que cambiaron de valor

- `motivo_de_defecto_de_linea_sin_candidato_dentro_del_cambio_lo_dice` (`calibra.rs`). `ModuloBorrado` con rangos ahora da `MOTIVO_MODULO_CON_INICIAL`. Antes daba "ningún lugar del cambio admite este defecto". Es la regla 5 del punto 2.
- `inyectar_cada_defecto_apunta_a_su_dimension_e_item` y `inyectar_de_dimension_lista_sus_defectos_y_funciona_tiene_el_suyo` suman aserciones de `CuerpoVaciado`. Ningún valor anterior cambió.

## 6. Llamadores revisados

| Símbolo | Cambio | codegraph | grep -w |
|---|---|---|---|
| `comportamiento` | recibe `inicial` y devuelve el operador | `inyectar_con` | `inyectar_con` |
| `modulo_borrado` | recibe `inicial` | `inyectar_con` | `inyectar_con` |
| `Contexto` | campo `inicial` | literales en `inyectar`, `medir_caso`, `defectos.rs` y tests | los mismos, todos actualizados |
| `Inyeccion` | campo `operador` | literales en `inyectar_con` y en tests de `calibra.rs` | los mismos, todos actualizados |
| `TODOS` | pasa de 10 a 11 | solo `inyectar.rs` | solo `inyectar.rs` |

## 7. Preguntas para el planner

1. **Rotación de Completitud.** `de_dimension(Completitud)` ahora da `[ModuloBorrado, CuerpoVaciado]` y `calibrar` rota los casos entre ambos. La mitad de los casos de Completitud no aplica. Sin `bootstrap/` aplica solo `modulo_borrado`. Con `bootstrap/` aplica solo `cuerpo_vaciado`. Decisión: seguí el brief y no toqué la rotación. Propongo filtrar la lista según el estado inicial en `calibra.rs`.
2. **`CuerpoVaciado` en funciones sin tests.** Decisión: no agregué la comprobación de que la suite ejecute la función. Conviene una tarea aparte.
3. **Descripción de `en_cambio`.** Decisión: no toqué el comentario ni `docs/interfaces.md` para `CuerpoVaciado`.
4. **Operador `if: agregar !`.** Puede dar código que no compila, por ejemplo `!x == 1`. Funciona nunca cuenta una compilación rota. Decisión: lo dejé como pide el brief.
5. **Cruce del cuerpo.** Decisión: una función cruza el cambio si alguna línea desde su firma hasta su cierre, ambas incluidas, cae en un rango.
