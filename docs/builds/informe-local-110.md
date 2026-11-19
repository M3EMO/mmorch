# Informe de la Tarea 110 (builder local)

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 64361fa | 3326 | 0 | 4 |
| Después del punto 1 | 3331 | 0 | 4 |
| Final (punto 2) | 3339 | 0 | 4 |

Los 4 ignorados son los de podman. El orquestador los corre al mezclar.
No hubo tests de tiempo fallidos en el hook. Ningún commit se reintentó.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `sin_veredicto` mezclaba "no aplica" con "no se pudo medir" | `oraculo_eficiencia_sin_tabla_rust_es_no_aplica`, `oraculo_eficiencia_sin_tabla_no_rust_es_no_aplica`, `oraculo_entrega_sin_entrega_es_no_aplica`, `oraculo_arquitectura_sin_capas_ni_modulos_suma_el_faltante_sin_modulos` | 967096b |
| 2 | "Listo" salía con dimensiones sin veredicto | `informe_diseno_sin_veredicto_da_sin_decision_y_nombra_la_regla`, `informe_diseno_inestable_en_la_tarea_da_sin_decision`, `informe_resumen_sin_veredicto_nombra_la_dimension_sin_medir`, `informe_resumen_sin_veredicto_nombra_las_dos_dimensiones_sin_medir`, `informe_veredicto_viejo_sin_no_aplica_con_diseno_sin_veredicto_da_sin_decision` | a6dae11 |

Guardas que ya estaban en verde (no probaron un defecto):
- Punto 1: Diseño sin juez da `sin_veredicto` y no `no_aplica`. Un JSON viejo se lee con `no_aplica` vacío.
- Punto 2: `no_aplica` deja "listo". Un test oculto en rojo da "no listo". Un secreto da "bloqueado".

Nota de proceso: el subagente del punto 1 vio los rojos en una copia del árbol, no en la rama.
La copia existía porque `informe.rs` no compilaba sin los literales nuevos.
Yo agregué esos literales en el commit del punto 1, como permite el brief.

## 3. Qué falló o quedó incompleto

- Lectores de calibración quedan con la lectura vieja. Ver pregunta 1.
- La regla de "inestable" no tiene constante propia. Ver pregunta 3.
- No corrí la validez, `corre`, `calibrar` ni la pasada reales, como pide el brief.
- Ningún test con `saltar_sin_podman` es nuevo.

## 4. "No aplica" contra "no se pudo medir"

Cada lugar revisado y su decisión:

| Lugar | Decisión |
|---|---|
| `perfiles/herramientas.rs` `eficiencia()`, sin `[eficiencia]` | No aplica. Motivo `MOTIVO_SIN_EFICIENCIA`. |
| `mod.rs` `medir`, Eficiencia sin `[eficiencia]` en lenguaje no Rust | No aplica. Antes daba "sin sonda para <lenguaje>". |
| `entrega.rs` `MOTIVO_SIN_ITEMS` (nada propio que medir) | No aplica. |
| `perfiles/rust.rs` `MOTIVO_SIN_MODULOS` (Arquitectura) | No aplica, solo si ninguna parte del juez falta. |
| `mod.rs` `NO_MEDIDA` (la calibración no pidió la dimensión) | No se pudo medir. |
| Completitud sin requisito medido | No se pudo medir. |
| Juez no corrido, causa del juez, rúbrica inválida, cobertura menor a dos tercios | No se pudo medir. |
| Diseño sin rúbricas o sin juez | No se pudo medir. |
| Funciona sin tests o con escenarios sin medir | No se pudo medir. |
| Entrega con causas de build (`sin_veredicto_sonda`) | No se pudo medir. |
| Sondas con error de herramienta, de configs o de mutación | No se pudo medir. |
| `calibrada` de un puntaje `no_aplica` | Siempre falso. Lo fuerza `evaluar_con`. |

Regla en `juntar_partes`: el puntaje es `no_aplica` solo si todos los motivos de las partes son de "no aplica".
Un motivo mixto, como "sin módulos" más "juez no corrido", sigue como `sin_veredicto`.
Con rúbricas reales, Arquitectura tiene ítems del juez. Entonces "sin módulos" solo da un faltante, y el juez da el valor.
Por eso Arquitectura da `no_aplica` solo cuando no hay parte del juez, como en los tests.

## 5. Tests que cambiaron de valor

