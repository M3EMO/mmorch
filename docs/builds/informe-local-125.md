# Informe de la Tarea 125

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base b6245d9 | 3491 | 0 | 5 |
| Final | 3508 | 0 | 5 |

La primera corrida final dio una falla: `corre::ejecutor::tests::vencimiento_sigue_borrando_el_contenedor_por_nombre`.
Este test es de tiempo y vive en un archivo que esta tarea no toca.
Corrió solo y pasó. La segunda corrida completa dio 0 fallas.
Los commits pasaron el hook de pre-commit al primer intento, sin reintento.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Una coma final tiraba el juicio con "JSON inválido". | `parsear_repara_coma_final_en_objeto_y_arreglo`, `parsear_repara_coma_final_dentro_de_bloque_json`, `quitar_comas_finales_quita_coma_con_espacios_y_saltos`, `quitar_comas_finales_quita_coma_tras_comilla_escapada`, `quitar_comas_finales_respeta_barra_escapada`, `juzgar_con_coma_final_trae_el_descarte_de_reparacion` | 70b555e |
| 2 | Un JSON roto no tenía reintento. | `juez_json_cortado_reintenta_y_juzga_con_la_segunda_respuesta`, `juez_reintento_por_json_marca_el_prompt_y_agrega_la_linea`, `juez_dos_json_cortados_dan_sin_veredicto_con_las_dos_causas` | 341d80c |

Guardas que pasaron desde el inicio:
- Punto 1: coma dentro de texto, comilla escapada, JSON cortado, reparo fallido con el error estricto, sin coma sin descarte.
- Punto 2: error de proveedor sin reintento, respuesta truncada sin reintento, huella sin cambio.

Nota de proceso: para compilar los tests en rojo, el subagente del punto 1 agregó un andamiaje mínimo. La implementación real lo reemplazó.

## 3. Qué falló o quedó incompleto

- No hay puntos sin hacer.
- No corrí la repetición de P101 a ni ningún test con podman. Las corre el orquestador al mezclar.
- No hay tests con `saltar_sin_podman` nuevos.

## 4. La huella

La huella no cambió. Un test guarda los tres valores.

| Valor | Antes | Después |
|---|---|---|
| `huella_del_prompt()` | 9263145aa4df4c7a | 9263145aa4df4c7a |
| `huella_de(Diseno)` | 19b81dea4119d429 | 19b81dea4119d429 |
| `huella_de(Arquitectura)` | 60c5fb9b0d576398 | 60c5fb9b0d576398 |

`LINEA_REINTENTO_JSON` queda fuera de `huella_del_prompt`.

## 5. Tests que cambiaron de valor

- Ninguno cambió de valor ni de aserción.
- No edité tests de otros archivos.
- `juez_json_roto_da_sin_veredicto` y `juez_no_truncado_json_roto_sigue_con_su_causa` ahora hacen 2 llamadas al modelo falso. Siguen verdes porque solo miran el prefijo "JSON inválido".

## 6. Llamadores revisados

- Ninguna firma pública cambió.
- `pedir_con_reintento_json`, `quitar_comas_finales` y `leer_json` son funciones privadas nuevas.
- `Respuesta` es una struct privada de `juez.rs`. Ganó el campo `descartes`.
- `grep -w pedir` y `grep -w parsear` en `src/oracle/` fuera de `juez.rs` solo muestran coincidencias de texto en `colab.rs` y `junit::parsear`, que es otra función.
- Codegraph no está inicializado en este worktree. No corrí `init`, por el disco casi lleno. El grep cubre las dos funciones.

## 7. Preguntas para el planner

1. **¿Qué errores cuentan como "JSON inválido" para el reintento?**
   Decisión: solo los que empiezan con "JSON inválido", es decir, los que `serde_json` rechaza.
   Un objeto sin `items` o un arreglo en lugar de objeto no se reintentan. Son JSON válido con forma errónea.
2. **¿El reintento por JSON reintenta si el prompt con la línea extra supera el límite de entrada?**
   Decisión: no. El juicio queda sin veredicto con la causa original.
3. **¿Cuánto gasta el reintento?**
   Un JSON roto ahora cuesta dos llamadas al juez. El reintento lleva `reintento_juez` y pasa por el mismo control de gasto.
