# Informe de la Tarea 112

Rama `laptop/tarea-112`, base 64361fa. Todo se corrió con `ROOT_SIN_PODMAN=1`.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (64361fa) | 3326 | 0 | 4 |
| Final (punto 2) | 3336 | 0 | 4 |

- Una corrida intermedia, tras el punto 1, mostró una falla. Las dos corridas siguientes dieron 0 fallas.
- No se identificó el test de esa falla. Parece un test de tiempo bajo carga.
- El commit del punto 1 y el del punto 2 pasaron el hook al primer intento.
- Un test nuevo usa `saltar_sin_podman` y no corrió: `limpieza::tests::listar_lee_la_hora_de_creacion_de_podman_real`. El orquestador lo corre al mezclar.
- `cargo clippy --all-targets` y `cargo fmt --check` salen limpios.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1, el dueño por tiempo | El barrido daba por muerto un dueño vivo de otro binario (`root-<hash>`) | `dueno_vivo_llamado_root_hash_que_arranco_antes_se_conserva`, `dueno_que_arranco_400_s_despues_de_la_creacion_se_quita`, `dueno_sin_hora_de_arranque_se_conserva`, `contenedor_sin_hora_de_creacion_se_conserva` | 37f1c02 |
| 2, una sola regla con `corre` | `corre::barrido` tenía su propia regla por nombre y su propia lectura de la hora de arranque | `corrida_de_proceso_vivo_llamado_root_hash_que_arranco_antes_no_se_barre` | f1f75ad |

Guardas del punto 1: `dueno_que_arranco_300_s_despues_se_conserva`, `dueno_sin_carpeta_en_proc_se_quita`, `pid_propio_cuenta_como_vivo_aunque_no_tenga_carpeta`, `parsear_listado_*`. Dos de ellos pasaron ya con la regla vieja, por coincidencia.
Guardas del punto 2: los tests de barrido de las Tareas 84, 85, 87 y 108, incluido el margen de 300 s. Quedaron sin cambios, salvo los dos de la sección 5.

## 3. Qué falló o quedó incompleto

- El formato de la hora de creación no se probó contra podman real. La regla de la tarea prohíbe correr podman. Ver la sección 4.
- Si el formato falla, `creado` queda en `None` y el barrido conserva todo. El error va del lado de no borrar, pero el barrido deja de limpiar.
- Se ejecutó una vez `podman --version` por descuido. No creó ni tocó contenedores.
- El test `listar_lee_la_hora_de_creacion_de_podman_real` copia el nombre de la imagen `localhost/root-oraculo-rust:2`. La constante original es `pub(super)` y no se alcanza desde `limpieza.rs`. La plan nombra la imagen `:4` en otro lugar. Conviene que el orquestador confirme que `:2` existe.

## 4. Formato de la hora de creación

- Formato: `podman ps -a --format '{{.Names}} {{.ListContainer.Created.Unix}}'`. Da el nombre, un espacio y los segundos desde la época.
- Fuente: la página de manual `podman-ps(1)` de esta laptop. Dice que `.Created` y `.CreatedAt` dan `Y-M-D H:M:S`, así que no sirven.
- `.ListContainer.Created.Unix` sale de mi lectura del código de podman: la plantilla accede al campo `Created` (un `time.Time`) del struct embebido. **No está verificado.**
- La verificación es el test con podman real. Si falla, hay que cambiar solo la cadena de `--format` en `listar` y `parsear_listado`.

## 5. Tests que cambiaron de valor

Punto 1, en `src/limpieza.rs`. Fijaban la regla del nombre del binario:
- Borrados: `pid_reusado_por_otro_programa_cuenta_como_muerto`, `cmdline_root_con_guion_y_sha_cuenta_como_vivo`, `cmdline_r00tkit_cuenta_como_muerto`, `exe_del_binario_actual_cuenta_como_vivo`.
- Cambiado: `barrido_quita_contenedor_de_pid_muerto_y_conserva_r00t_vivo`, ahora `..._conserva_dueno_vivo`. El vivo lleva `stat` y hora de creación.

Punto 2, en `src/corre/barrido.rs`:
- `pid_reciclado_por_otro_proceso_se_barre`: los cuatro procesos arrancan 400 s después del id. Se barren por "empezó después", no por "no es r00t".
- `cmdline_ilegible_es_de_otro_usuario_y_se_barre` pasó a `cmdline_ilegible_no_cambia_la_regla_y_se_conserva`. El cmdline ya no cuenta. El pid arrancó antes del id, así que la copia queda.

Los tests de las Tareas 107 y 107b que no fijaban el nombre siguen sin cambios.

## 6. Llamadores revisados

| Símbolo | Cambio | codegraph | grep -w |
|---|---|---|---|
| `r00t_vivo` | borrado | sin llamadores tras el cambio | solo `corre/barrido.rs`, ya migrado |
| `es_r00t` | borrado | sin llamadores | solo dentro de `limpieza.rs` |
| `mismo_binario` | borrado | sin llamadores | solo dentro de `limpieza.rs` |
| `NOMBRE_R00T` | borrado | no aplica | solo dentro de `limpieza.rs` |
| `huerfanos` | firma: recibe `&[Contenedor]` | llamador en `barrer` | solo `limpieza.rs` y sus tests |
| `listar` | devuelve `Vec<Contenedor>` | llamadores `barrer` y `quitar_de_pid` | solo `limpieza.rs` |
| `inicio_de_proceso`, `btime_del_sistema`, `TICKS_POR_SEG`, `MARGEN_S` | borrados de `corre::barrido`; los dos primeros pasan a `limpieza.rs` y quedan privados | sin llamadores externos | sin llamadores en `src/` ni `tests/` |

## 7. Preguntas para el planner

1. **Un pid que existe sin `stat` legible cuenta como vivo en `corre`.** Antes, un cmdline que no era `r00t` bastaba para barrer. Ahora una corrida con el pid reusado por otro programa y `stat` ilegible conserva su credencial. Decisión: sigue la regla de la tarea, "sin hora, vivo". El caso es raro y el error va del lado de no borrar.
2. **El pid propio siempre vive.** `dueno_de` devuelve `Vivo` para `std::process::id()` sin mirar la hora. Decisión: se conserva el atajo, porque el error va del lado de no borrar.
3. **Un contenedor de la familia sin pid legible se sigue quitando.** Es el comportamiento anterior. Decisión: no cambia.
4. **El pid de otro usuario, sin permiso para leer `stat`, cuenta como vivo.** Decisión: sin hora, vivo.
