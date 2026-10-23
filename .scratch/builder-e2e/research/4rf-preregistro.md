# Pre-registro de orchestration-4rf: notas de campo de corridas previas en el SDLC

Estado: borrador. Este diseño se congela antes de la primera corrida de un brazo.
Fecha: 2026-10-13. No se corrió nada.
Issue: bd `orchestration-4rf`. Mapa relacionado: `.scratch/builder-e2e/map.md`.

## 0. Decisiones del usuario (2026-10-13)

- La métrica primaria es la fricción por corrida: rechazos de gates más escalones del fix ladder.
- El éxito por ítem es una métrica secundaria informativa.
- El lote corre en el escritorio, donde vive el arnés de `orchestration-34k`.

## 0.1 Cambios de la revisión adversarial (2026-10-14, antes de congelar)

`deepseek-reasoner` revisó el borrador por `/verify-cross` y lo refutó con 6 hallazgos (US$0.004). El ensemble no corrió porque un proveedor devolvió saldo insuficiente (código 1113). Los cambios son estos:

- Fuga (§5): una corrida previa entra solo si su commit resultante es ancestro del commit base del ítem. Las ramas de revisión sin merge quedan afuera. El validador de §7.5 suma un chequeo de archivos nuevos del ítem.
- Asimetría (§6): cada repetición corre en un worktree nuevo, desde el commit base, con un PHASE único. El código reusa una rama `sdlc/<PHASE>` existente (`sdlc/pipeline.py:1774`), y eso contaminaría las repeticiones. El orden de los brazos se sortea con semilla.
- Métrica (§8.1): "corrida fallida" queda definida. Los gates son código determinista y el coder no puede saltearlos. Los gates posteriores al verde corren solo con aceptación verde (`sdlc/pipeline.py:1585`), y la imputación cubre las corridas sin verde.
- Regla (§9): un solo estimando, la media de `d_i`. El bootstrap remuestrea ítems. Los márgenes quedan en unidades explícitas. La no inferioridad dice "a lo sumo un ítem".
- Confusores (§11): se suman el orden y el caché de la API. La temperatura ya vale 0 en `llm()` (`sdlc/pipeline.py:154`); la API no acepta semilla.
- Fallas técnicas (§8.3): se suma un análisis de sensibilidad que las cuenta como fallas del brazo.

## 1. Pregunta

¿El coder del SDLC falla menos si recibe notas de campo de corridas previas sobre el mismo repo?

Una nota de campo copia el formato de universal-modder. Cada corrida deja una nota. La corrida siguiente sobre el mismo repo lee las notas anteriores.

## 2. Hipótesis

- H1: las notas bajan la fricción por corrida en al menos 25%.
- H0: las notas no cambian la fricción, o la suben.
- Riesgo: las notas agregan contexto que distrae al coder y bajan el éxito.

## 3. Hechos verificados en el código (commit `40a0868`)

- Las 11 llamadas con rol de coder pasan por `_rol()` (`sdlc/pipeline.py:330`): `llm(CODER, _rol(...) ...)`.
- Hay una excepción en la línea 161. Si el reasoner agota su presupuesto, `llm()` reintenta con el modelo del coder y el prompt del reasoner. Ese reintento no recibe notas en ningún brazo.
- Las llamadas del diagnosticador (`llm(DIAG, ...)`, líneas 1075 y 1178) no pasan por `_rol()`.
- El planner (`_planner`, línea 185) no lee memoria ni corridas previas.
- `rec_gate` (línea 243) agrega cada rechazo a `state["gate_rejects"]`.
- El estado guarda `compile_ladder` (línea 1202), `reasoner_rounds` (línea 1098), `test_rounds` (línea 1531) y `escalated_to_claude` (líneas 1118 y 1144).
- `_flush` escribe el estado completo en `run-log.json`. Con worktree, la ruta es `<wt>/docs/sdlc/run-log.json` (línea 133).
- Ningún archivo de `sdlc/pipeline.py` llama a `recall`, `remember` ni lee run-logs de otra corrida.

## 3.1 Prueba de humo del generador (2026-10-14)

- `scripts/notas_campo.py` corrió sobre 3 run-logs reales de ChatBot con `deepseek-chat`. Un ítem quedó en la primaria con 2 corridas elegibles.
- Las notas siguen la plantilla y marcan "desconocido" donde el log no muestra la causa.
- El modelo repetía las reglas al final de la nota. El prompt suma "Devolvé solo la nota." antes de congelar.
- Los run-logs de SSB no están en la laptop: el historial git de `~/Proyectos/ssb` no tiene ningún `docs/sdlc/run-log.json`. Probablemente viven en `logs/sdlc/` del escritorio.

## 4. Hechos a verificar en el escritorio antes de congelar

