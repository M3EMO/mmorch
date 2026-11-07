# Cierre del hito 1 de Root

**Estado: BORRADOR, sin firmar.** Faltan la repetición de la validez y los comandos de "Validation".

## Veredicto del hito

**La validez automática reprobó.** El usuario decidió firmar con ese resultado a la vista (P73 d). Root no evalúa un repo real hasta cumplir el recall sobre el dataset.

| Criterio | Umbral | Medido | Resultado |
|---|---|---|---|
| Exactitud de orden | 1,0 | 5 de 6 = 0,833 | **No cumple** |
| Recall de defectos reales | 0,8 | 1 de 8 = 0,125 | **No cumple** |
| Acuerdo con el usuario | Hito 1b | Sin medir | Sin medir |

Medición del 2026-11-05 con `e665a5f`: 0,97 USD y 199,7 minutos. Pendiente: la repetición con el detalle por caso. P74 cambia la definición de "encontrado": un test oculto en rojo cuenta, y la fila separa línea y test.

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

**Sin medir:** qué par falló y qué defecto se encontró.

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

**Pendiente.** Sale de los veredictos que guarda la validez repetida.

## Revisión de cierre

5 HIGH y 17 MEDIUM, de revisores que solo leyeron. Opus verificó los HIGH y seis MEDIUM en el código. Las Tareas 69, 70 y 71 corrigen lo que entra antes de firmar. Sus mezclas son locales, sin push.

## Validation

**Pendiente de correr:** cada comando de la sección "Validation" del plan, con su resultado.

## Dinero

| Concepto | USD |
|---|---|
| API hasta el 2026-11-06, con la pasada y la corrida | 22,62 |
| Límite útil del hito 1 | 24,00 |

Los builders se informan aparte en cada `docs/builds/*build-local-*.md`.

## Lo que queda fuera, con su motivo

- **Sin medir por falta de fuente:** los 7 defectos de F1, cuyos repos se borraron (decisión 59).
- **Hito 1b:** G3 y el acuerdo con el usuario, G4, Colab 7h, Graft 16b con G5, la lectura ampliada del juez y el contrato `sin_evidencia`.
- **Hito 1b, de la pasada:** `medicion` informa "0 de 10" aunque no lea nada, el tope frena con su reserva pesimista, y sale con 0 estando incompleta. Dos defectos reales que halló siguen en la punta: `src/corre/ejecutor.rs:339` y `src/corre/corrida.rs:281`.
- **Antes de la próxima calibración paga:** hallazgos 5, 18 y 19, y el arreglo de la Tarea 67.
- **Antes de evaluar un repo real:** el recall sobre el dataset, y los hallazgos 9, 11, 12, 14 y 15, sin verificar.
- **Dataset:** PRD en `.claude/prds/dataset.prd.md`, sujeto a su prueba de sustento (decisión 79).
- **Línea Buscador (P83):** busca un buscador con recall de 0,8 sobre defectos nuevos. No forma parte de este cierre, y nadie promete el resultado. Memora y brasa se borran "Cuando el dataset calibre" (decisión 70).

## Firma

Sin firmar. La firma es del usuario.
