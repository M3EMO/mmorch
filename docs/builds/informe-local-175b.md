# Informe de la Tarea 175b

Arregla `toda_regla_va_detras_de_nsenter` de `tests/red_builder.rs`. No cambia guiones ni binario.

## Causa

El `slirp4netns` falso corre en segundo plano y anota su llamada en el mismo registro. Esa línea caía a veces entre `nsenter -t <pid> -U -n iptables ...` y la línea `iptables ...`, y el test exigía adyacencia estricta.

## Arreglo

- Nueva función `revisar_adyacencia_nsenter(ls, pid)`, que recibe la lista de llamadas.
- Ignora las líneas que empiezan con `slirp4netns ` antes de revisar adyacencia (comentario de una línea).
- Rojo primero: `adyacencia_ignora_slirp_en_segundo_plano` falló antes del filtro (lista nsenter, slirp4netns, iptables) y pasa después. `adyacencia_exige_nsenter_justo_antes` cubre el caso negativo (ya pasaba en rojo, como se esperaba).
- Lo construyó un subagente Haiku; revisé el diff antes del commit.

## Otros tests de `tests/red_builder.rs`

Revisados `unshare_y_attach`, `con_nombre_puesto_y_sosten_muerto_recrea` y `la_comprobacion_corre_al_final` (los que usan `position`/`rposition`). No fallan por la misma línea: comparan orden relativo (`<`, `>`) entre llamadas, no adyacencia, así que una línea ajena en medio no cambia el resultado. `reglas_de_dns_van_antes_que_los_cortes` filtra por `iptables -A` y tampoco depende de adyacencia. No hizo falta tocar ninguno.

## Commits

| Commit | Contenido |
|---|---|
| 1e82ccc | Punto 1: la adyacencia de nsenter ignora la línea de slirp4netns |
| (este informe) | Informe de la Tarea 175b |

## Diez corridas de `cargo test --test red_builder`

Diez de diez en verde (25 passed, 0 failed cada una, ~2,2 s).

## Suite completa (`tools/test-seguro.sh`, `ROOT_SIN_PODMAN=1`)

Salida 0. 4260 passed, 0 failed. Los tests con `saltar_sin_podman` quedaron salteados; el orquestador los corre al mezclar.

## Notas

El hook de commit no falló por tiempo: no hizo falta reintento.

## Preguntas para el planner

Ninguna.
