# Informe de la Tarea 107

## 1. Resultado de la suite

Corrida con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (88169de, cifra final del informe 106) | 3286 | 0 | 4 |
| Final | 3310 | 0 | 4 |

`cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` pasan. Ningún commit tuvo falla de tiempo en el hook.
Los tests que necesitan podman no corrieron. El orquestador los corre al mezclar.

## 2. Commits

Los puntos 1 y 4 se construyeron en worktrees aparte (`local107-p1`, `local107-p4`) y entraron con `cherry-pick`.

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 3 | El orden de los reemplazos dependía del hilo | `reemplazos_con_cuatro_hilos_dan_las_mismas_inyecciones_con_la_misma_semilla`, `la_reserva_sobrante_va_al_caso_de_menor_indice_con_cualquier_orden`. Guarda ya verde: `con_una_principal_y_una_reserva_por_caso_ningun_caso_comparte_inyeccion` | e340097 |
| 1 | El perfil `ci` no tenía `slow-timeout` | `c_construir_tiene_default_4`, `d_probar_tiene_default_4_y_chequeo_nextest`, `f_nextest_toml_va_despues_de_nextest_con_el_contenido`, `g_probar_chequea_slow_timeout_en_perfil_ci`. El script prueba la imagen al mezclar | 7baf37b |
| 4 | Una señal dejaba vivos los contenedores de `evaluar`, `calibrar` y `auditar` | 7 tests de `limpieza.rs` (señal, SIGINT con 130, pánico, barrido, pid reusado, pids con el mismo inicio). Guarda: tests de señal de `corre` | 13e3e6c |
| 5 | Una entrada de equivalente repetida se aceptaba | `equivalente_repetido_es_error_de_carga_con_archivo_linea_y_edicion` | 4726c3c |
| 2 | El plazo de suite era fijo en 3600 s | Ninguno en rojo (ver sección 3). Guarda: nextest con junit y código 100 | 92c9ce4 |

## 3. Qué falló o quedó incompleto

- **Punto 2 sin rojo previo.** El subagente escribió la implementación antes que los tests. Lo compensó con cuatro mutaciones a mano. Cada mutante lo cazó un test. Los tests nuevos nunca se vieron en rojo.
- **Punto 4 sin margen de 300 s.** Los nombres de contenedor no llevan hora de inicio. El barrido usa solo la regla de `/proc` (pid sin proceso `r00t` vivo). Un pid reusado por otro `r00t` deja un contenedor viejo.
- **Punto 4 duplica código de `corre`.** `limpieza.rs` copia el vigía de señales y el borrado por prefijo. `corrida.rs`, `ejecutor.rs` y `barrido.rs` quedaban fuera de la lista.
- **Punto 4 no se probó con podman real.** Los tests usan un `podman` falso.
- **Punto 1 sin imagen.** No se construyó `:4`. Lo prueba `scripts/probar-imagen-oraculo.sh` al mezclar.
- **Punto 3 pierde un motivo.** Desaparece el motivo "ninguna línea candidata está en una función que la suite ejecute". Todo agotamiento dice "candidatos distintos agotados".

## 4. `slow-timeout`

- Sintaxis: `slow-timeout = { period = "60s", terminate-after = 5 }` dentro de `[profile.ci]`.
- Fuente: https://nexte.st/docs/features/slow-tests/ (la forma en línea con `period` y `terminate-after`).
- La página no dice desde qué versión existe. No se confirmó contra 0.9.100. La URL `.../configuration/slow-tests/` da 404.
- El orquestador debe confirmar la sintaxis al construir `:4` y correr el script de prueba.

## 5. Tests que cambiaron de valor

- Se borró `fila_cuenta_una_vez_los_equivalentes_repetidos` (calibra.rs). Cargaba dos entradas iguales y esperaba contar una. La carga ahora lo rechaza. El punto 5 cubre el caso.
- `tests/imagen_oraculo.rs`: dos tests pasan de `:3` a `:4` y se renombran. El texto del `printf` de nextest suma `slow-timeout`.
- `tests/calibra.rs`: tres literales de `Fila` suman `duracion_contrato` y `duracion_oculto` en `None`. Solo compilan.
- La clave de "equivalente repetido" recorta los espacios de la edición, igual que `cuantos_equivalentes`.

## 6. Llamadores revisados

| Símbolo | Cambio | Codegraph y grep |
|---|---|---|
| `preelegir`, `Ronda`, `Preelegidas` | campos `previos` y `usadas` borrados, `candidatos` nuevo | Un llamador de `preelegir`: `calibrar_medida_con_hilos`. Sin uso externo de los campos. |
| `MOTIVO_AGOTADAS`, `MOTIVO_SIN_EJECUTAR` | borrados | Cero llamadores en ambos. |
| `la_suite_no_la_ejecuta` | devuelve `Result<Option<bool>>` | Un llamador: `medir_caso`. Ambos coinciden. |
| `Fila` | dos campos nuevos con `serde(default)` | Literales en `calibra.rs`, `oracle/mod.rs` y `tests/calibra.rs`. `docs/interfaces.md` regenerado. |
| `correr_comando`, `PLAZO_LIMPIEZA` | pasan a `pub(crate)` | Solo los usa `limpieza.rs`. |
| `cuantos_equivalentes` | sin cambio | Su deduplicación ya no se alcanza con tareas cargadas. Queda por seguridad. |

## 7. Preguntas para el planner

1. ¿Se agrega la hora de inicio al nombre del contenedor, o se consulta `podman inspect`, para aplicar el margen de 300 s? Decisión: no se aplica el margen. El barrido es conservador.
2. ¿`corre` pasa a usar `limpieza.rs` en una tarea aparte? Decisión: se deja la duplicación.
3. Si `evaluar`, `calibrar` o `auditar` no registran las señales, salen con error de infraestructura, igual que `colab`. ¿Mejor seguir con un aviso? Decisión: se deja el error.
4. La señal no mata el `podman run` hijo. Un contenedor creado justo después puede escapar. Lo barre el arranque siguiente. Decisión: se acepta.
5. ¿La duración de la base es la corrida más lenta de cada suite, o la suma de repeticiones? Decisión: la más lenta, porque el plazo vale por corrida.
6. Un vencimiento del plazo de Root con junit presente sigue abortando la calibración. Decisión: se deja como hoy.
7. Un caso limpio que vence el plazo aborta la calibración con error, porque no tiene inyección que reemplazar. Decisión: se deja el error.
8. El plazo de Root se reconoce por el texto "timeout tras ". Un error tipado es más limpio, pero toca `contenedor.rs` y `ErrorOraculo`. Decisión: se deja el acoplamiento por texto.
9. El estado de duraciones y plazos es thread-local, igual que el de mutación. Decisión: se sigue el patrón del repo.
10. El punto 3 copia el repo hasta cinco veces más por caso con defecto antes de medir. Decisión: se acepta el costo.
11. Si una inyección elegida no se vuelve a aplicar al medir, el punto 3 devuelve error de infraestructura. Decisión: se deja el error, porque indica no determinismo.
12. Se acepta perder el motivo de la sonda (sección 3). Decisión: sí, según el texto del punto 3 del brief.
