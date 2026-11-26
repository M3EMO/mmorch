# Brief de la Tarea 175b para el builder local

Este brief vale solo. No necesita otra conversación. La Tarea 175b arregla un test de la 175. No cambia ningún guion ni el binario.

## El defecto

El test `toda_regla_va_detras_de_nsenter` de `tests/red_builder.rs` falla en la laptop 7 de 8 veces. En Colab pasó.

- El `slirp4netns` falso corre en segundo plano y anota su llamada en el mismo registro que los demás falsos.
- Esa línea cae a veces entre `nsenter -t <pid> -U -n iptables ...` y la línea `iptables ...` del falso.
- El test exige que la línea previa a cada `iptables` sea su `nsenter`, y por eso falla.
- El guion `tools/colab/red-builder.sh` está bien: el orden real no depende de esa línea.

La falla vista, literal: "`iptables -C OUTPUT -d 10.0.2.3 -p udp --dport 53 -j ACCEPT` debe ir justo detrás de nsenter". La línea previa era la de `slirp4netns --configure ...`.

## Qué hacer

1. Escribí primero una prueba en rojo que reproduzca el orden mezclado sin depender del azar. Por ejemplo, separá la revisión de adyacencia en una función que reciba la lista de llamadas, y probala con una lista que tenga la línea de `slirp4netns` en el medio.
2. Hacé que la revisión de adyacencia ignore las líneas que empiezan con `slirp4netns `. Un comentario de una línea dice por qué: el falso corre en segundo plano.
3. Revisá los otros tests de `tests/red_builder.rs` que miran el orden o la adyacencia, con `position`, `rposition` o índices. Si alguno puede fallar por la misma línea, arreglalo igual. Si no, anotá en el informe por qué no.
4. Corré `cargo test --test red_builder` diez veces seguidas. Las diez deben dar verde. Anotá el resultado en el informe.

## Límites

- Tocá solo `tests/red_builder.rs` y el informe `docs/builds/informe-local-175b.md`.
- No cambies `tools/colab/red-builder.sh`, ni el falso de `slirp4netns`, salvo que el arreglo del test no alcance. En ese caso, frená y anotalo como pregunta.
- No corras nada con podman. Exportá `ROOT_SIN_PODMAN=1`.

## El informe

El informe trae la tabla de commits, el resultado de las diez corridas, la suite completa con `tools/test-seguro.sh` y las "Preguntas para el planner".
