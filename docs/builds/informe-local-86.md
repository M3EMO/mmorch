# Informe de la Tarea 86 (builder local)

Rama `laptop/tarea-86`, base 3cd5898. Todo corrió con `ROOT_SIN_PODMAN=1`.

## 1. Resultado de la suite

- Base: la suite de la punta de la Tarea 85 no se corrió aparte. El total del binario de la biblioteca sube en 6 tests nuevos (5 del punto 1 y 1 del punto 3).
- Final: la biblioteca da 2368 verdes, 0 fallas, 0 ignorados. Los demás binarios dan 0 fallas.
- `cargo clippy --all-targets` del punto 1 no dio avisos.
- El hook de commit falló una vez por `rustfmt` en el punto 1. Corrí `cargo fmt` y el commit pasó. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La señal en la ventana entre el spawn y el registro no veía al hijo. | `senal_en_la_ventana_entre_spawn_y_registro_termina_el_hijo`, `spawn_pedido_despues_de_terminando_no_lanza_nada`, `lanzar_con_terminando_no_crea_copia_ni_lanza`, `quitar_builders_no_registra_su_pid_ni_pisa_el_del_hijo`, `registro_del_hijo_se_borra_al_recoger_el_wait` | 14eded8 |
| 2 | `ENTRADA_JUEZ_BYTES` no tenía usos. | Ninguno. La guarda es la suite verde. | 207e57d |
| 3 | cargo audit no tenía guarda con la herramienta real. | Ninguno. Necesita podman. | 35e95c0 |

Guardas del punto 1: los tests de señal de las Tareas 84 y 85 siguen verdes.

## 3. Qué falló o quedó incompleto

- Punto 3: no vi el test en rojo ni en verde. Sin podman, el test se saltea.
- Punto 3: no sé si el `cargo audit` real acepta la mini-base de RustSec que arma el test. Puede pedir otro `.git` o más campos. El orquestador lo ve al mezclar.
- Punto 1: queda una ventana mínima entre el `wait` y el borrado del pid. Un pid reciclado en esa ventana es casi imposible. Cerrarla exige tomar el cerrojo durante el `wait`.
- Punto 1: si `esperar` da error, el hijo no se mata. Ese comportamiento viene de antes. Ver pregunta 2.
- El registro sigue con un solo hueco. Dos `correr_proceso` concurrentes se pisan. Hoy no ocurre en producción.
- Queda `.codegraph/` sin seguimiento en el worktree. No entró a ningún commit.
- Los defectos MEDIUM 3, 4 y 5 de la revisión (error de borrado tragado, deriva de `btime`, barrido que aborta) no estaban en el brief. No se tocaron.

## 4. Tests que cambiaron de valor

- Siete tests viejos de `ejecutor.rs` toman ahora `serie_de_pruebas()`: cinco de `lanzar` del ejecutor real y dos de `correr_proceso_con_tope`. Sin el cerrojo, un test de señal en paralelo dejaba `terminando` en true y los hacía fallar.
- `serie_de_pruebas()` devuelve un guardia propio. Al tomarlo y al soltarlo borra `terminando` y las costuras.

## 5. Llamadores revisados

- `correr_proceso`: la firma no cambió. Llamadores de codegraph y de grep: `lanzar`, `correr_paso_de_quitar`, los tests de `ejecutor.rs` y `corrida.rs:3272`. Ninguno requirió cambio. Por dentro llama a `correr_hijo(..., registrar)`.
- `HIJO_EN_CURSO` pasó a `ESTADO_HIJO`. Los usos de `pid_en_curso()` siguen iguales.
- `ENTRADA_JUEZ_BYTES`: grep en `src/`, `tests/` y `docs/` halló la definición y dos notas históricas en `docs/builds/`. Codegraph halló 0 llamadores. Se borró. También se quitó el `use` de `MAX_ENTRADA_JUEZ_BYTES`, que quedó sin uso.

## 6. Guarda de cargo audit

- Aviso: RUSTSEC-2020-0071, crate `time` 0.1.43. Es el mismo aviso del fixture `tests/fixtures/sondas/cargo-audit.json`.
- Fuente: el test escribe una mini-base de RustSec con ese aviso. No salió de la base real de la imagen, porque esta tarea no toca `~/.local/share/root/`. El orquestador puede cambiar `AVISO_TIME` por el original.
- Categoría del aviso: el test usa `code-execution`. El fixture dice `memory-corruption`. No afecta el resultado.
- Comando para ver el rojo con podman: en una copia local, quitar `--exclude=./.cargo/audit.toml` de `COMANDO_CARGO_AUDIT` (`src/oracle/perfiles/herramientas.rs:423`). Después correr:

```
env -u ROOT_SIN_PODMAN tools/test-seguro.sh guarda_cargo_audit_ignora_el_audit_toml_del_repo
```

## 7. Preguntas para el planner

1. ¿Se cierra la ventana entre el `wait` y el borrado del pid? Decisión: no. El brief pide que el cerrojo cubra solo el spawn y el registro.
2. ¿Un error de `esperar` debe matar al hijo? Decisión: no se cambió. Hoy el hijo queda vivo con la credencial en ese caso. Conviene un punto propio.
3. ¿El registro del hijo debe admitir varios hijos a la vez? Decisión: no. Hoy no hay concurrencia en producción.
