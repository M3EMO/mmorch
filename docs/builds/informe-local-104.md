# Informe de la Tarea 104

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (22d0732) | 2664 (por resta, no medido) | 0 | 0 |
| Final | 2669 | 0 | 0 |

Los tests de integración y de otros binarios también dan verde. Dos ignorados vienen de pruebas de documentación y ya existían.
El hook de commit no falló por tiempo. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 2 | El comentario de `EXCLUIR_DELETE_FIELD` decía que `--exclude-re` excluye esos mutantes. No los excluye. | Ninguno: no cambia comportamiento. La guarda es la suite verde. | 3a1751d |
| 1 | La línea base no corría las sondas de Seguridad de configs. Un hallazgo previo de configs bloqueaba. | `previos_secreto_de_configs_igual_en_bootstrap_queda_previo`, `previos_quitar_previos_deja_el_secreto_previo_de_configs`, `previos_linea_base_con_la_version_vieja_no_se_reusa`, `previos_sonda_de_configs_sin_veredicto_no_guarda_la_base` | 1e960a9 |

Guarda del punto 1: `previos_secreto_de_configs_solo_en_el_repo_queda_nuevo` y los tests de la 103 y la 103b.

## 3. Qué falló o quedó incompleto

- **El bloqueo de un secreto de configs previo no está hecho.** `es_secreto` en `src/oracle/informe.rs:414` solo reconoce `gitleaks`. Un secreto previo de `configs-secretos` sigue en el puntaje, pero no bloquea. `informe.rs` está fuera de la tabla. Falta ampliar `es_secreto` a `configs-secretos`, con su test.
- **La fuente de descarga no entra en la línea base.** Su id sale de funciones privadas de `src/oracle/descarga.rs`, fuera de la tabla.
- **Hadolint no entra.** Es de dimensión Entrega y no produce hallazgos de Seguridad.
- Un test de hallazgo de severidad 3 de configs (no secreto) igual en `bootstrap/` y en el repo se cubre con la misma ruta de código que el secreto. No hay un test aparte.

## 4. Fuentes de la línea base

Corre ahora: cargo audit, gitleaks, sondas nativas de Seguridad y las sondas locales de configs de Seguridad (estructura, secretos, reglas de Dockerfile).
La clave suma el hash de `bootstrap/`, `VERSION_SONDAS_SEGURIDAD` (ahora 3), imagen, lenguaje y la huella de RustSec.
`LineaBase` tiene el campo nuevo `configs`, con `#[serde(default)]`. Una sonda de configs sin veredicto deja la base sin guardar.

## 5. Tests que cambiaron de valor

Ninguno.

## 6. Llamadores revisados

- `clave_de`: la firma no cambia. Delega en `clave_de_version`, nueva y privada. Grep: solo `previos.rs`. Codegraph no tiene índice en este worktree; no lo inicié.
- `quitar_previos`: cambia el comportamiento, no la firma. Su único llamador es `src/oracle/mod.rs` (Seguridad).
- `marcar_configs`: nueva. Su único llamador es `src/oracle/mod.rs:1136`.

## 7. Preguntas para el planner

1. ¿Se amplía `es_secreto` en `informe.rs` a `configs-secretos`? Decisión: no lo toqué, está fuera de la tabla. Sin eso, un secreto de configs previo no bloquea. Es el punto más importante.
2. ¿Se incluye la fuente de descarga en la línea base? Decisión: no. Pide exponer la construcción de hallazgos de `descarga.rs`.
3. Los hallazgos de Seguridad de reglas de Dockerfile (por ejemplo, correr como root) ahora pueden quedar previos. Decisión: sí, sigue la regla de P96.
4. `calcular` usa `sondas_configs_locales` y no corre hadolint sobre `bootstrap/`. Decisión: el organizador cambió esto frente a la orden del subagente, porque hadolint es de Entrega y costaba un contenedor extra.
