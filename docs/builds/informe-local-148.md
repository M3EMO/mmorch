# Informe de la Tarea 148

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 05a5364 | 3790 (calculado) | 0 | 5 |
| Final | 3793 | 0 | 5 |

La base no se corrió. Se calcula como el final menos los 3 tests nuevos. Ningún test se borró.
Ningún commit necesitó reintento del hook.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La lista de la sonda se perdía en la cola de 4000 caracteres del stdout | `sonda_lastre_con_la_cola_de_4000_del_stdout_da_las_300_funciones`, `sonda_lastre_sin_archivo_da_el_error_de_las_marcas`. Guarda: la calibración de punta a punta de la 143 | e69b3a2 |
| 2 | `MOTIVO_LASTRE_SIN_LUGAR` hablaba de una función vieja | `motivo_de_lastre_sin_lugar_nombra_la_sonda_y_no_habla_de_funcion_vieja` | 22178a7 |

## 3. Qué falló o quedó incompleto

- Quedan sin probar los tests con podman y la calibración de Eficiencia en Colab. Los corre el orquestador.
- Dos documentos fuera de la tabla quedan desactualizados: `docs/builds/informe-local-147.md`, líneas 29 y 94. Dicen que el motivo conserva "función vieja". No se tocaron.

## 4. Lo que se borró

- `INICIO` y `FIN` (constantes), `tramo` y `salida_valida` de `sonda_lastre.rs`.
- Sus únicos llamadores eran `leer_salida`, `salida_valida` en `medir_lastre` y los tests de la sonda. Todos se actualizaron.
- `grep -w` no halló otros usos fuera de esos dos archivos.

## 5. Los lectores del stdout

- `src/oracle/colab.rs:1172`: `marcador(&lectura.stdout, "ROOT_GPU=")`. El stdout de Colab tiene tope de 16 MiB (`TOPE_SALIDA_COLAB`) y la salida del guion de GPU es de una línea. No hay riesgo hoy.
- `src/corre/corrida.rs:740`: `resultado_exitoso` busca la línea de resultado en el stdout de corridas previas. No es una marca de inicio.
- `src/oracle/contenedor.rs`: el lector de `STEP` guarda el paso aparte porque la cola puede perderlo. Es el mismo problema, resuelto de otra forma.
- Ningún otro código busca una marca de inicio en el stdout de `ejecutar` o `ejecutar_en`.

## 6. Tests que cambiaron de valor

En `sonda_lastre.rs`:
- `sonda_lastre_leer_salida_da_las_funciones_que_corrieron`: lee una lista, no marcas.
- `sonda_lastre_leer_salida_ignora_lo_que_esta_fuera_de_las_marcas` pasó a `..._lista_vacia_no_da_funciones`. Las marcas ya no existen.
- `sonda_lastre_comando_corre_nextest_sin_cortar_y_lista_marcas` pasó a `..._lista_al_archivo`.
- `sonda_lastre_comando_lleva_el_filtro_antes_de_la_marca` pasó a `..._antes_del_ls`.

En `src/oracle/mod.rs`, archivo de la tabla:
- `SondaFalsa` y `LastreFalso` implementan `ejecutar_en` y escriben el archivo. Detectan la sonda por `root-sonda.lista`.
- `SondaFalsa` recibe `Result<Option<&str>, &str>`: `Some` escribe la lista y `None` no escribe nada.
- Cambiaron los tests de comando, de salida válida y de salida sin marcas.

No se editó ningún test de archivos fuera de la tabla.

## 7. Llamadores revisados

- `cmd_sonda`, `leer_salida`: solo `medir_lastre` y los tests de la sonda. Codegraph y `grep -w` coinciden.
- `salida_valida`, `tramo`: borrados. Sin otros llamadores.
- `MOTIVO_LASTRE_SIN_LUGAR`: solo `calibra.rs`. El texto viejo queda solo en `informe-local-136.md` (histórico) y `informe-local-147.md`.

## 8. Preguntas para el planner

1. Los informes 147 (líneas 29 y 94) describen el motivo viejo. Decisión: no se tocaron, porque están fuera de la tabla. Conviene anotar la corrección en el plan.
2. `SondaFalsa` cambió su firma de respuesta a `Option`. Decisión: se eligió `Option` en lugar de un campo booleano, por simplicidad.
