# Informe de la Tarea 117 (builder local)

Rama `laptop/tarea-117`, base 64361fa. Todo corrió con `ROOT_SIN_PODMAN=1`.

## 1. Resultado de la suite

| Momento | Pruebas de la librería | Fallas | Ignoradas |
|---|---|---|---|
| Base 64361fa | 2751 verdes | 0 | 0 |
| Final | 2759 verdes | 0 | 1 (la medición del punto 3) |

La suma de la base incluye los otros binarios de `tests/`. Todos quedaron en verde.
Los tests nuevos son 5 del punto 1 y 3 del punto 2. El punto 3 suma 1 ignorado.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Cuatro copias usaban `cp -r` y ponían fechas nuevas | `comando_eficiencia_no_usa_cp_r`, `comando_oculto_no_usa_cp_r`, `comando_oculto_con_target_no_usa_cp_r`, `comando_lock_no_usa_cp_r`, `comando_lock_copia_repo_con_cp_a` | 302aca0 |
| 2 | Los contenedores compilaban con debuginfo | `contenedor_argumentos_llevan_perfil_sin_debuginfo`, `contenedor_argumentos_separado_llevan_perfil_sin_debuginfo`, `contenedor_env_propio_manda_sobre_el_perfil_sin_debuginfo` | 857fd6c |
| 3 | Sin medida de la ganancia | Ninguno: es una medición con `#[ignore]` | Ver el commit del informe |

Las guardas de los puntos 1 y 2 siguen verdes. Son `descarga_comando_lock_genera_en_la_copia_del_repo_con_su_config` y los tests de endurecimiento y montajes de `contenedor.rs`.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- La guarda con podman no se probó aquí, por la regla de la tarea. La corre el orquestador.
- `src/corre/contenedor.rs` tiene su propia función `argumentos`. Esa función no usa `opciones`. Por eso el builder de Claude no recibe las variables. Está fuera de mi lista de archivos. Ver la pregunta 1.

## 4. La ganancia medida

El test es `perfil_sin_debuginfo_da_los_mismos_tests_y_mide_la_ganancia`. Corrió 3 veces en serie, sobre el fixture de calibración, que no tiene dependencias.

| Lado | Mínimo de tiempo (3 corridas) | Tamaño del `target` (mínimo) |
|---|---|---|
| A, sin variables | 0,35 s | 19 141 154 bytes |
| B, con `debug=0` | 0,30 s | 4 068 139 bytes |

- Las corridas de A dieron 0,35, 0,40 y 0,62 s. Las de B dieron 0,34, 0,30 y 0,34 s.
- La diferencia de tiempo cae dentro del ruido. El fixture compila en menos de un segundo.
- El tamaño sí cambia: el `target` pesa unas 4,7 veces menos.
- Los dos lados dan los mismos tests con el mismo resultado. El test lo afirma.
- No se midió un proyecto grande. La ganancia de tiempo real en producción sigue sin medir.
- Una doble corrida en paralelo se descartó, porque sus tiempos no son comparables.

## 5. Guardas con podman para el orquestador

- `target_podman_reusado_da_la_misma_completitud_que_en_frio` (punto 1).
- Los tests de `contenedor.rs` con `saltar_sin_podman` siguen igual.

## 6. Tests que cambiaron de valor

- `descarga_comando_lock_genera_en_la_copia_del_repo_con_su_config`: su literal pasó de `cp -r /repo/. /tmp/w/` a `cp -a /repo/. /tmp/w/`. El comando cambió en el punto 1.
- Ningún otro test existente cambió.

## 7. Preguntas para el planner

1. `src/corre/contenedor.rs` arma sus propios argumentos y no recibe las variables de debuginfo. ¿Se suma allí? Decisión: no se tocó. El archivo está prohibido en esta tarea.
2. El test de medición no mide un proyecto con dependencias. ¿Se necesita esa medida? Decisión: no. El brief pide el fixture.

## Notas

- El hook pasó a la primera en los commits de los puntos 1 y 2. No hubo fallas de tiempo.
- Durante el trabajo vi un contenedor `cargo mutants` ajeno, con el `ROOT_HOME` real. No es de esta tarea y no lo toqué.
