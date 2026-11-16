# Informe de la Tarea 157

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes de la lib | Fallas | Ignorados |
|---|---|---|---|
| Base (505cf34) | 3323 | 0 | 1 |
| Final (db48893) | 3331 | 0 | 1 |

- Los 8 tests nuevos de la lib son 6 en `colab_calibra.rs` y 2 en `cli.rs`.
- Los demás binarios de test no cambian: todos verdes.
- La base sale de restar los 8 tests nuevos al total final.
- Ningún test de tiempo falló en el hook. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 2 | El ciclo siempre llama a `stop` y no anota el runtime abierto | `sin_stop_no_llama_a_stop`, `sin_stop_imprime_las_tres_lineas_por_runtime`, `sin_stop_no_filtra_el_valor_de_una_clave`, `sin_stop_con_trabajo_que_falla_deja_el_runtime_abierto`, `sin_stop_conserva_la_sesion_en_activo_json`. Guarda: `sin_la_bandera_la_lista_termina_en_stop` | 36a59f3 |
| 1 | `colab calibrar` no tiene la bandera `--sin-stop` | `colab_calibrar_con_sin_stop_lo_activa` (no compilaba). Guarda: `colab_calibrar_sin_la_bandera_da_falso` | db48893 |

El punto 2 va primero porque el punto 1 necesita su firma. El commit del punto 2 toca `lib.rs`: pasa `false` fijo, para que compile. El punto 1 cablea la bandera.

## 3. Qué falló o quedó incompleto

- **El campo "abierto a propósito" en `activo.json` no existe.** `EntradaActivo` vive en `src/oracle/colab.rs`, fuera de mi lista. `activo.json` conserva la sesión sin marca. El aviso impreso dice que el runtime queda abierto.
- **El cableado de `lib.rs` no tiene test propio.** El test del punto 1 prueba el parseo de clap. El compilador obliga a nombrar el campo en el brazo del `match`.
- **Quedan sin probar:** la suite con podman, y la Prueba 0g con `--sin-stop` en Colab. Las corre el orquestador.
- **Sin captura del texto de fallo de cada rojo.** El subagente del punto 2 anotó solo la aserción que falló.
- **`docs/interfaces.md` es doc escrito a mano.** Lo actualicé a mano. No hallé un generador. No revisé `r00t esquema` a fondo, y ningún test de esquema falló.

## 4. Las líneas impresas

Valores falsos:

```
ROOT_SESION_ABIERTA=root-tarea-prueba-0
ROOT_CARPETA_REMOTA=/content/root-paquete
ROOT_TRABAJO_CMD=[{"cmd":["/content/root-paquete/r00t","oraculo","calibrar","--tarea","tarea-prueba"],"dimension":"eficiencia","entorno":{"GITHUB_TOKEN":null,"IDIOMA_SALIDA":"es","ROOT_HOME":"/content/root-paquete"},"tarea":"tarea-prueba"}]
El runtime root-tarea-prueba-0 sigue abierto y gasta cerca de 0,08 unidades por hora. Ciérrelo con `colab stop -s root-tarea-prueba-0` o con `oraculo pendientes --liberar`.
```

- Las tres líneas `ROOT_` salen sin sangría, para buscarlas con `grep ^ROOT_`.
- El aviso es una cuarta línea.
- Con `--json`, cada runtime suma la clave `abierto` con `sesion`, `carpeta` y `trabajo_cmd`.

## 5. Tests que cambiaron de valor

- Ningún test cambió de valor.
- No edité tests de otros archivos. El helper `correr` de `tests_calibrar` pasa `false` por la firma nueva. Está en `colab_calibra.rs`, mi archivo.

## 6. Llamadores revisados

| Firma | codegraph | grep -w |
|---|---|---|
| `calibrar_cli` (suma `sin_stop`) | `colab_calibrar` (`lib.rs`) | el mismo |
| `calibrar_con` de `colab_calibra` (suma `sin_stop`) | `correr` (tests) | el mismo, más `calibrar_con` de `oracle/calibra.rs`, que es otra función |
| `ciclo_de_runtime` (suma `sin_stop`) | `calibrar_con` | el mismo |
| `colab_calibrar` en `lib.rs` (suma `sin_stop`) | `ejecutar_sin_codigo` | el mismo |
| `ResultadoRuntime` (suma `abierto`) | `sin_runtime` y `ciclo_de_runtime` lo construyen | el mismo |

## 7. Preguntas para el planner

1. **¿La marca de "abierto a propósito" va en `activo.json`?** Pide un campo en `src/oracle/colab.rs`. Decisión: no lo toqué. El aviso impreso cubre el caso.
2. **¿`ROOT_TRABAJO_CMD` lleva un elemento por trabajo?** El brief dice "el comando del trabajo". Decisión: un array JSON, un elemento por trabajo, con `cmd`, `entorno`, `tarea` y `dimension`.
3. **¿Cómo se marca un valor secreto?** Decisión: el nombre va y el valor es `null`. El orquestador pone el valor al repetir el trabajo.
4. **¿`ROOT_TRABAJO_CMD` lleva `ROOT_HOME`?** El script real lo fija. Decisión: va como `/content/root-paquete`.
5. **¿El runtime queda anotado como abierto si `colab new` falla?** Decisión: sí. Con `--sin-stop` el aviso sale igual. El texto puede nombrar un runtime que no existe. `colab stop` o `--liberar` lo limpian sin daño.
6. **El script `trabajo-N.py` sigue enviando los valores secretos al runtime.** Es el comportamiento de siempre. No cambia con esta tarea. Va como aviso.
7. **El costo de 0,08 unidades por hora sale del brief.** No lo verifiqué contra la CLI real.
8. **¿El aviso cita `colab stop -s <sesión>`?** Decisión: sí, con la sesión real. No verifiqué la sintaxis contra la CLI real.
