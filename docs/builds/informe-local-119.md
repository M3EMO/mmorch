# Informe de la Tarea 119

Base: cfb8970. Rama: `laptop/tarea-119`. Todo corrió con `ROOT_SIN_PODMAN=1`. Nada usó podman, API ni `ROOT_HOME` real.

## 1. Resultado de la suite

| Momento | Verdes (lib) | Fallas | Ignorados |
|---|---|---|---|
| Base cfb8970 | 2849 | 0 | 1 |
| Final | 2863 | 0 | 1 |

Los demás binarios de test quedan verdes y sin cambio. Ningún hook falló por tiempo. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Cada llamada crea un `/salida` nuevo | `ejecutar_en` no existe en el trait (E0599, E0407). `medir` recibe 2 argumentos y no 3 (E0061). Los tests de carpeta por lado no compilan. | c3c598e |
| 2 | La memoria pico incluye a `rustc` | `cmd_tests` conserva `--no-run`. La corrida con `Compiling` deja memoria con valor. La compilación fallida no deja el lado sin valor. | 01df1fa |
| 3 | Un id desconocido no dispara | `informe_eficiencia_id_desconocido_con_regresion_dispara` da Listo y se esperaba NoListo. | 2168f9a |
| 4 | El empate nombra la última del vector | `informe_eficiencia_empate_nombra_la_medida_del_orden_fijo` y `informe_eficiencia_empate_con_id_desconocido_gana_el_conocido`. | 0d05e64 |

Guardas: los tests de eficiencia de la 115, `informe_eficiencia_*`, `ejecutar_en_por_defecto_llama_a_ejecutar_sin_usar_la_carpeta` y `informe_eficiencia_mayor_razon_desconocida_sigue_ganando`.

Los commits 2168f9a y 0d05e64 salen de un cherry-pick de un worktree auxiliar. El auxiliar corrió en paralelo con los puntos 1 y 2. Esos dos commits llevan el trailer "Claude Haiku 5.5" y los otros dos llevan "Claude Sonnet 5.5". El subagente de los puntos 3 y 4 se identificó como Haiku.

## 3. Qué falló o quedó incompleto

- **Sin probar con podman.** Los cinco puntos de la sección "Tests con podman" del brief siguen sin probar. Tampoco corrió la prueba del orquestador.
- **`grep` en la imagen.** `cmd_tests` usa `grep`. Nadie verificó que la imagen lo traiga.
- **Rama `Err` de `compilar_tests`.** No tiene test directo. El ejecutor falso no devuelve `Err` en esa llamada.
- **Rama sobrante.** La rama local `laptop/tarea-119-inf` queda sin borrar. Sus commits están en `laptop/tarea-119`. Borrala con `git branch -D laptop/tarea-119-inf`.
- **Orden entre ids desconocidos.** Con dos ids desconocidos de igual razón, gana el último del vector. Ningún orden los separa.

## 4. Los comandos de cada medida

Los dos comandos nuevos, con `{PRELUDIO}` y `{filtro}` sin expandir:

`cmd_compilar_tests()`:

```
{PRELUDIO}cd /repo && export CARGO_TARGET_DIR=/salida/target && cargo nextest run --no-run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml >/dev/null 2>&1; echo "ROOT_COMPILAR $?"
```

`cmd_tests(filtro)`:

```
{PRELUDIO}cd /repo && export CARGO_TARGET_DIR=/salida/target && a=$(date +%s%N); cargo nextest run --offline --profile ci --tool-config-file root:/etc/root-oraculo/nextest.toml{filtro} 2>&1 | grep -E '^ *Compiling'; b=$(date +%s%N); echo "ROOT_NS $((b-a))"; echo "ROOT_MEM $(cat /sys/fs/cgroup/memory.peak 2>/dev/null)"
```

`cmd_metadata`, `cmd_lista` y `cmd_compilacion` no cambian.

**Carpetas.** `medir` crea `home/tmp/sondas-XXXX/` con `carpeta_de_salida`. Dentro crea `inicial/` y `repo/`. Todas las llamadas de un lado usan su subcarpeta como `/salida`: metadata, lista, compilación de tests, corridas medidas y build. Al terminar, `medir` borra la carpeta con `borrado::borrar_dir_dentro`, también tras un error.

**Detección de recompilación.** Una línea `Compiling` en el `stdout` de una corrida medida deja tiempo y memoria sin valor. El motivo es "la corrida medida recompiló".

**Orden de las llamadas por lado.** Metadata, lista, compilación de tests, tres corridas medidas y tres builds. Los dos lados se alternan.

## 5. Tests que cambiaron de valor

- Seis llamadas `medir(&...)` de `eficiencia.rs` pasan a `medir_de_prueba(&...)`. Es un wrapper que crea su `tempdir`. Las aserciones no cambian.
- `Falso` de `eficiencia.rs` suma el campo `compilar`, con valor por defecto `ROOT_COMPILAR 0`. Clasifica `--no-run` como compilación de tests, y `ROOT_NS` sin `--no-run` como corrida medida.
- **Test de otro archivo.** `EficienciaFalsa` en `src/oracle/mod.rs` leía el literal `--no-run`. Ahora acepta `cmd_compilar_tests()` y responde `ROOT_COMPILAR 0`. El cambio viajó en el commit del punto 2.
- Ningún test de `informe.rs` cambió de valor. El test de empate del punto 4 pone `tiempo_tests` primero en el vector. Con `tamano_binario` primero ya pasaba antes del cambio, así que no era rojo.

## 6. Llamadores revisados

- **`medir`.** codegraph lista solo definiciones ajenas con ese nombre. grep -w da un llamador de producción, `src/oracle/mod.rs:840`, y seis de test en `eficiencia.rs`. Todos usan la firma nueva.
- **`carpeta_de_salida`.** Ahora es `pub(crate)`. codegraph y grep dan `herramientas.rs:95`, un test en `herramientas.rs:1635` y `eficiencia.rs:581`.
- **`ejecutar_en`.** Es nuevo y tiene un valor por defecto. grep da el trait, `EjecutorContenedor`, `Lado::ejecutar` y dos ejecutores de test.
- **Wrappers de memo.** `EjecutorGrabador` y `EjecutorReusador` de `memo.rs` no reenvían `ejecutar_en`. `mod.rs` pasa a `medir` los ejecutores de `deps.herramientas`, sin memo. Si un wrapper llegara a `medir`, ignoraría la carpeta fija.

## 7. Preguntas para el planner

1. **El borrado falla.** `medir` ignora un error de `borrar_dir_dentro`. La carpeta puede quedar con un `target/` de varios GB. Decisión: no avisar y no cambiar la medida.
2. **Prefijo del motivo.** "la corrida medida recompiló" no nombra el lado. El brief lo dio así. Decisión: dejarlo sin prefijo.
3. **Marca de la compilación de tests.** El brief decía que esa llamada no lee nada. Sin una marca, una compilación fallida pasa inadvertida. Decisión: leer `ROOT_COMPILAR <código>`. Un código distinto de cero deja el lado sin valor, con el motivo "<lado>: la compilación de los tests falló".
4. **Frase genérica sin números.** Si faltan referencia o repo, la frase es "La medida <id> empeora <razón> veces." Hoy `mod.rs` siempre llena esos campos.
5. **Línea del resumen con números faltantes.** `linea_de_eficiencia` usa la frase genérica para un id conocido sin números. Antes no daba línea. El efecto práctico es nulo hoy.
6. **Desempate entre ids desconocidos.** Decisión: no inventar un orden. Falta decidir si debe ser alfabético.
