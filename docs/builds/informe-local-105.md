# Informe de la Tarea 105 (builder local)

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes (lib) | Fallas | Ignorados |
|---|---|---|---|
| Base (3b48385) | 2672 | 0 | 2 (en tests de integración) |
| Final | 2681 | 0 | 2 |

Los 9 tests nuevos explican la diferencia. Ningún test de tiempo falló en el hook. Ningún commit se reintentó.
Ningún test nuevo necesita podman.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `FUENTES_DE_SECRETOS` repetía literales y no incluía la descarga | `previos_quitar_previos_deja_el_secreto_previo_de_la_descarga` | 6c5a5f0 |
| 2 | El secreto en `ENV` o `ARG` de un Dockerfile salía con la fuente `dockerfile` y no bloqueaba | `previos_secreto_de_env_en_dockerfile_igual_en_bootstrap_es_secreto_previo`, `previos_secreto_previo_de_dockerfile_bloquea_con_la_nota`. Guarda: `previos_dockerfile_corre_como_root_igual_en_bootstrap_no_es_secreto` | 9586059 |
| 3 | Ningún test ataba cada emisor a su bloqueo | Guardas desde el inicio. Rojo de `previos_cada_fuente_de_secretos_tiene_su_test` con una fuente falsa temporal | 6c9cf8b |

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- La prueba de exhaustividad comprueba que cada fuente tiene una fila en `EMISORES_DE_SECRETOS`. No comprueba que el nombre de test de esa fila exista.
- La medición de cuatro variantes con t1 la corre el orquestador al mezclar.
- Los tests no usan podman. Ninguno quedó con `saltar_sin_podman`.

## 4. Emisores de secretos

| Fuente | Constante | Test |
|---|---|---|
| `gitleaks` | `juez::SONDA_GITLEAKS` | `previos_secreto_previo_de_gitleaks_bloquea` |
| `configs-secretos` | `configs::SONDA_SECRETOS` | `previos_secreto_previo_de_configs_bloquea` |
| `configs-secretos` (regla `ENV` o `ARG`) | `configs::SONDA_SECRETOS` | `previos_secreto_previo_de_env_de_dockerfile_bloquea` |
| `descarga` | `descarga::SONDA_DESCARGA` (nueva) | `previos_secreto_previo_de_la_descarga_bloquea` |

`FUENTES_DE_SECRETOS` tiene tres entradas, armadas con las constantes.

## 5. Tests que cambiaron de valor

Ninguno. Solo se amplió el `use` del módulo de tests de `previos.rs`.

## 6. Llamadores revisados

- Ninguna firma cambió y ningún símbolo se borró.
- `SONDA_GITLEAKS` y `SONDA_SECRETOS` pasaron de privadas a `pub(crate)`. Esto no rompe a ningún llamador.
- `FUENTES_DE_SECRETOS` pasó de 2 a 3 elementos. `grep -w` la encuentra solo en `previos.rs`. Su único uso es `es_fuente_de_secretos`.
- `es_fuente_de_secretos` la usan `previos.rs` (`quitar_previos`) e `informe.rs`. Ambos reciben el efecto buscado.
- No corrí codegraph: no hubo cambio de firma. El grep de `SONDA_GITLEAKS`, `SONDA_SECRETOS`, `FUENTES_DE_SECRETOS` y `es_fuente_de_secretos` cubre los usos.
- Además cambié los literales `"gitleaks"` de `previos.rs` (`calcular` y `marcar_herramientas`) por la constante. El comportamiento es igual.

## 7. Preguntas para el planner

1. Los tests de "Dockerfile bloqueado" viven en `previos.rs`, no en `configs.rs` ni en `informe.rs`. La tabla del brief solo permitía `configs.rs` en el punto 2. Decisión: usé `previos.rs`, que el punto 3 permite. ¿Los querés en otro archivo?
2. La regla `ENV` o `ARG` cambió de fuente de `dockerfile` a `configs-secretos`. `VERSION_SONDAS_SEGURIDAD` sigue en 3 porque la clave de la línea base no usa la fuente. Decisión: no la subí. ¿Querés subirla a 4 por política?
3. Una línea base guardada con `ENV` o `ARG` sigue válida. El hallazgo previo se reconoce por id y severidad. Decisión: no hice migración.
