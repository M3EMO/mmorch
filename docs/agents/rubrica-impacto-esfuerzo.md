# Rúbrica de impacto y esfuerzo

Esta rúbrica puntúa issues de bd en una escala continua de 0 a 5.
El script `scripts/puntaje_issues.py` calcula los dos ejes y escribe la tabla.
Los puntajes de impacto viven en `docs/agents/puntajes.json`, uno por issue.

## Esfuerzo (medido)

El script mide el esfuerzo con git.
Un commit cuenta para un issue cuando su mensaje nombra el ID del issue.
Un commit que nombra varios issues reparte sus líneas en partes iguales.
Las líneas excluyen `vault/`, `.beads/`, `README.md` y `docs/generated/`.

La fórmula es `5 * ln(1 + L/16) / ln(1 + 2500/16)`, con tope 5.
`L` es la suma de líneas agregadas y borradas.

| Líneas | Esfuerzo |
|---|---|
| 0 | 0 |
| 50 | 1.4 |
| 150 | 2.3 |
| 400 | 3.2 |
| 1000 | 4.1 |
| 2500 o más | 5 |

Límite: el script no mide sesiones, cupo ni tiempo de diseño.
Límite: un commit que no nombra el ID no cuenta para el issue.
Límite: el reparto igual subestima el issue más grande de un commit compartido.
Un issue con un mapa wayfinder o una sesión aparte lleva `extra` en el JSON.
`extra` suma de 0 a 1 punto, y el JSON dice el motivo.

## Impacto (rúbrica)

El impacto combina tres criterios de 0 a 5.
La fórmula es `(2*V + 2*A + E) / 5`.

### V: valor o riesgo que saca

- 0: El cambio no saca riesgo ni agrega valor.
- 1: El cambio arregla un detalle cosmético o un texto.
- 2: El cambio saca fricción manual o un paso repetido.
- 3: El cambio arregla un bug que confunde o gasta cupo.
- 4: El cambio arregla un gate que deja pasar trabajo malo.
- 5: El cambio cierra un hueco de seguridad, pérdida de datos o veredicto falso.

### A: alcance

- 0: Ninguna corrida usa el cambio.
- 1: Un caso raro usa el cambio.
- 2: Un repo usa el cambio en algunas corridas.
- 3: Un repo usa el cambio en cada corrida.
- 4: Varios repos usan el cambio en cada corrida.
- 5: Toda corrida de todo repo usa el cambio.

### E: evidencia

- 0: Nadie vio el problema; el valor es una suposición.
- 2: Una persona vio el problema una vez.
- 3: Un log o un run-log muestra el problema.
- 4: Un test reproduce el problema antes del arreglo.
- 5: Un oráculo midió el efecto en varias corridas.

## Uso

1. Agregá el issue a `docs/agents/puntajes.json` con `V`, `A`, `E` y una nota.
2. Corré `python scripts/puntaje_issues.py` para ver la tabla.
3. Corré `python scripts/puntaje_issues.py --json` para alimentar un chart.

Un issue pendiente no tiene commits.
Su esfuerzo sale de `estimado` en el JSON, y la tabla lo marca como estimado.
