# Informe de la Tarea 176c

`colab empaquetar --corre` ahora exige `GOAL.hash` en la raíz de Root y lo copia al paquete, junto a `GOAL.md`. No verifica el hash: eso lo hace `r00t corre` en el runtime. Solo se tocó `src/colab_corre.rs`.

## Commits

| Commit | Contenido |
|---|---|
| 9f066bb | Chequeo y copia de `GOAL.hash`, doc del módulo, dos tests nuevos y `GOAL.hash` en `Escenario::nuevo` |
| (este) | Informe |

Los tests y el código van en un solo commit: un commit con tests en rojo no pasaría el hook.

## Rojo visto

Ambos tests se corrieron antes del cambio de producción, con `ROOT_SIN_PODMAN=1 bash tools/test-seguro.sh --lib <nombre>`. Ambos compilan y fallan en runtime.

- `corre_paquete_trae_goal_hash_igual_al_de_la_raiz`: `panicked at src/colab_corre.rs:1727:9: falta GOAL.hash en el paquete`.
- `corre_sin_goal_hash_en_la_raiz_es_error_de_uso`: `se esperaba Uso, llegó Ok(Resumen { ... archivos: 14 ... })`.

Tras el cambio, `--lib colab_corre` dio 81 pasados y 0 fallados.

## Punto 4

El único lugar de `src/colab_corre.rs` que arma una raíz con `GOAL.md` es `Escenario::nuevo`. Se le agregó `GOAL.hash`. Ningún otro test arma su propia raíz.

## Suite completa

`ROOT_SIN_PODMAN=1 bash tools/test-seguro.sh`: salida 0. La suite de la librería dio 3634 pasados, 0 fallados y 1 ignorado. El resto de los binarios dio 0 fallados.

## Hook

El commit de código pasó a la primera. No hubo reintento por tests de tiempo.

## Preguntas para el planner

Ninguna.