- El arnés `orch-spike/scripts/sdlc_planner` existe en el commit `fc933ae` y reproduce las 12 corridas de `34k`.
- El arnés reemplaza la escalada a Claude por un comando vacío, así que el lote no gasta cupo.
- Cada ítem tiene su run-log histórico con fecha, en el historial git de su repo (`docs/sdlc/run-log.json`).
- La versión de mmorch del lote queda fija en un commit y se registra en cada fila.
- Los run-logs de SSB están commiteados en `docs/sdlc/run-log.json` de algún commit. Si viven solo en `logs/sdlc/`, `scripts/notas_campo.py` necesita leerlos desde archivo antes de generar las notas.

Si un hecho falla, este pre-registro se corrige y se vuelve a revisar antes de congelar.

## 5. Ítems

- Base: las 12 corridas repetibles de `34k` (9 de SSB, 1 de Estudio y 2 de ChatBot).
- Commit base del ítem: el `base_sha` de su run-log histórico (`sdlc/pipeline.py:1791`).
- Regla de elegibilidad: una corrida histórica `j` del mismo repo da nota al ítem `i` solo si se cumplen dos condiciones.
  - La corrida `j` empezó antes que la corrida histórica de `i`.
  - El commit resultante de `j` es ancestro del commit base de `i` (`git merge-base --is-ancestor`).
- Un ítem sin ninguna corrida elegible queda afuera de la primaria. Se informa aparte.
- Estudio queda afuera porque no tiene otra corrida en su repo.
- Resultado esperado: unos 9 ítems en la primaria (8 de SSB y 1 de ChatBot). El número exacto sale del paso 4.

La regla de elegibilidad evita fugas. Una corrida posterior, o una rama sin merge, puede describir código que no existe en el commit base del ítem.

## 6. Brazos

- **N0 (en frío).** El SDLC actual sin cambios. Es el control.
- **N1 (con notas).** El mismo SDLC, y `_rol()` agrega el bloque de notas del ítem al final del prompt de sistema del coder.

Los dos brazos usan el mismo planner (`deepseek-reasoner`), el mismo coder (`deepseek-v4-pro`), el mismo test de aceptación y la misma versión de mmorch.
Cada ítem corre 3 repeticiones por brazo.

- Cada corrida usa un worktree nuevo desde el commit base del ítem.
- Cada corrida usa un PHASE único: `4rf-<item>-<brazo>-<rep>`. Así `sdlc/<PHASE>` nunca existe de antes.
- El arnés registra el commit base y el commit final en cada fila.
- El orden dentro de cada par de repeticiones (N0 y N1 con el mismo número) se sortea con semilla 20261013.
- Los pares corren intercalados: el lote nunca corre todo N0 antes que N1.

## 7. Notas de campo

### 7.1 Fuente

- Una nota por corrida histórica. La fuente es solo su `run-log.json`.
- La nota nunca lee el diff, el código ni los tests de la corrida.

### 7.2 Generación

- Un modelo escribe cada nota una sola vez, antes del lote: `deepseek-chat`, temperatura 0.
- El prompt de generación queda congelado en la sección 7.4.
- Las notas quedan en `notas.json` con SHA-256, congeladas antes de correr N1.

### 7.3 Plantilla de la nota

```
Tarea: <una línea>
Stack y toolchain: <lenguaje, comando de compilación, comando de tests>
Archivos tocados: <lista de plan_files>
Problemas (síntoma → causa → arreglo):
- <síntoma tal como lo reportó el gate> → <causa probable> → <qué lo destrabó, o "no se destrabó">
Cómo se verificó: <aceptación y suite>
```

### 7.4 Prompt de generación (congelado)

```
Abajo va el run-log JSON de una corrida del pipeline SDLC sobre un repo.
Escribí una nota de campo para la próxima corrida sobre el MISMO repo, con esta plantilla exacta:
{plantilla}
Reglas: máximo 200 palabras; nada de código; nombrá archivos, comandos y gates tal como aparecen en el log;
si el log no muestra la causa o el arreglo, escribí "desconocido". Devolvé solo la nota.
RUN-LOG:
{run_log}
```

### 7.5 Bloque que recibe el coder en N1

```
Notas de campo de corridas anteriores sobre este repo (de la más reciente a la más vieja):
<<<
{notas}
>>>
Usalas si aplican. Pueden no aplicar a este archivo.
```

- Tope: 1500 tokens, estimados como 4 caracteres por token. Si las notas superan el tope, entran las más recientes.
- El run-log entra al prompt de generación truncado a 40 000 caracteres de JSON.
- Implementación: `scripts/notas_campo.py` (tests en `tests/test_notas_campo.py`). Lectura en el coder: `_notas()` en `sdlc/pipeline.py`, activada por la variable `SDLC_NOTAS`.
- Un validador aborta en dos casos, antes de congelar `notas.json`:
  - El bloque contiene el nombre del archivo del test de aceptación del ítem.
  - El bloque nombra un archivo que el ítem agrega entre su commit base y su commit histórico final (incluye su test de aceptación).

