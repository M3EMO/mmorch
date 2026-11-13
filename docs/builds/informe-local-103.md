# Informe de la Tarea 103

La Tarea 103 hace que solo bloquee lo que el cambio introduce (decisión P96, opción a).
Base: a301c80. Rama: `laptop/tarea-103`. Todo corrió con `ROOT_SIN_PODMAN=1`.

## 1. Resultado de la suite

| Momento | Tests de la librería | Fallas | Ignorados |
|---|---|---|---|
| Base (a301c80 más R1, un test) | 2631 | 0 | 0 |
| Final (10d4892) | 2657 | 0 | 0 |

- La base pura tiene 2630 tests de librería. La Tarea 103 suma 27.
- Los demás binarios de test dan verde. Hay 4 ignorados en ellos, como antes.
- El hook pre-commit pasó a la primera en todos los commits. No hubo reintento por tests de tiempo.
- No corrí podman. Ningún test nuevo necesita `saltar_sin_podman`.

## 2. Commits

| Grupo | Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|---|
| A | A1 | No hay línea base de Seguridad ni campo `previo` | `previos_mismo_aviso_de_cargo_audit_en_la_base_queda_previo`, `previos_crate_vulnerable_que_no_estaba_en_la_base_queda_nuevo`, `previos_nativo_con_id_de_la_base_en_linea_del_cambio_queda_nuevo`, `previos_id_dos_veces_en_el_repo_y_una_en_la_base_queda_nuevo_el_del_cambio`, `previos_linea_base_guardada_no_vuelve_a_correr_sondas` | ff3193e |
| A | A2 | El veredicto no marca previo ni avisa sin `bootstrap/`; el puntaje cuenta lo previo (B2) | `oraculo_aviso_de_cargo_audit_del_estado_inicial_sale_previo`, `oraculo_sin_bootstrap_da_el_aviso_y_ningun_hallazgo_es_previo`, `oraculo_aviso_previo_no_baja_el_puntaje_de_seguridad`, `oraculo_secreto_previo_de_gitleaks_si_baja_el_puntaje_de_seguridad`, `previos_quitar_previos_saca_el_aviso_previo_y_recalcula_el_valor` | e8c3430 |
| B | B1 | `bloqueos` bloquea por todo hallazgo de Seguridad con severidad 3 | `informe_siete_avisos_previos_de_cargo_audit_no_bloquean`, `informe_aviso_previo_mas_uno_nuevo_bloquea_y_cita_el_nuevo`, `informe_secreto_previo_bloquea_y_la_frase_lo_dice` | bc35386 |
| B | B3 | El informe no lista los avisos previos ni los cuenta | `informe_avisos_previos_llena_la_lista_el_markdown_y_el_resumen`, `informe_resumen_con_siete_avisos_previos_va_en_plural` | 10d4892 |
| R | R1 | El aviso de mutantes de más dice "1 mutantes" | `mutacion_aviso_de_mas_usa_singular_con_uno` | f704295 |
| R | R2 | `calibrar` no imprime `Calibrada.avisos` | `lineas_de_avisos_escribe_una_linea_por_aviso_en_orden` | fc27d53 |

Guardas verdes desde el inicio:
- `previos_otra_version_del_mismo_crate_queda_nuevo`
- `tipos_hallazgo_viejo_sin_previo_lee_false`
- `informe_veredicto_viejo_sin_previo_lee_false_y_sigue_bloqueado`
- `informe_sin_previos_no_tiene_seccion_ni_linea`
- `informe_secreto_previo_no_aparece_en_avisos_previos`
- `informe_viejo_sin_avisos_previos_lee_vacio`
- `lineas_de_avisos_sin_avisos_es_vacio`

B2 (puntaje) vive en `src/oracle/mod.rs`, no en `informe.rs`. Lo hace `previos::quitar_previos` en el commit A2.
R1 y R2 se construyeron en worktrees aparte y entraron con `git cherry-pick`.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- R2 prueba el helper `lineas_de_avisos`. La conexión con `oraculo_calibrar` se verificó leyendo el diff, porque esa función usa la evaluación real.
- No corrí la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.
- Ninguna prueba corrió con podman real. La línea base con contenedor real queda sin probar.

## 4. Dónde vive la línea base

- Ruta: `$ROOT_HOME/previos/<tarea>/<clave>.json`.
- Clave: sha256 de cuatro partes separadas por un byte cero. Son el hash de `bootstrap/` (`historial::hash_de_version`), `VERSION_SONDAS_SEGURIDAD`, la imagen de la tarea y si hay base de RustSec.
- Se recalcula cuando cambia `bootstrap/`, sube `VERSION_SONDAS_SEGURIDAD` (hoy 1), cambia la imagen o cambia la presencia de RustSec.
- No se guarda si una sonda dio sin veredicto, por ejemplo sin RustSec. Entonces se recalcula en cada evaluación.
- Las líneas base viejas no se borran. Queda un archivo por clave.

