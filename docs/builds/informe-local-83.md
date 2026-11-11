# Informe de la Tarea 83

Rama `laptop/tarea-83`, base 5aa2dc6 (punta de `laptop/tarea-82`). Todo corrió con `ROOT_SIN_PODMAN=1`.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 5aa2dc6 | 2859 | 0 | 5 |
| Final 260b19b | 2884 | 0 | 4 |

- `cargo clippy --all-targets` termina sin avisos.
- Un ignorado menos: se borró `calibra_cota_de_t1_de_s2_sin_api`, que imprimía la cota vieja.
- Ningún test de tiempo falló en el hook. No hubo reintentos de commit por carga.
- Un commit falló una vez por `cargo fmt` (el hook de formato), no por tiempo. Se formateó y se repitió.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La cota por caso (3,25 USD en Diseño) impedía calibrar con tope | Aceptación 0,285 / 0,07 / 0,30 de `reservar`; dos hilos con lugar para uno; reintento del juez; cierre de reserva; juez rechazado deja detención. Todos en rojo por compilación (API inexistente) | d14681c |
| 2 | Un ítem del juez inestable daba falsas alarmas y cobertura al azar | 4 tests de `inestables.rs`, 3 de `calibra.rs` (aceptación 0/2, dimensión inestable, limpio que baja 2) y 4 de `evaluar` e informe | 2ffee42 |
| 3 | Los casos no guardaban los extractos del juez | 5 tests, en rojo por compilación (faltaban los campos) | ac24b6d |
| 4 | Una línea más larga que `extracto_max` se omitía en silencio | 4 tests de marca (línea 7000, total, ventana de cambio, archivo que empieza con línea larga) | 260b19b |

Guardas (verdes desde el inicio): sin límite todo igual, modelo sin precio da error, dimensión solo de sondas igual que hoy, ítem 3-2-3 estable, filas, puntajes, veredictos y casos viejos sin el campo nuevo leen vacío, ventanas de las Tareas 81 y 82.

Orden de commits: el punto 4 se construyó en un worktree aparte y entró con `cherry-pick` al final. El punto 3 se empezó en otro worktree y se terminó en esta rama después del punto 2.

## 3. Qué falló o quedó incompleto

- **Sin test de integración de `validar_precios_de_home`.** El `Config` fusiona precios por defecto y no deja quitar un precio desde `root.toml`. Hay test unitario de `validar_precios_de_jueces`.
- **Sin test del límite que rechaza una repetición de la base.** La ruta usa el mismo `con_limite` que la base y los casos.
- **El juez de config no guarda extractos.** Su camino no usa `ResultadoJuez`. Cambiarlo toca `descarga/juez_config.rs`, fuera de la lista.
- **Las dimensiones con juez juzgan la base 3 veces** aunque el juez no corra en esa tarea. Gasta llamadas de más.
- **La mezcla y la calibración real no se probaron.** Las corre el orquestador.

## 4. Tests que cambiaron de valor

- Con 4 hilos y tope, qué casos entran varía entre corridas. `calibra_tope_incompleta_con_la_misma_semilla_se_repite` ahora verifica solo el conteo (base y 2 casos).
- Dos tests de la detención en `tests/calibra.rs` cuentan juicios reales contra el servidor falso, no el medidor `gasto()`.
- `calibra_el_tope_se_revisa_antes_de_cada_reemplazo` simula el rechazo con `anota`.
- `calibra_tope_nombra_el_archivo...` pasó a `calibra_tope_sin_casos_medidos_nombra_ninguno_en_el_error`.
- Siete tests de reuso de limpios en Diseño: los helpers `evaluador_con_limpios` y `evaluador_real` ya no cuentan las repeticiones de base como limpios. Los valores esperados no cambiaron.
- Dos tests de Diseño y de limpios corrieron su corte de `llamadas[1..]` en 2, porque la base suma 2 juicios.
- Borrados por quedar sin objeto: `calibra_tope_sin_precio_del_juez_no_arranca` (integración), `calibra_peor_caso_*` (unitarios y de integración), `calibra_el_tope_no_se_pasa_con_cuatro_llamadas_en_paralelo`, los dos de `gasto_comprometido` y `calibra_cuatro_hilos_los_casos_con_reserva_no_suman_dos_veces`. La garantía atómica la cubre la prueba de dos hilos de `proveedores`.
- Cuatro llamadas de test a `puntaje_diseno` cambiaron de firma.

## 5. Funciones borradas

Todas en `src/oracle/calibra.rs`. Se revisaron con `codegraph callers` y `grep -w`. Coinciden y no quedan llamadores.

- `peor_caso_de_caso`, `peor_caso_de_caso_con_entrada`, `peor_caso_con_jueces`, `peor_caso_con_jueces_entrada`.
- `peor_caso_de_home`, `peor_caso_de_home_con_entrada`, `peor_caso_de_home_con_tarea`.
- `llamadas_del_caso`, `LlamadaJuez`, `precio_de_eleccion`, `detencion_si_no_entra`, `gasto_comprometido`.
- Constantes `PEDIDOS_POR_JUICIO_CONFIG` y `PEDIDOS_POR_JUICIO_DISENO`.
- Campos `Tope.peor_caso`, `Tope.casos_con_reserva` y `Presupuesto.peor_caso`.

