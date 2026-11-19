# Informe de la Tarea 162b

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (6c345ed) | 3977 | 0 | 5 |
| Final (8cad866) | 3982 | 0 | 5 |

Los cinco ignorados son: `target_podman`, dos `builder_real`, `descarga_python_git` e `importar_metrics_reales`.
El hook de commit corrió la suite en los dos commits. Ningún test de tiempo falló, así que no hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 2 | El motivo decía siempre "plazo de 180 s" | `motivo_de_plazo_usa_el_plazo_de_la_consulta`. Guarda: `el_plazo_que_marca_paso_se_lee_como_plazo_en_el_motivo` | cbfff3b |
| 1 | Un corte de la descarga salía con `informe` nulo | `oraculo_corte_lock_invalido_trae_informe_que_nombra_el_hallazgo`, `oraculo_corte_fuente_no_aceptada_trae_informe_que_nombra_el_hallazgo`. Guardas: `oraculo_camino_normal_trae_informe_y_estado_de_su_decision` y las nueve dimensiones dentro de `assert_corte_con_informe` | 8cad866 |

## 3. Qué falló o quedó incompleto

- **El corte "no instala" no tiene test.** Solo ocurre con un contenedor. `preparar_rust` no tiene una costura para inyectar el ejecutor. Esa costura vive en `descarga.rs`, que el brief prohíbe tocar. El código del arreglo cubre los tres cortes, porque comparten el mismo brazo. Falta una prueba directa del caso.
- **El informe no dice que las suites no corrieron.** Dice "descarga sin veredicto: <detalle>" y marca las nueve dimensiones "Sin medir". Decirlo con esas palabras exige cambiar `informe.rs`, que está prohibido.
- **El test del plazo de 50 s llama a `motivo_de_error` directo.** Con un restante menor que 180 s, el plazo total de `esperar_trabajo` corta antes. El motivo de una consulta no llega a la salida por esa vía.
- `as_secs` trunca. Si el restante cruza un segundo entre el cálculo y `paso`, el motivo puede decir un segundo menos.
- Pendiente del orquestador: la suite con podman y la nueva verificación de b8d0db4058 con su lock real.

## 4. La decisión de cada corte

| Corte | Decisión de `decidir` | Estado | Qué dice el informe |
|---|---|---|---|
| Lock inválido | `SinDecision` (medida) | `SinVeredicto` | Nombra el hallazgo en el resumen y en los riesgos (Entrega, severidad 3). Nueve dimensiones "Sin medir". |
| Fuente no aceptada | `SinDecision` (medida) | `SinVeredicto` | Igual que el anterior. |
| No instala | `SinDecision` por lectura de `decidir`. Sin medir. | — | Sin medir. |

Ninguna decisión difiere de lo esperado. No se cambió ninguna regla.
El test viejo `oraculo_descarga_sin_veredicto_deja_las_nueve_sin_veredicto` usa `LOCK_ROTO` con `Cargo.toml`. Ese caso da "fuente no aceptada", no "lock inválido".

## 5. Tests que cambiaron de valor

Ninguno. Ningún test de otro archivo se editó.

## 6. Llamadores revisados

- `motivo_de_error` cambió de firma. Recibe el plazo. `grep -w` halló un solo llamador de producción, `esperar_trabajo`, y los tests nuevos. `src/corre/corrida.rs` tiene otra función con el mismo nombre, sin relación.
- `evaluar_con` no cambió de firma.
- `codegraph` no se usó. `grep -w` cubre estos dos nombres.

## 7. Preguntas para el planner

1. **¿Se agrega una costura en `descarga.rs` para probar el corte "no instala" sin contenedor?** Decisión del organizador: no se tocó el archivo. El caso queda sin test directo.
2. **¿Debe el informe decir "las suites no corrieron"?** Decisión del organizador: no se tocó `informe.rs`. Se anota el hueco.
3. **P121 sigue abierta.** Los tres cortes dan `SinDecision` con informe.