## 5. Tests que cambiaron de valor

- Ninguno cambió de valor esperado.
- Se agregó `previo: false` y `avisos_previos: Vec::new()` a los literales de `Hallazgo` e `Informe` en varios archivos, solo para compilar.
- `docs/interfaces.md` se regeneró dos veces. Suma `previo` en `Hallazgo`, y `avisos_previos` con `AvisoPrevio` en `Informe`.

## 6. Llamadores revisados

- `oracle::medir` suma el parámetro `base: Option<&previos::LineaBase>`. Su único llamador es `evaluar_con`, en `src/oracle/mod.rs`. `grep -w` lo confirma. Las llamadas a `medir` de `auditar.rs` son otra función.
- `Hallazgo` e `Informe` suman campos. `grep "Hallazgo {"` y `grep "Informe {"` dieron cada literal, y todos se actualizaron.
- `aviso_de_mas` cambia solo su texto. Sus llamadores son `mod.rs:1087` y sus tests.
- Codegraph: no inicializé `.codegraph/` en este worktree. Los llamadores salen solo de grep, y no hace falta más porque ningún símbolo se borró.

## 7. Preguntas para el planner

1. **Secreto previo.** La frase del bloqueo dice "<id> es un hallazgo de seguridad con severidad 3; estaba en el estado inicial; los secretos bloquean siempre." Decisión: la `regla` queda igual, `bloqueo_seguridad`.
2. **Línea del resumen.** "N avisos previos de Seguridad, no bloquean." se agrega después de la redacción del LLM. El gate de redacción exige un id por oración, y esa línea no cita ninguno. ¿Querés que cite un id? Eso cambia el gate.
3. **Esquema de `Informe`.** `markdown` solo ve el `Informe`. Se agregó `avisos_previos: Vec<AvisoPrevio>` a `Informe` en `src/tipos.rs`. Ese archivo está en la tabla del grupo A, no del B. Decisión: el cambio solo agrega y lleva `#[serde(default)]`.
4. **Aviso de cargo audit.** Es el primer token que empieza con "RUSTSEC-". Sin RUSTSEC, es la primera frase del detalle.
5. **Fuente de un juez.** Se nombra "juez <modelo>". El brief no lo fijaba.
6. **Sección en el markdown.** "Avisos previos" va entre "Riesgos" y "Confianza".
7. **Sondas de configs.** Las sondas de configs, hadolint y la descarga nunca son previo, porque el brief no las incluye en la línea base. Un secreto en un YAML sigue bloqueando.
8. **Línea base incompleta.** No se guarda si una sonda dio sin veredicto. Así un fallo de infraestructura no queda fijo. El costo: sin RustSec se recalcula cada vez.
9. **Atribución.** Los commits de A y B llevan "Claude Haiku 5.5", porque Haiku los construyó. Los de R1 y R2 llevan "Claude Sonnet 5.5", como pedía el prompt del subagente. ¿Querés uniformarlos?
10. **Línea de `cambios`.** `rangos_cambiados` corre siempre que existe `bootstrap/`, aunque no haya mutación ni sondas nativas. El costo es menor.

## Tarea 103b

La Tarea 103b arregla lo que halló la revisión de seguridad de la 103. Cada punto va en un commit.

| Punto | Commit | Qué cambia |
|---|---|---|
| 1 (HIGH) | 3d1c2fb | La línea base compara por id, severidad y, en cargo audit, el id del aviso. |
| 6 | 62f1a12 | Los dos tests de Colab miran solo los avisos de Colab. |
| 2 | c33b1b1 | La clave de la línea base incluye una huella del contenido de RustSec. |
| 3 | 8a27ea9 | Un archivo ilegible o no UTF-8 cuenta como cambiado entero en la marca de previo. |
| 4 | 680b2b2 | La clave de la línea base incluye el lenguaje de la tarea. |
| 5 | 9974545 | La línea base se escribe con archivo temporal y `rename`. |

Notas:

- El punto 5 no tiene test en rojo. Desde afuera, `fs::write` y temporal más `rename` dan el mismo resultado. El test queda como regresión.
- Los puntos 1 y 6 salieron del índice que dejó la corrida cortada. Los revisé y los usé.
- El punto 3 cambia también `src/oracle/mod.rs`: la marca de las nativas usa `previos::cambios_para_marca`. `cambios.rs` no cambia.
- El punto 2 suma `huella_rustsec` al trait `EjecutorHerramienta`, a `EjecutorContenedor` y a `memo.rs`.

Suite: `tools/test-seguro.sh` con `ROOT_SIN_PODMAN=1`. Son 39 binarios, 3238 tests pasan, 0 fallan y 4 se ignoran.
Los dos tests de Colab del punto 6 corrieron con podman y pasan.
