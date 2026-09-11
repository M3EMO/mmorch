# Protocolo de comparacion
Type: grilling
Status: resolved
Blocked by: 06
Map: ../map.md

## Question

Fijar el protocolo de medición para las features del ticket 06: métricas por feature (aceptación verde, USD por fase del ledger, minutos de pared, intervenciones por nivel, falsos rechazos de gates), brazos (pipeline v3 vs engine de 1 etapa vs híbrido), cuántas repeticiones, y el criterio de 'gana'. Cómo se reporta: tabla en README 'Measured' + nota de vault. Regla del mapa: si no gana, se dice.

## Decisions (grilling 2026-09-11, en curso)

- Brazos: (a) v3 puro, 0 Claude, solo como control barato; (b) v3 + Claude fijo: revision de spec (ya decidida) + revision del diff antes del PR + Claude en escalacion. El producto es (b). (a) existe para medir que atrapa la revision de Claude que los gates no atraparon.
- Repeticiones: 1 corrida por feature grande (S3), 3 por feature chica (D1, D3, S2), held-out S1 una sola vez. Cada corrida con la cache de worklist de mmorch APAGADA (una cache prendida repite la misma corrida byte a byte). La cache de prefijo de DeepSeek queda prendida: abarata input, no cambia el output.
- Suite total de mmorch (`pytest tests -q`) tras cada corrida de dogfood: es el gate de regresion del ticket 13, no una metrica.
- Metricas por feature, todas computables: aceptacion verde/roja; USD por fase (ledger); minutos de pared; nivel maximo de escalacion; rechazos por gate; lineas agregadas/borradas (git diff --numstat); mutation score (ticket 13, mutmut); ruff+mypy nuevos = 0; suite total verde. Sin metricas de "calidad" por juicio LLM.
- Costo de las repeticiones: real, no cero. El output de los modelos no se cachea; solo el input repetido baja de precio. Estimado: ~US$2 extra sobre el total.
- Revision del diff por Claude antes del PR: bloquea SOLO con evidencia. Claude escribe un test que falla y el pipeline vuelve a build con ese test. Sin test, la observacion queda anotada en `supervision.md` y el PR sigue. Coherente con el ticket 03 (Claude promueve solo con oraculo + evidencia).
- Criterio de "gana" contra el engine viejo: verde en >= 5 de 6 features incluida la held-out; mediana de USD por feature < US$1; cero intervenciones humanas antes del nivel 4. Si una condicion falla, el README lo dice tal cual.
- Revision fija de Claude: queda siempre, como seguro, aunque no atrape nada en las 6 features. El usuario acepta su costo en minutos. Se reporta cuantos defectos atrapo que los gates no atraparon.
- Reporte: tabla en README "Measured" (una fila por feature y brazo) + nota en el vault con tag `mmorch`.
- Topes por corrida: POR AVANCE, no por reloj. Cada vuelta del fix loop registra tests que fallan y novedad del diff. (1) Fallos no bajan en 2 vueltas seguidas -> escala un nivel. (2) Menos del 10% de lineas cambiadas son nuevas respecto de la vuelta anterior -> escala (generaliza el detector de atasco por hash del ticket 03). USD queda como tope de fondo: US$3 por corrida, corta y escala a humano. Reloj solo por comando (10 min para un `mvn`/`pytest` colgado), nunca por feature. Claude NO vigila vueltas: seria un juez sin numero.
- Valores por defecto en `sdlc.toml`: `usd_max = 3`, `stall_rounds = 2`, `diff_novelty_min = 0.10`, `cmd_timeout_s = 600`.

## Resolution (2026-09-11)

Protocolo cerrado: 2 brazos (v3 puro = control; v3 + Claude fijo = producto), repeticiones 1/3/held-out x1 con cache de worklist apagada, 9 metricas computables, bloqueo de Claude solo con test que falla, criterio de gana 5/6 + mediana < US$1 + 0 humanos antes del nivel 4, topes por avance + USD de fondo. Reporte: README "Measured" + vault.
