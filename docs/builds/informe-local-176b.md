# Informe de la Tarea 176b

El aviso de `/proc` del builder llega al veredicto. Rama `laptop/tarea-176b`, base `laptop/cadena-176` (cd2a76e). Sin push.

## Commits

| Commit | Punto |
|---|---|
| 896b797 | Punto 1: `anotar_ejecutor` recibe `aviso_proc: Option<&str>`; `EjecutorElegido::aviso_proc()` delega en `EjecutorBwrapBuilder::aviso_proc()`; `correr_interno` lo pasa; comentario de `correr` actualizado |
| (este informe) | Informe de la Tarea 176b |

Solo se tocó `src/corre/corrida.rs`. `bwrap.rs` no hizo falta. No cambian la revisión de mountinfo ni los argumentos de bwrap.

## Rojo visto

Un subagente Haiku escribió los tests antes de la implementación. El rojo fue de compilación (9 errores):
- E0061: `anotar_ejecutor` takes 2 arguments but 3 supplied.
- E0599: no method named `aviso_proc` found for enum `EjecutorElegido`.

Tests nuevos: `corre_aviso_proc_con_proc_builder_deja_el_aviso_una_sola_vez` (llama dos veces; el aviso sale una vez) y `corre_aviso_proc_sin_proc_builder_no_lo_lleva`. Los dos `corre_anotar_ejecutor_*` pasan `None`. En verde, los cuatro pasan y `cargo clippy --lib -- -D warnings` queda limpio.

Los tests arman el ejecutor con `elegir_ejecutor` y no lanzan nada ni leen mountinfo; no necesitan `saltar_sin_podman`.

## Suite

`ROOT_SIN_PODMAN=1 tools/test-seguro.sh`: rc=0, 4275 passed, 0 failed, 5 ignored.

## Hook

El commit pasó al primer intento; no hubo reintento por tests de tiempo.

## Preguntas para el planner

1. Los tests cubren `anotar_ejecutor` y `elegir_ejecutor`, no una corrida completa (`correr`) en bwrap. Una corrida real en bwrap lanzaría `bwrap` y su chequeo previo lee mountinfo. Decisión: no cubrirla aquí; queda para el orquestador si la quiere.
2. Orden de los avisos: con las dos variables, el aviso de `/proc` queda antes que "builder en bwrap" en `avisos`. Decisión: no importa el orden; nada lo lee.
