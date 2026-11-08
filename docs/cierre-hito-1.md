# Cierre del hito 1 de Root

**Estado: FIRMADO por el usuario el 2026-11-08, con la validez reprobada a la vista.**

## Veredicto del hito

**La validez automática reprobó.** El usuario decidió firmar con ese resultado a la vista (P73 d). Root no evalúa un repo real hasta cumplir el recall sobre el dataset.

| Criterio | Umbral | Medido | Resultado |
|---|---|---|---|
| Exactitud de orden | 1,0 | 5 de 6 = 0,833 | **No cumple** |
| Recall de defectos reales | 0,8 | 3 de 8 = 0,375 | **No cumple** |
| Recall por línea | Sin umbral | 0 de 8 | Sin umbral |
| Recall por test rojo | Sin umbral | 3 de 8 | Sin umbral |
| Acuerdo con el usuario | Hito 1b | Sin medir | Sin medir |

Medición del 2026-11-07 con `bfd5110`: 0,87 USD y 133 minutos. La medición del 2026-11-05 dio 1 de 8 con `e665a5f`, 0,97 USD y 199,7 minutos.

**La suba no es una mejora del juez.** P74 cambió la definición de "encontrado": un test oculto en rojo cuenta. Por línea, el juez bajó de 1 de 8 a 0 de 8.

## Lo que significa

- Root ordena bien los casos claros y mide bien lo que sus tests y sondas cubren.
- Root todavía no es "un evaluador con criterio" para defectos reales fuera de esas clases.
- Una calibración de 1,0 sobre defectos inyectados no prueba criterio. Los inyectores cubren las clases que las sondas buscan.

## Causas conocidas del recall

**Por código:**
- La métrica solo contaba hallazgos con archivo y línea. Los 3 defectos de test dieron rojo 5 de 5, y no contaron.
- El juez descarta un hallazgo sin requisito afectado.
- La evidencia de un ítem no cuenta como hallazgo.
- Ninguna sonda busca traversal, recursión ni tamaño de lectura.

**Por simulación:** 5 de 8 defectos no llegan al juez, porque lee los 5 archivos más largos y 6000 caracteres de cada uno.

**Medido el 2026-11-07:**
- Root halla por test los 3 defectos con test oculto: spin, FIFO y backend.
- Ningún hallazgo cae a 3 líneas o menos de un defecto. El más cercano queda a 5 líneas, en `src/renderer.rs:70`.
- `src/scene.rs:180` aparece en la evidencia de un ítem, y no como hallazgo.
- Falló el par `memora-sdlc-ba9d6e8` sobre `memora-sdlc-876f72a`: 0,399 contra 0,446. Ambos dan "bloqueado".
- La causa de ese par es Estructura: 0,0 contra 0,56. Sin esa dimensión, el orden queda correcto: 0,50 contra 0,42. La fórmula no cambia por este dato: ajustarla tras ver un par acomoda la medición.
- La diferencia de ese par, 0,047, cae dentro de la banda de variación de `global`. Igual cuenta como fallado.
- Los 3 tests estaban en `ocultos_fallidos`. Otra corrida dio FIFO como inestable, así que el 3 de 8 es frágil. Con la Tarea 78, un inestable no cuenta como rojo.

## Pasada de búsqueda de defectos (P73 c)

Medición única, con prompt fijo, sobre 10 defectos confirmados de Root `e665a5f`, en un recorte de 8 archivos. `deepseek-v4-pro`, 0,4665 USD.

| Medida | Resultado |
|---|---|
| Defectos encontrados | **0 de 10** |
| Cota superior de Wilson al 90 % | 0,14 |
| Hallazgos, según lectura de Opus | 6: 4 reales, 1 dudoso, 1 falso |
| Reales que la revisión de cierre no tenía | 2 |

La pasada leyó todo y no se cortó. No puntúa y no toca `global`. No mejora el recall oficial. No se midió sobre los 8 de la validez: no entra en el saldo. Según el umbral fijado antes de medir, S3b del dataset da "frena".

## Calibración por dimensión

Protocolo: `n = 20`, semilla 42. Solo entran a `global` las filas de 0,9 o más (P66 a).

