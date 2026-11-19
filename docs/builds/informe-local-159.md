# Informe de la Tarea 159

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (6ed2525): 3334 verdes en el crate principal, 1 ignorado, 0 fallas. Todos los binarios suman 3937 verdes y 5 ignorados.
- Final: 3347 verdes en el crate principal, y 3950 verdes en total, 0 fallas, 5 ignorados.
- Los 13 tests nuevos explican la diferencia: 5 del punto 1, 4 del punto 2 y 4 del punto 3.
- Ningún commit falló en el hook. No hubo reintentos por tests de tiempo.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El reintento armaba ids de pytest también en Rust | `inestable_reintento_rust_arma_filtro_nextest`, `inestable_reintento_rust_sin_reintento_da_el_comando_entero`, `inestable_reintento_rust_binario_se_separa_en_primer_doble_dospuntos`, `inestable_reintento_lenguaje_sin_regla_es_error` | 81677b2 |
| 2 | Un lock que no cuadra salía como infraestructura | `descarga_lock_no_cuadra_con_cargo_toml_es_producto`, `descarga_hallazgo_lock_no_cuadra_va_sobre_cargo_lock` | 33211cb |
| 3 | `id-token: write` salía como secreto | `configs_secretos_id_token_write_en_permissions_no_cuenta`, `configs_secretos_valores_de_permiso_no_cuentan_sin_importar_forma`, `configs_secretos_toml_token_permiso_no_cuenta_y_token_real_si` | a6d41c1 |

Guardas verdes desde el inicio:
- Punto 1: `inestable_reintento_python_mismos_ids_con_y_sin_plantilla` y los 4 tests `inestable_reintento_*` previos.
- Punto 2: `descarga_paquete_inexistente_y_version_siguen_sobre_cargo_toml` y `descarga_red_gana_a_lock_no_cuadra`.
- Punto 3: `id-token` con `ghp_...` sigue en severidad 3, y `token = "ghp_..."` en TOML también.

## 3. Qué falló o quedó incompleto

- No subí la versión de las sondas. Ver la sección 5.
- La suite con podman y la verificación de las 4 tareas de watchexec quedan para el orquestador.
- No corrí la validez, `corre`, `calibrar` ni la pasada reales.

## 4. El id de un test de nextest

- `junit::id_de` arma la clave `<classname>::<nombre>`. En nextest, el `classname` del `testcase` es el id del binario, y el `name` es el nombre del test.
- `junit::id_de_clave` convierte la clave en id. Un `classname` sin puntos queda como un solo segmento. El id final es `<binario>::<nombre>`.
- `termino_nextest`, en `src/oracle/mod.rs`, separa en el primer `::`. Lo anterior es el binario. Lo demás es el nombre. Sin `::`, el id es solo el nombre.
- Límite: un binario con `::` en su id, como `crate::bin/x`, se separa mal. Ver la pregunta 1.

## 5. La versión de las sondas

- Existe: `VERSION_SONDAS_SEGURIDAD` en `src/oracle/previos.rs`. Vale 3.
- No cambió. Vive fuera de mi lista de archivos, así que el brief manda detenerme y anotarlo.
- El punto 3 cambia la sonda `configs-secretos`. Ver la pregunta 2.

## 6. Tests que cambiaron de valor

- Cuatro tests `inestable_reintento_*` de `src/oracle/mod.rs` reciben el nuevo argumento `"python"`. Su resultado esperado no cambió.
- Ningún test de otro archivo cambió.

## 7. Llamadores revisados

- `reintento_de`: codegraph da `evaluar_suite` y los tests `inestable_reintento_*`. `grep -w` da lo mismo, solo en `src/oracle/mod.rs`. El llamador de producción pasa `&c.tarea.lenguaje`.
- `hallazgo_no_instala`: la firma no cambió. Solo cambia su conducta para el texto del lock. Los llamadores son `preparar_rust` en dos sitios.
- `es_literal_secreto`: la firma no cambió. Sus llamadores son `clave_con_secreto` y la regla de Dockerfile para `ENV` y `ARG`.

## 8. Preguntas para el planner

1. **¿Cómo separar un binario de nextest con `::` en su id?** Decisión: separar en el primer `::`. Un id como `crate::bin/x::test` daría un filtro que no elige nada. La corrida extra no vería ese test.
2. **¿Subir `VERSION_SONDAS_SEGURIDAD` de 3 a 4?** Decisión: no la toqué, porque vive en `src/oracle/previos.rs`. El cambio altera la línea base de `configs`. Falta subir la constante antes de congelar.
3. **El punto 3 también afecta la regla de Dockerfile.** `ENV TOKEN=read` ya no da hallazgo, porque comparte `es_literal_secreto`. Decisión: lo dejé, porque el brief dice "con cualquier clave".
