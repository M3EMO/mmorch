# Brief de la Tarea 176b para el builder local

Este brief vale solo. No necesita otra conversación. La Tarea 176b termina el punto 1 de la 176: el aviso llega al veredicto.

## Lo que falta

La 176 dejó en `src/corre/bwrap.rs` la constante `AVISO_PROC_BUILDER` y el método `EjecutorBwrapBuilder::aviso_proc()`. El texto es "builder con /proc del runtime en solo lectura". Nadie los llama todavía.

El brief de la 176 pedía ese aviso en el veredicto, y su tabla de archivos no nombraba `src/corre/corrida.rs`. Por eso quedó sin cablear.

## Qué hacer

1. Escribí primero un test en rojo en `src/corre/corrida.rs`: con `ROOT_PROC_BUILDER` fijada y el modo bwrap, el veredicto lleva el aviso una sola vez.
2. Escribí un segundo test: sin `ROOT_PROC_BUILDER`, el veredicto no lo lleva.
3. Cableá el aviso junto a `anotar_ejecutor`, que hoy agrega `AVISO_BWRAP` en `correr_interno`. El aviso nuevo sale de `aviso_proc()` del ejecutor elegido. No lo repitas si ya está.
4. Actualizá el comentario de `correr`, que hoy nombra solo el aviso "builder en bwrap".

Usá la forma que ya tiene el código: `anotar_ejecutor`, `EjecutorElegido` y sus tests `corre_anotar_ejecutor_*`. No agregues tipos nuevos si un parámetro alcanza.

## Límites

- Tocá solo `src/corre/corrida.rs`, `src/corre/bwrap.rs` si hace falta exponer algo, y el informe `docs/builds/informe-local-176b.md`.
- No cambies la revisión de mountinfo ni los argumentos de bwrap de la 176.
- No corras nada con podman. Exportá `ROOT_SIN_PODMAN=1`.

## El informe

El informe trae la tabla de commits, el rojo visto de cada test, la suite completa con `tools/test-seguro.sh` y las "Preguntas para el planner".