## 8. Métricas

### 8.1 Primaria: fricción por corrida

`F = len(gate_rejects) + len(compile_ladder) + len(reasoner_rounds) + escalated_to_claude`

- Una corrida fallida es una corrida cuya aceptación no queda verde sin escalar a Claude. Incluye las que cortan en cualquier etapa.
- Una corrida fallida corta antes y acumula menos fricción. Para no premiar esa falla, su F se reemplaza por la F máxima observada en ese ítem en cualquier brazo y repetición.
- Los gates son código determinista: el coder no puede saltear un gate. Los gates posteriores al verde corren solo si la aceptación queda verde (`sdlc/pipeline.py:1585`).
- El análisis informa además la F por etapa alcanzada.
- La F de un ítem es la media de sus 3 repeticiones.

### 8.2 Secundarias

- Éxito por ítem: la aceptación pasa sin escalar a Claude.
- F cruda, sin el reemplazo de 8.1.
- `len(test_rounds)`, USD de DeepSeek, minutos por corrida y tokens del prompt del coder.
- Fricción por gate, para ver qué gates cambian.

### 8.3 Fallas técnicas

Una falla del arnés, como un error de git o de red, no cuenta como falla del brazo. La corrida se repite una vez. Si falla de nuevo, el ítem sale de los dos brazos y se informa.

Análisis de sensibilidad: el script repite la regla de §9 contando cada falla técnica como corrida fallida de su brazo. Si los dos análisis dan veredictos distintos, el veredicto final es el más conservador.

## 9. Regla de decisión (congelada)

Sea `d_i = F_N0(i) − F_N1(i)` por ítem, en unidades de fricción. El estimando único es `D = media de d_i`. La línea base es `B = media de F_N0(i)`.

- Bootstrap: 10 000 remuestreos de los ítems con reposición, semilla 20261013. Cada remuestreo recalcula `D`.
- `L` es el percentil 5 de esos `D`: la cota inferior al 90%.
- Éxitos: `S_N0` y `S_N1` cuentan los ítems con al menos 2 de 3 repeticiones exitosas.

N1 se adopta si se cumplen las tres condiciones:

1. `D / B ≥ 0.25`, o sea, una baja de al menos 25% de la fricción media.
2. `L > 0`.
3. `S_N1 ≥ S_N0 − 1`: N1 pierde a lo sumo un ítem exitoso.

Veredictos posibles:

- ADOPTAR: se cumplen 1, 2 y 3.
- PROMETEDOR: se cumplen 1 y 3, y `L > −0.10 × B`. Abre un banco ampliado con más ítems y un brazo placebo.
- NO ADOPTAR: cualquier otro caso.

Un script congelado aplica la regla. Nadie decide mirando los números.

## 10. Potencia y costo

- Con unos 9 ítems y 3 repeticiones, el banco es un piloto. No detecta efectos chicos.
- Corridas: 9 ítems × 3 repeticiones × 2 brazos = 54, más los ítems informados aparte.
- Costo estimado con las cifras de `34k`: unos US$0.22 de DeepSeek por corrida, unos US$13 en total.
- Tiempo estimado: mediana de 9 minutos por corrida, unas 8 horas en serie. Corre de noche en el escritorio.
- Cupo de Claude: cero, porque el arnés anula la escalada.

## 11. Amenazas conocidas

- Largo del contexto: N1 recibe más texto que N0. El banco no separa el efecto del contenido del efecto del largo. Un brazo placebo con texto neutro del mismo largo queda para el banco ampliado.
- Calidad de la nota: si `deepseek-chat` escribe notas pobres, el banco mide esas notas y no la idea. El análisis informa cuántas notas dicen "desconocido".
- Pocos ítems de ChatBot: la primaria queda dominada por SSB.
- No determinismo: la temperatura vale 0 en `llm()` (`sdlc/pipeline.py:154`), pero la API de DeepSeek no acepta semilla. Las 3 repeticiones existen por eso.
- Orden y caché: el caché de contexto de DeepSeek puede cambiar el costo entre corridas. El sorteo intercalado de §6 reparte ese efecto entre brazos. El análisis informa la F por posición en el orden.
- Run-logs sin ruta del repo: `state` guarda task, phase y `base_sha`, pero no el repo. El arnés asocia cada run-log a su repo por la ruta donde lo encuentra.
- Cambio de código: N1 necesita que `_rol()` lea el bloque de notas desde una variable de entorno. Sin esa variable, `_rol()` queda igual. El cambio lleva un test.

## 12. Desvíos

Todo desvío se declara antes de ver el resultado del brazo afectado, con fecha y motivo, en esta sección.