| Test | Antes | Ahora | Motivo |
|---|---|---|---|
| `oraculo_arquitectura_sin_capas_ni_modulos_suma_el_faltante_sin_modulos` | `sin_veredicto` "sin módulos" | `no_aplica` "sin módulos" | Pedido por el brief. |
| `oraculo_sin_dockerfile_no_llama_a_podman_y_entrega_queda_sin_veredicto` (ahora `..._queda_no_aplica`) | `sin_veredicto` | `no_aplica` | Entrega sin `[entrega]` ni Dockerfile no aplica. |
| `oraculo_calibracion_marca_calibradas_y_global_es_la_de_seguridad` | filtro `sin_veredicto` | filtro `!sin_valor()` | `sin_calibrar` ya no lista dimensiones `no_aplica`. |
| `oraculo_con_sondas_sanas_dejan_sin_veredicto_arquitectura_diseno_y_entrega` | filtro `sin_veredicto` | filtro `sin_valor()` | Mismo resultado. |
| `informe_eficiencia_sin_veredicto_deja_listo_y_la_lista_en_confianza` (ahora `..._da_sin_decision_...`) | `Listo` | `SinDecision` | Eficiencia sin sondas es "no se pudo medir". |
| `oraculo_el_estado_sale_de_la_decision_y_no_del_conteo` | segunda corrida `Listo` y `Verde` | `SinDecision` y `SinVeredicto` | El fixture no tiene juez: Diseño queda sin veredicto. El test sigue mostrando que el conteo no decide. |

## 6. Llamadores revisados

No hay firma cambiada ni símbolo borrado. Hay símbolos nuevos.
Se une `codegraph callers` (tras `codegraph sync`) con `grep -w`.

| Símbolo nuevo | Codegraph | grep -w |
|---|---|---|
| `Puntaje::sin_valor` | 6 | 10 (con tests) |
| `es_no_aplica` | 2 | 1 |
| `puntaje_no_aplica` | 2 | 1 |
| `sin_veredicto_de` | 3 | 2 |

Cambios de visibilidad: `MOTIVO_SIN_ITEMS` pasa a `pub(crate)` y `MOTIVO_SIN_EFICIENCIA` es `pub`.
El campo `Puntaje.no_aplica` suma un literal en cada constructor: se agregó `no_aplica: None` en 14 archivos.

Lectores de `.sin_veredicto` fuera de mi tabla que no cambié:
- `calibra.rs`: `aplicar`, `global_de`, `motivo_con_juez`, `motivo_de_deteccion` y las reglas de mutantes.
- `calibra/regla.rs:43` y `calibra/compartidos.rs:88`.
- `informe.rs:543`, de `informe.rs`, no se cambió: hoy no importa porque `no_aplica` nunca es calibrada.

## 7. Preguntas para el planner

1. **Lectores de calibración.** Leen `sin_veredicto.is_some()` como "sin valor". Un puntaje `no_aplica` tiene `valor` 0 y `sin_veredicto` vacío, así que esos lectores lo ven como un 0 medido.
   - Decisión: no los toqué, porque están fuera de la tabla. Mitigué dos puntos: `calibrada` queda falso y `sin_calibrar` usa `sin_valor()`.
   - Propuesta: una tarea que cambie esos lectores a `!sin_valor()`. La corrida de `calibrar` de Entrega en brasa es el caso que más importa.
2. **Singular y plural de "Sin medir".** El brief dice "con una sola, va en singular". No hay forma gramatical distinta para esa palabra.
   - Decisión: el texto es "Sin medir: Diseño." con una y "Sin medir: Arquitectura y Diseño." con varias. Cambia solo la lista.
3. **Dimensión inestable.** Ya llega como `sin_veredicto` con el prefijo "inestable en esta tarea", así que cuenta sin código extra.
   - Decisión: la lectura vive en `sin_veredicto_de` (`informe.rs`), con un comentario que nombra P104. No agregué una constante del prefijo, porque quedaría sin uso.
   - Si P104 relaja la regla, el cambio va en esa función.
4. **Contrato visible.** `Veredicto.sin_calibrar` ya no lista dimensiones `no_aplica`. `Informe.confianza.sin_veredicto` tampoco las lista.
   - Decisión: lo dejé así. Un lector externo que contaba esas dimensiones ahora ve menos.
5. **Eficiencia sin `[eficiencia]` en lenguajes que no son Rust.** Antes daba "sin sonda para <lenguaje>". Ahora da `no_aplica`.
   - Decisión: la tarea no pide la dimensión, así que es "no aplica" en cualquier lenguaje.