| Dimensión | memora | brasa |
|---|---|---|
| Seguridad | 1,0 / 1,0 | 1,0 / 1,0 |
| Entrega | 1,0 / 1,0 | No aplica |
| Completitud | 1,0 / 1,0 | 1,0 / 1,0 |
| Estructura | 1,0 / 1,0 | 1,0 / 1,0 |
| Arquitectura | 1,0 / 1,0 | 1,0 / 1,0 |
| Eficiencia | No aplica | No aplica |
| Funciona | 0,9 / 1,0 | **Fuera:** 0,78 / 1,0, exactitud 0,889 |
| Pruebas | **Fuera:** 0,6 / 1,0, exactitud 0,80 | 1,0 / 1,0 |
| Diseño | **Fuera:** sin correr, 3 de 10 limpios sin veredicto | **Fuera:** 0,5 / 0,9, exactitud 0,70 |

**Límites de estas filas:**
- Con 9 o 10 defectos, un caso mueve 10 puntos. La cota de Wilson al 90 % de 8/10 es 0,60.
- En Completitud y Funciona, los tests ruidosos salen de los limpios. Una especificidad de 1,0 no prueba estabilidad.
- En Pruebas, los limpios reusan la mutación de la base. Su especificidad es 1,0 por construcción.
- La muestra de mutación mide 60 de 2263 mutantes en memora.
- El juez lee solo los primeros 6000 caracteres de cada archivo. Diseño detectó por cita 4 de 4 dentro del corte y 0 de 6 fuera.

## Corrida real de memora (Tarea 16)

**Cumple: `r00t corre` llega al juez final de punta a punta.** Binario `b1e8278`, exit 0, reanudada, 6 vueltas.

| Medida | Resultado |
|---|---|
| Contrato y oculto | 61/61 y 68/68, sin inestables |
| `global` | 0,785, sobre 6 dimensiones calibradas |
| Decisión | Listo |
| API | 0,0876 USD |

**Cómo leer esa fila:**
- **"Listo" significa "pasa lo que medimos".** No significa "sin defectos". El recall real sigue reprobado.
- **`invalida = true` es un falso positivo.** El builder corrió `ps`, y vio la ruta del host en sus propios argumentos. No leyó el oráculo.
- **Costo.** `usd_cupo` = 181,57 trae un corte con la fórmula vieja. El total de lista es cerca de 84 USD, por estimación.
- **Tiempo.** `minutos` = 16,95 cubre solo planifica y la última vuelta. La retoma duró 389 minutos de reloj.
- **`commit_root`** dice `85631b9`, y el binario es `b1e8278`.
- **Seguridad 0,556** sale de 2 llamadas `unsafe` normales. **Estructura 0,286** sale sobre todo de avisos de tamaño. Las dos pesan en `global`, y sus fórmulas van a revisión en el dataset.
- **No se compara.** La fila es reanudada, y salió de varios lanzamientos.

## Comparación de `global`

Sale de los veredictos de la validez repetida, en `validez/veredictos/`.

| Par | Mejor | Peor | Decisión | Resultado |
|---|---|---|---|---|
| memora-ecc contra memora-sdlc | 0,812 | 0,399 | Listo contra bloqueado | Correcto |
| brasa-ecc contra brasa-sdlc | 0,587 | 0,514 | Listo contra bloqueado | Correcto |

Los dos pares van en el orden esperado. Son 2 pares, sin intervalo de confianza. La distancia de brasa, 0,073, queda justo fuera de la banda de 0,07 medida abajo. Su decisión da más apoyo que su número.

## Revisión de cierre

5 HIGH y 17 MEDIUM, de revisores que solo leyeron. Opus verificó los HIGH y seis MEDIUM en el código. Las Tareas 69, 70 y 71 corrigen lo que entra antes de firmar. Sus mezclas son locales, sin push.

**Revisión del código nuevo, 2026-11-07.** 3 HIGH y 13 MEDIUM, de revisores que solo leyeron. Ninguno cambia un número de este cierre. La Tarea 78 corrige, antes de usar la Línea Buscador o repos ajenos:
- secretos de varias líneas, rutas de gitleaks y citas en `--solo-medir` (HIGH);
- medición de líneas no visibles, un candidato que cubre dos defectos, e inestables contados como rojos;
- lectura sin tope de `/salida`, y rutas con `..` hacia el juez.

La sospecha de doble conteo con `--resume` queda refutada por una muestra: la última vuelta informó 0,74 USD y 14 turnos.

## Validation

Punta probada: `8a9992c`, y después `9781451`. El árbol de código de `3108290` es igual a `9781451`.

