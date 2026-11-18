# Informe de la Tarea 162

## 1. Resultado de la suite

- Base (5efb613): 3971 verdes, 0 fallas, 5 ignorados. El dato sale del informe de la 161.
- Final: 3977 verdes, 0 fallas, 5 ignorados.
- Todo corrió con `ROOT_SIN_PODMAN=1`. Los tests con `saltar_sin_podman` salen verdes sin correr.
- El hook del commit no falló por tiempo. No hubo reintento.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Un corte de la descarga sale sin informe | Ninguno: el punto se detuvo (sección 3) | sin commit |
| 2 | "Colab no responde" no dice el motivo | `cinco_plazos_dan_el_motivo_plazo`, `cinco_codigos_1_dan_el_motivo_codigo_1`, `cinco_sin_marcador_dan_el_motivo_sin_marcador_sin_el_stdout_entero` | 5534d9d |

Guardas del punto 2, verdes a la primera:
- `guiones_del_trabajo_lanzan_consultan_y_dan_fin_o_murio` corre los tres guiones con `python3 -I`. No halló ningún bug.
- `dormir_con_senal_vuelve_al_llegar_sigterm`.
- `senal_durante_la_espera_corta_el_trabajo_y_libera_el_runtime`.

El mensaje nuevo termina en "; último motivo: plazo de 180 s", "código N" o "sin marcador". Un error de la CLI usa su primera línea, con 80 caracteres como máximo.

## 3. Qué falló o quedó incompleto

**Punto 1: sin hacer.** El brief manda detenerse si `bloqueos` no cuenta el hallazgo.
- Función: `bloqueos`, en `src/oracle/informe.rs` (línea 440).
- Motivo: el hallazgo bloquea solo si su dimensión es Seguridad y su severidad es 3. Los hallazgos de la descarga son de Entrega (`hallazgo` en `descarga.rs` usa `Dimension::Entrega`). `bloqueos` no los cuenta.
- Efecto: con el informe armado, `decidir` daría `SinDecision`, no `Bloqueado`. Las nueve dimensiones sin veredicto disparan `sin_decision`. El estado seguiría sin veredicto.
- No agregué ninguna regla. Armar el informe sin la regla da informe visible, pero la decisión sigue en `SinDecision`. Esa parte queda para el planner.

**Sin probar en esta tarea:** la verificación de b8d0db4058 con su lock real y la suite con podman. Las corre el orquestador.

## 4. La decisión de un corte

No se midió con código, porque el punto 1 se detuvo. Por lectura de `decidir`:
- "No instala", "lock inválido" y "fuente no aceptada" dan hallazgos de Entrega con severidad 3.
- `bloqueos` no los cuenta (solo Seguridad, requisitos de peso bloqueante e ítems de rúbrica con `bloquea`).
- `no_listo` no dispara: no hay tests fallidos ni `no_compila`.
- `sin_decision` dispara por las nueve dimensiones sin veredicto. Resultado: `SinDecision`, no `Bloqueado`.

## 5. Tests que cambiaron de valor

- Ninguno cambió de valor.
- No edité tests de otros archivos.
- `cinco_consultas_sin_respuesta_dan_colab_no_responde` conserva sus asserts.

## 6. Llamadores revisados

- Ninguna firma pública cambió. `motivo_de_error` y `primera_linea_corta` son funciones privadas nuevas, con un solo llamador: `esperar_trabajo`.
- `esperar_trabajo` no cambió de firma. Su error `Infra` suma un trozo al final del mensaje.
- No corrí `codegraph`. El cambio no toca firmas existentes.

## 7. Preguntas para el planner

1. **¿Cómo bloquea un hallazgo de Entrega con severidad 3?** `bloqueos` solo cuenta Seguridad. Decisión mía: no agregué regla y detuve el punto 1. Opciones: (a) `bloqueos` cuenta Entrega con severidad 3 de la fuente `descarga`; (b) el corte de la descarga emite su hallazgo en Seguridad; (c) el informe del corte queda en `SinDecision`.
2. **¿Armo el informe del corte aunque la decisión sea `SinDecision`?** Decisión mía: no, porque el brief liga el informe a `Bloqueado`.
3. **El plazo del motivo dice siempre 180 s.** Usa `PLAZO_LOCAL_TRABAJO`, aunque `lim.restante()` pueda acortar el plazo real. Decisión mía: aceptarlo, como pedía el brief.
4. **`motivo_de_error` detecta el plazo por el texto "colab superó".** Lo pone `colab::paso`. Si ese texto cambia, el motivo cae a la primera línea del error. Decisión mía: aceptarlo; un error tipado exige tocar `oracle/colab.rs`, que el brief prohíbe.
