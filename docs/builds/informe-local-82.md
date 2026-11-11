# Informe de la Tarea 82

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (f0aacbc) | 2 841 | 0 | 4 |
| Final (945fd79) | 2 859 | 0 | 5 |

El ignorado nuevo es `calibra_cota_de_t1_de_s2_sin_api`. Ningún test de tiempo falló en los commits.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La huella de Seguridad dependía del prompt del juez. | `calibra_huella_de_seguridad_no_depende_del_prompt`. Guardas: `calibra_huella_de_diseno_depende_del_prompt`, `calibra_huella_de_diseno_no_cambia_de_valor` y los de la Tarea 81. | 1906118 |
| 2 | Una ventana larga se cortaba al final y perdía lo cambiado. | `juez_veinte_lineas_cambiadas_entran_enteras_si_el_contexto_baja`, `juez_cambio_que_solo_pasa_el_maximo_se_parte_en_ventanas`. Guardas: los tests de ventanas de la Tarea 81. | 1089523 |
| 3 | El veredicto no mostraba por qué un puntaje no calibra. | `oraculo_motivo_sin_calibrar_de_fila_con_huella_de_otro_juez`, `informe_muestra_el_motivo_junto_a_la_dimension_sin_calibrar`. Guardas: calibrado trae `None`, `tipos_puntaje_viejo_sin_motivo_sin_calibrar_se_lee_none`, `tipos_ida_y_vuelta_de_puntaje_con_motivo_sin_calibrar`. | bd074b2 |
| 4 | La cota contaba un pedido y 256 KiB fijos por llamada. | `cota_de_spec_enorme_es_el_maximo`, `cota_crece_con_la_spec`, `cota_cubre_el_prompt_real_del_reintento`, `calibra_peor_caso_de_juez_de_diseno_cuenta_sus_dos_pedidos`, `calibra_peor_caso_con_entrada_chica_baja_y_crece_hasta_el_maximo`. Guardas: modelo sin precio da cota infinita y error de configuración. | 945fd79 |

## 3. Qué falló o quedó incompleto

- **La cota nueva no llega al CLI.** Existe `peor_caso_de_home_con_tarea`, pero `src/lib.rs:185` sigue llamando `peor_caso_de_home`. Esa función usa 256 KiB. El CLI cuenta 2 pedidos para diseño, así que su cota sube hasta que se conecte la función nueva. Falta una edición de `src/lib.rs`, fuera de mi lista.
- **La cota de entrada baja poco.** Para t1 de S2 baja de 262 144 a 247 124 bytes. La mayor parte son los topes de `LimitesPrompt` por 4 bytes por carácter.
- **Las reservas de bytes son supuestos sin medir.** Son 200 caracteres por hallazgo, 4 KiB de encabezados y 8 KiB de reintento. El detalle de un hallazgo y las citas del reintento no tienen tope en el código. El envío real sí tiene tope: `MAX_ENTRADA_JUEZ_BYTES`.
- La cota de t1 usa requisitos vacíos: no leí `tarea.toml`, como pide el brief.
- Punto 1: las filas guardadas de Seguridad tienen la huella vieja. Dejan de calibrar una vez. Hace falta recalibrar Seguridad. No miré `ROOT_HOME`.
- No corrí `codegraph`: el worktree no tiene índice y `init` crearía una carpeta. Usé `grep -w`.
- Quedan tres worktrees temporales, `~/.cache/root-wt/local82-p1` a `-p3`, con sus ramas `laptop/t82-p1` a `-p3`. Su trabajo ya está en `laptop/tarea-82`.

## 4. Tests que cambiaron de valor

- `calibra_peor_caso_suma_el_juez_de_diseno_y_el_de_config_por_todos_los_intentos`. Antes esperaba `2 × INTENTOS_MAX × por_intento`. Ahora espera `(2 + 1) × INTENTOS_MAX × por_intento`. El juez de diseño hace 2 pedidos, por el reintento. El juez de config hace 1: `juzgar_config` no reintenta.

## 5. Cota del peor caso para t1 de S2

Salida de `tools/test-seguro.sh --lib calibra_cota_de_t1 -- --ignored --nocapture`. La spec mide 3 441 bytes. Es el juez de diseño.

| | Bytes de entrada por intento | Tokens de salida por intento | Intentos por caso |
|---|---|---|---|
| Vieja | 262 144 | 32 000 | 3 |
| Nueva | 247 124 | 32 000 | 6 |

El orquestador pasa estos números a USD con `root.toml`. Si el caso usa el juez de config, suma 3 intentos más con 262 144 bytes, en ambas cotas.

## 6. Llamadores revisados

No cambió ninguna firma pública existente. Hay tres firmas nuevas: `cota_bytes_de_prompt`, `peor_caso_de_caso_con_entrada` y `peor_caso_de_home_con_tarea`.

- `ventanas` (privada, firma cambiada en el punto 2): `grep -w` da un solo llamador, `extractos_de_cambios`. Codegraph no corrió.
- `peor_caso_de_caso`: `calibra.rs` y `tests/calibra.rs`. Mantiene su firma.
- `peor_caso_de_home`: `src/lib.rs:185` y `tests/calibra.rs`. Mantiene su firma.
- `Puntaje` suma un campo. Los literales de 10 archivos llevan `motivo_sin_calibrar: None`.

## 7. Preguntas para el planner

1. ¿Se conecta `peor_caso_de_home_con_tarea` en `src/lib.rs`? Decisión: no lo toqué. La cota del CLI sube hasta que se conecte.
2. ¿Qué supuestos de bytes valen para hallazgos y reintento? Decisión: 200 caracteres, 4 KiB y 8 KiB. Con 600, 16 KiB y 16 KiB la cota daba el máximo y el punto no servía.
3. ¿Se bajan los topes de `LimitesPrompt` para que la cota baje más? Decisión: no. Eso cambia el prompt y la huella.
4. El punto 3 suma `Confianza.motivos_sin_calibrar` y `Ord` en `Dimension`. Decisión: lo acepté, porque `markdown()` solo recibe el `Informe` y necesita el dato. Por eso `docs/interfaces.md` suma 132 líneas, no solo el campo de `Puntaje`. ¿Se confirma?
5. El punto 3 editó una línea en cada literal de `Puntaje` de archivos fuera de la lista (`compartidos.rs`, `regla.rs`, `perfiles/mod.rs`, `requisitos.rs`, `validez.rs`). Decisión: cambio mecánico, sin otro cambio.
6. Un puntaje sin veredicto con huella vieja recibe el motivo, pero el informe no lo muestra. ¿Debe mostrarse?
7. Punto 2: una línea sola más larga que `extracto_max` se omite en silencio. Si se agota `extractos_total`, la última ventana aún corta al final. Dos particiones contiguas muestran la marca de recorte sin perder líneas. Decisión: dejé todo así, como pide la regla 3.
8. Punto 1: ¿se recalibra Seguridad una vez, o se acepta el corte?