| Comando | Resultado |
|---|---|
| `cargo fmt --check` y `cargo clippy` | Limpios en `8a9992c` |
| Suite con podman | `9781451`: 2754 pasan, 0 fallan, 4 ignorados; `builder_real` pasa |
| `r00t goal verificar` | Pasa: `311a6371c0cd414a` |
| `evaluar` memora-ecc | `global` 0,819, listo, contrato 61/61, oculto 65/68, 3 inestables |
| `evaluar` brasa-ecc | `global` 0,541, listo, contrato 53/53, oculto 20/21 |
| Calibración de 8 dimensiones | **No se repitió:** las filas son de binarios anteriores, por presupuesto. La Tarea 71 cambió los comandos de gitleaks, audit y clippy. No debería cambiar estos repos, sin medir |
| `informe` memora-sdlc | Bloqueado, con módulos ausentes, código 1 |
| `oraculo validar` | La medición de arriba |
| `corre` y `track listar` | La fila de arriba |
| `aprende --replay` | Sin datos: 1 fila, que además es inválida |

**"Niveles estables en 3 corridas": no cumple en memora.** Comparé `evaluar` del 2026-11-07, la validez y el humo del 2026-11-04, con binarios distintos.
- Entre las dos primeras, 14 de 16 niveles coinciden. R4 y R13 varían entre 2 y 3.
- El humo también da R7 en 1, y coincide en R4 y R13 con la corrida de hoy.
- La causa visible son 3 tests inestables: `test_bgsave_no_bloquea`, `test_blpop_fifo_entre_clientes` y `test_client_id`.
- Brasa coincide en 14 de 14 entre las dos corridas de hoy.

**`global` varía sobre el mismo commit.** Memora da 0,812 y 0,819. Brasa da 0,587 y 0,541.
- **La causa es el juez.** En brasa, Estructura pasa de 0,29 a 0,19 con los mismos 6 hallazgos, porque cambia `estructura.legibilidad`. Arquitectura pasa de 0,73 a 0,60.
- Los limpios guardados lo confirman, con 9 y 10 muestras. `global` varía por el juez en un rango de 0,069 en memora y 0,046 en brasa. Esto no incluye tests inestables.
- Diseño, sin calibrar, va de 0,82 a 0,0 en brasa. No entra a `global`.
- **Una diferencia de `global` menor a 0,07 no alcanza para ordenar dos repos.**
- El campo `commit` no es un commit: es un hash del contenido. Por eso difiere entre el clon y la carpeta del banco, que trae `.pytest_cache/`. Es una inferencia, sin ejecutar.

## Dinero

| Concepto | USD |
|---|---|
| API hasta el 2026-11-07, con validez y "Validation" | 23,83 |
| Límite útil del hito 1 | 24,00 |

Los builders se informan aparte en cada `docs/builds/*build-local-*.md`.

## Lo que queda fuera, con su motivo

- **Sin medir por falta de fuente:** los 7 defectos de F1, cuyos repos se borraron (decisión 59).
- **Hito 1b:** G3 y el acuerdo con el usuario, G4, Colab 7h, Graft 16b con G5, la lectura ampliada del juez y el contrato `sin_evidencia`.
- **Hito 1b, de la pasada:** `medicion` informa "0 de 10" aunque no lea nada, el tope frena con su reserva pesimista, y sale con 0 estando incompleta. Dos defectos reales que halló siguen en la punta: `src/corre/ejecutor.rs:339` y `src/corre/corrida.rs:281`.
- **Hito 1b, de la revisión nueva:** copias de gitleaks en disco, un `RUN` de más de 1 MiB, veredictos que se pisan, y los 5 MEDIUM de `corre`.
- **Hito 1b, de estabilidad:** causa de `test_bgsave_no_bloquea` y `test_client_id`.
- **Antes de la próxima calibración paga:** hallazgos 5, 18 y 19, y el arreglo de la Tarea 67.
- **Antes de evaluar un repo real:** el recall sobre el dataset, y los hallazgos 9, 11, 12, 14 y 15, sin verificar.
- **Dataset:** PRD en `.claude/prds/dataset.prd.md`, sujeto a su prueba de sustento (decisión 79).
- **Línea Buscador (P83):** busca un buscador con recall de 0,8 sobre defectos nuevos. No forma parte de este cierre, y nadie promete el resultado. Memora y brasa se borran "Cuando el dataset calibre" (decisión 70).

## Firma

El usuario respondió a P88 el 2026-11-08: "a) Firmo así".

La firma cubre este documento tal como queda. La Tarea 78 y el hito 1b siguen pendientes, y no cambian ningún número de arriba. Pregunta: P88 del plan.