Quedan sin borrar:
- `cota_bytes_de_prompt` (`juez.rs`): sin llamadores de producción, solo 4 tests. Candidata a borrar.
- `casos_con_reserva_abierta` y `casos_con_reserva_abierta_en` (`observe/uso.rs`): son `pub` y no dan aviso. Sin uso en producción. Candidatas a borrar.

## 6. Cota de `auditar`

No se cambió. En `src/oracle/auditar.rs:1148` la cota de un prompt es `INTENTOS_MAX` por `costo_peor_caso(precio, bytes_de_prompt(p), p.max_tokens)`. Se calcula por prompt, antes de cada envío. Compara `gasto() + peor_caso(&prompt)` con su propio `--tope-usd`. No usa lo borrado.

## 7. Llamadores revisados

| Firma o símbolo | Llamadores |
|---|---|
| `Presupuesto` (sin `peor_caso`) | 10 usos, en `lib.rs`, `calibra.rs` y `tests/calibra.rs`. Codegraph y grep coinciden |
| `reservar` (revisa el límite) | Un llamador, `reintentar`. Coinciden |
| `modelos_sin_precio` | Misma firma. Coinciden |
| `calibrar_medida_con_hilos` | 10 llamadores, firma sin cambio público. Coinciden |
| `finalizar_casos` | 19 llamadores, firma sin cambio. Coinciden |
| `TipoCaso` (variante `BaseRepetida`) y `reuso_de` | Un `match` en `reuso_de`. Coinciden |
| `medir`, `parte_juez`, `puntaje_diseno`, `aplicar_cobertura`, `peso_total` | Privadas en `oracle/mod.rs`. Se revisaron con `grep -w` en `src` y `tests`. Sin índice de codegraph en esos pasos |
| `numerar_rango`, `partir`, `largo_filas`, `ventanas` | Privadas en `juez.rs`. Ningún test las llamaba directo |

## 8. Preguntas para el planner

1. **Archivos fuera de la lista.** El punto 3 necesitó `Veredicto.extractos` (`tipos.rs`, `oracle/mod.rs`), que el brief solo permite al punto 2. También se tocó `informe.rs` (render de ítems inestables), `ExtractoGuardado` (derive `JsonSchema` en `juez.rs`) y literales en `perfiles/mod.rs`, `requisitos.rs`, `validez.rs` y `calibra/*.rs`. Decisión: se tocaron, porque son campos nuevos sin lógica. El brief decía que Veredicto ya guardaba los extractos, y no era así.
2. **Dimensión inestable.** Decisión: `calibrar` no mide casos. Devuelve una fila con `n = 0`, `inestables` y `no_aplica = "inestable en esta tarea: ..."`. Ahorra plata. Usa `no_aplica` aunque su sentido original sea otro.
3. **Gasto de la base.** Con juez, la base cuesta 3 juicios aunque la tarea no lo use. ¿Se salta la repetición si el juez no corre?
4. **Dónde van los extractos de la base.** Decisión: `Fila.extractos_base`, una lista por juicio. No hay renglón de base en `.casos.jsonl`.
5. **Defecto sobre un ítem inestable.** Decisión: sigue contando en la sensibilidad de la dimensión. Solo salen las métricas por ítem.
6. **`evaluar` y calibración.** Decisión: una medición de calibración no excluye ítems, porque leería su propia fila vieja. Un ítem inestable nunca se recuperaría.
7. **Huella.** `evaluar` usa la última fila sin mirar la huella. Si cambió el prompt del juez, la lista vieja sigue excluyendo. ¿Se exige huella vigente?
8. **Hallazgos del juez sobre ítems excluidos** siguen en el veredicto. No se filtraron.
9. **Rechazo por límite por hilo.** Decisión: un caso se descarta solo si su propio juez fue rechazado. El brief sugería mirar el rechazo global, que descartaba casos ya pagados.
10. **`Presupuesto.gasto_desde`** quedó sin lectura en `calibrar_tarea_con`. El gasto de la detención sale del rechazo. ¿Se quita?
11. **Validación de precios.** Vive en `calibrar_tarea_con`, no en `lib.rs`, porque necesita las partes. Mismo mensaje y mismo código 2.
12. **Marca de línea larga.** Decisión: el texto es `N: [línea N omitida por largo]`. El umbral es `extracto_max`, no el presupuesto restante. Un secreto largo conserva la marca de secreto.
13. **Limpios compartidos.** Los limpios compartidos entre Estructura, Arquitectura y Diseño guardan el veredicto del mismo juez, así que los extractos coinciden. Se aceptó.
14. **`docs/interfaces.md`** se regeneró en los commits de los puntos 2 y 3, porque el hook corre el snapshot. No quedó un paso aparte al final.
