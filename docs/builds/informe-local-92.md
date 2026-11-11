# Informe de la Tarea 92 (builder local)

Base: 9b47e65. Rama: `laptop/tarea-92`. Sin push.

## 1. Resultado de la suite

- Base, con `ROOT_SIN_PODMAN=1`, solo la biblioteca: 2490 verdes, 2 fallas, 0 ignorados.
- Final, con `ROOT_SIN_PODMAN=1`, `cargo test --no-fail-fast`: 3070 verdes, 2 fallas, 4 ignorados.
- Las 2 fallas son las mismas de la base: `mcp::tests::mcp_unit_humano_por_mcp_lleva_origen_mcp` y `mcp::tests::mcp_unit_plan_aprobado_por_mcp_lleva_origen_mcp`. No tocan esta tarea.
- No medí la suite completa de la base. Solo medí la biblioteca.
- Con podman: `cargo test --test entrega` da 49 verdes, y `cargo test --lib entrega` da 161 verdes.
- `cargo clippy --lib --tests -- -D warnings` y `cargo fmt` quedan limpios.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `sh` y `timeout` del envoltorio se resolvían con el PATH activado | `envoltorio_con_activacion_usa_rutas_absolutas_de_sh_y_timeout`, `sesion_iniciar_rechaza_sh_bajo_tmp`, `ruta_de_sistema_valida_tabla`, `sesion_iniciar_timeout_bajo_tmp_se_ignora` (rojo por error de compilación) | d67536b |
| 2 | La sonda de red quedaba fija por proceso y no distinguía "no corrió" | `build_con_control_que_falla_da_infraestructura_no_pendiente`, `build_con_control_verde_y_prueba_roja_queda_pendiente_con_su_motivo`, `sonda_que_fallo_una_vez_y_pasa_la_segunda_da_hay_red`, `hay_red_de_una_imagen_no_vale_para_otra`, `sonda_de_red_nohay_y_nocorrio_no_se_guardan` (rojo por error de compilación) | 81fda80 |

Guardas: los tests de la 91c del orden del PATH, los de clase B de la 91b y los de `oracle/mod.rs` con `responde_build_con_sonda`. Todos siguen verdes sin cambios de intención.

## 3. Qué falló o quedó incompleto

- Los rojos son errores de compilación (tipos y campos nuevos). No hay rojos por comportamiento.
- El test con podman del `sh` falso no prueba el defecto con activación. No hay imagen local con Python. Sin activación el PATH del sistema ya va primero. El test queda como guarda con alpine y lenguaje `shell`. El unitario `envoltorio_con_activacion_usa_rutas_absolutas_de_sh_y_timeout` cubre el defecto.
- El `sh` externo de `podman exec` no usa ruta absoluta. El brief pide solo el `sh` interno y `timeout`.
- Los tests nuevos del punto 2 crean su carpeta bajo `CARGO_MANIFEST_DIR`, no en `/tmp`.
- El helper `es_control_de_red` distingue el control de `tiene_sh` por el prefijo `root-entrega-` de la etiqueta. Si cambia `etiqueta_unica`, hay que ajustarlo.

## 4. Tests que cambiaron de valor

- `ENVOLTORIO_SIN_ACTIVAR`, `ENVOLTORIO_CON_TIMEOUT` y dos tests de `paso_args`: esperan `/usr/bin/sh` y `/usr/bin/timeout` por ruta.
- `sesion_iniciar_detecta_sh_y_timeout_por_el_ejecutor`: la sesión hace una sola llamada de sonda en vez de dos.
- `ExecFalso` responde la sonda `command -v` con `sh=/usr/bin/sh` y `timeout=/usr/bin/timeout`. Sin esto, 22 tests de evaluación fallaban por "la imagen no trae sh".
- `sesion_de_prueba` y `codigo_del_envoltorio` arman las rutas nuevas. `codigo_del_envoltorio` usa `/bin/sh` del host.
- Punto 2: ningún test existente cambió de valor.

## 5. Llamadores revisados

- `red_disponible` (ahora devuelve `Red`): codegraph da `construir_con`. grep -w da la misma llamada, en `entrega.rs:622`. Ningún otro llamador.
- `envoltorio_de` (nueva firma): grep -w no halla uso fuera de `entrega.rs`. Su único llamador es `Sesion::paso_args`.
- `Sesion.tiene_timeout` (borrado): grep -w no halla uso fuera de `entrega.rs`. Los tests se actualizaron.
- `sonda_de_red` pasó a devolver `Red`. Sus llamadores son `PodmanReal::red_disponible` y el método por defecto del trait.

## 6. Preguntas para el planner

1. ¿El `sh` externo de `podman exec` debe usar ruta absoluta? Decisión: no. El brief pide solo el interno.
2. `ruta_de_sistema_valida` rechaza también rutas con `..`, y trata `/repo` como carpeta de trabajo. Decisión: sí, porque cierra un rodeo evidente.
3. La sonda de rutas corre con el PATH del sistema fijo, no con el de la imagen. Decisión: sí, para no depender del `ENV PATH` del Dockerfile.
4. La causa de infraestructura de la sonda va en `sin_veredicto` con `imagen: None` y la línea del `RUN`. Decisión: esa forma, igual a las demás causas.
5. La memoria de "hay red" vive por proceso y por imagen. Decisión: sí, como pide el brief.
6. ¿Hace falta una imagen local con Python para probar con podman el defecto con activación? Decisión: no la creé.
