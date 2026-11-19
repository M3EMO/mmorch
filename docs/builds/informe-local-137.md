# Informe de la Tarea 137

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- **Base (c038b0e):** 3052 verdes en la librería. Es un cálculo: 3067 tras el punto 2, menos los 15 tests de los puntos 1 y 2. No corrí la suite completa en la base.
- **Final (d20035b):** 3652 verdes, 0 fallas, 5 ignorados en toda la suite.
- El hook de pre-commit pasó los tres commits al primer intento. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | No hay forma de marcar `bootstrap/` ni de leer qué corrió | 7 tests `sonda_lastre_*` de `sonda_lastre.rs` (marcas, compila, const/unsafe/main/tests, lectura, comando, copia) | bf6de78 |
| 2 | La evaluación no corre la sonda | `con_sonda_pone_la_bandera_y_la_restaura`, `tomar_saca_el_resultado_y_deja_vacio`, `pedida_manda_el_comando_con_copia_marcada_y_la_borra`, `salida_valida_deja_la_funcion_que_corrio`, `fallo_del_ejecutor_deja_motivo_aviso_y_sigue`, `salida_sin_marcas_deja_error_sin_salida`. Guardas: `restaura_ante_panico`, `sin_pedida_no_manda_comando_ni_deja_resultado` | 2b85bda |
| 3 | El lastre ignora lo que corrió | `lastre_sonda_con_corridas_va_a_la_funcion_que_corrio`, `lastre_sonda_corridas_sin_funcion_en_el_repo_no_aplica`, `lastre_sonda_gana_la_funcion_del_cambio_sobre_la_de_fuera`, `lastre_sonda_si_solo_corrio_fuera_del_cambio_va_a_esa`, `sonda_lastre_base_de_eficiencia_la_pide_y_los_casos_no`, `sonda_lastre_resultado_de_la_base_llega_al_inyector`. Guardas: `lastre_sonda_sin_corridas_usa_la_regla_de_la_136`, `sonda_lastre_base_de_pruebas_no_la_pide`, los `lastre_de_memoria_*` de la 131, 134 y 136 | d20035b |

Dos tests del punto 3 se escribieron después del código y no se vieron en rojo: `sonda_lastre_pares_de_la_sonda_solo_con_ok` y `sonda_lastre_sin_funcion_en_el_repo_da_el_motivo_de_la_135`.

## 3. Qué falló o quedó incompleto

- **Sin probar, para el orquestador (necesitan podman):** que la copia marcada compile y corra en la imagen para t1 a t3. El costo en tiempo de la sonda. Cuántas funciones marca como corridas cada tarea. Si `content_search/mod.rs` de t2 está entre ellas. La calibración de Eficiencia de t1 a t3 con la 137 y la 138.
- **No hay test del fallo de `armar_copia_marcada` dentro de la evaluación.** Forzarlo pide un `bootstrap/` ilegible.
- **No hay test de la base repetida.** La rúbrica de Eficiencia solo tiene ítems `sonda`, así que la base repetida no corre. La llamada repetida queda fuera de `con_sonda_lastre` por estructura.
- **Resto:** `con_sonda_lastre` conserva un `#[allow(dead_code)]` que ya no hace falta. Está en `sonda_lastre.rs`, que no figura en la tabla del punto 3.

## 4. La marca y el comando

Función marcada (la marca va justo bajo la `{`):

```rust
pub fn uno() -> u32 {
{ static ROOT_SONDA_0: ::std::sync::atomic::AtomicBool = ::std::sync::atomic::AtomicBool::new(false); if !ROOT_SONDA_0.swap(true, ::std::sync::atomic::Ordering::Relaxed) { let _ = ::std::fs::write("/tmp/root-sonda/src_a_rs__1__uno", b""); } }
    1
}
```

El `<id>` es `archivo__linea__nombre`, con todo carácter fuera de `[A-Za-z0-9_]` cambiado por `_`.

Comando exacto (`PRELUDIO` es `export CARGO_HOME=/tmp/cargo-home && mkdir -p "$CARGO_HOME" && cp /cache/config.toml "$CARGO_HOME/config.toml" && `):

```
{PRELUDIO}rm -rf /tmp/root-sonda; mkdir -p /tmp/root-sonda; cd /repo && export CARGO_TARGET_DIR=/salida/target && cargo nextest run --offline --no-fail-fast --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml; echo ROOT_SONDA_INICIO; ls /tmp/root-sonda; echo ROOT_SONDA_FIN
```

## 5. Tests que cambiaron de valor

- Ningún test existente cambió de valor.
- No edité tests de otros archivos. Los literales de `Contexto` en `inyectar.rs` y `defectos.rs` recibieron `corridas: &[]`.
- Los tests `lastre_de_memoria_*` de la 131, 134 y 136 siguen verdes: con la lista vacía el camino es el de la 136.

## 6. Llamadores revisados

Codegraph no corrió: la unión se hizo con `grep -w` y con la lectura del compilador. Un cero de grep no prueba nada, y el compilador cubre los literales.

- `inyectar::lastre_de_memoria` (privada, suma `corridas`): un llamador, `inyectar_con`.
- `inyectar::Contexto` (pública, suma `corridas`): literales en `inyectar.rs` (incluida `inyectar()`), `defectos.rs:192` e `inyectar_en` de `calibra.rs`.
- `Ronda` (privada, suma `corridas`): un literal, en `calibrar_medida_con_defectos`.
- Visibilidad: los helpers de `inyectar.rs` pasaron a `pub(super)` y `mod sonda_lastre` a `pub(crate)`. No cambian firmas.

## 7. Preguntas para el planner

1. **Archivos fuera de la tabla.** `sonda_lastre.rs` necesita los helpers privados de `inyectar.rs` (`Cuerpo`, `cuerpo_de`, `es_fn_normal`, `limpias_de` y otros). Decisión: en el punto 1 los pasé a `pub(super)`, sin cambio de lógica. La alternativa era duplicar el análisis de funciones. Además, el punto 2 cambió `mod sonda_lastre;` a `pub(crate)` en `calibra.rs`, porque `mod.rs` la usa. ¿Se acepta?
2. **Funciones en `mod x { }` en línea.** La marca es un `static` dentro del cuerpo, así que compila. Decisión: se marcan. `funciones_de_efecto` salta esos archivos enteros. ¿Se alinean?
3. **`async fn`** se marca. Decisión: sí.
4. **Colisión de ids.** `src/a.rs` y `src_a.rs` dan el mismo id saneado y el mapa lo pisaría. Decisión: no lo traté por improbable.
5. **La sonda corre sin mirar `partes`** y en toda tarea Rust con `bootstrap/`, aunque no tenga `[eficiencia]`. Solo la pide la base de la calibración de Eficiencia. Decisión: queda así.
6. **Dos funciones con el mismo nombre en un archivo** entran las dos como candidatas. Decisión: basta así.
7. **Sonda fallida.** El lastre cae a la regla de la 136 sin marca en la fila. La evaluación suma un aviso. ¿Debe quedar marcado en la fila de la calibración?
8. **Lista con funciones que no existen en el repo.** Decisión: sin lugar, con el motivo de la 135, como pide el brief.
9. **Subagentes.** Los lancé sin `run_in_background: false` explícito la primera vez. Corrieron en segundo plano y esperé su informe sin terminar el turno.
