# Informe de la Tarea 122

## 1. Resultado de la suite

La suite corre con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (ea04410) | 3471 | 0 | 5 |
| Final | 3478 | 0 | 5 |

Siete tests nuevos explican la diferencia. Ningún hook de commit falló, así que no hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `podman_falso` de `corrida.rs` no probaba el script | `podman_falso_no_da_etxtbsy_con_fork_concurrente`, `sonda_de_podman_falso_no_deja_marca`, `podman_falso_vacio_es_sintaxis_valida_y_calla_ps` | 929a587 |
| 2 | `podman_falso` de `tests/corre.rs` no probaba el script | `podman_falso_sonda_sale_con_0_rapido_y_no_deja_pid`, `podman_falso_no_da_etxtbsy_con_fork_concurrente` | 3dd0689 |
| 3 | La lista vacía daba un error de sintaxis de `sh` | `podman_falso_sin_contenedores_es_sintaxis_sh_valida`, `podman_falso_sin_contenedores_lista_en_silencio` | 66bf796 |

Ningún test quedó solo como guarda.

## 3. Qué falló o quedó incompleto

Nada quedó sin hacer. Ningún punto necesitó podman. Ningún cambio toca código de producción.

## 4. El rojo de la carrera

El rojo se reprodujo en los dos archivos.

- `corrida.rs`: 2 de 200 repeticiones dieron errno 26 (`ExecutableFileBusy`).
- `tests/corre.rs`: 5 de 200 repeticiones dieron errno 26.
- Tras el arreglo, la carrera pasó 6 de 6 corridas en `corrida.rs`.

## 5. Tests que cambiaron de valor

Ningún test existente cambió de valor.

En `corrida.rs`, el test `sonda_de_podman_falso_no_deja_marca` falló en rojo por el error de sintaxis. Falló antes de llegar a la comprobación del log. Hoy comprueba las dos cosas.

## 6. Preguntas para el planner

1. **El helper de `corrida.rs` también tenía el error de la lista vacía.** El brief lo pedía solo para `limpieza.rs`. Decisión: el organizador agregó `:` en `corrida.rs` y un test. La regla del brief dice que un helper no imprime errores falsos.
2. **Dos tests comparten el nombre `podman_falso_no_da_etxtbsy_con_fork_concurrente`.** Están en `limpieza.rs` y `corrida.rs`. Los módulos distintos evitan el choque. Decisión: se dejó el mismo nombre para que un filtro de `cargo test` los encuentre juntos.
3. **Un helper común.** Hay tres copias de la prueba con `--sonda`. Decisión: no se unificó. `tests/` no ve `src/`, y el brief pidió copiar.
