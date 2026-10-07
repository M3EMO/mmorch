# Extractor y banco para TypeScript
Type: prototype
Status: resolved
Blocked by: 05 (resuelto)
Map: ../map.md

## Question

¿Un extractor de literales compartidos sobre tree-sitter pasa el banco de tareas en TypeScript? Repos candidatos para inspirar las tareas: Estudio (TS). Mini-repos TS con escritor, almacén y lector acoplado; oráculo por tests ocultos (node). Mismo criterio del ticket 09. En TS los literales de datos también viven en tipos, constantes exportadas y esquemas: el extractor debe cubrirlos o el banco lo va a mostrar. Solo si pasa, el hook se activa para `.ts`/`.tsx` (decisión Q7 del ticket 03).

## Comments

**2026-09-26 — pre-registro (Claude, antes de correr):**

- Banco: los 8 repos del ticket 09 portados a TypeScript idiomático (campos como propiedades, no strings), 12 tareas, 30 señuelos TS por repo; Node 24 (TypeScript sin compilar, `node:sqlite`, `node:test`). Oráculo validado: referencia completa pasa en las 12; "solo escritor" falla en las 8 acopladas y pasa en las 4 de control.
- Extractor sobre tree-sitter, dos variantes con el mismo formato que `mmorch/impacto.py` (especificidad, resumen > 5 módulos, tope 10 líneas):
  - v1: solo literales de texto con forma de dato (igual que Python).
  - v2: v1 + nombres de propiedad de datos: claves de objetos literales, propiedades de tipos e interfaces, y accesos `obj.campo` que no son llamadas a método.
- Condiciones: A (sin informe), B1 (informe v1), B2 (informe v2). deepseek-v4-pro, 3 repeticiones.
- Criterio: una variante pasa si supera a A por >= 25 puntos en acopladas sin perder más de 10 en control. Si pasan las dos, gana la de mayor acierto en acopladas; en empate, v1 (más simple).

## Answer

Aprobado (2026-09-26). Banco TS (8 repos, 12 tareas, deepseek-v4-pro, 108 corridas, US$1.21, cero errores de API): acopladas A 38% (9/24) → B1 100% (24/24) y B2 100% (24/24), +62 puntos, p = 1.2e-06; control A 12/12, B1 12/12, B2 11/12. Por pre-registro gana v1 (empate en acopladas; más simple). Dato: v1 acertó t02 y t03 sin ver el campo exacto; alcanza con señalar al módulo que comparte datos (en t02, el nombre de archivo `calls.jsonl`). Instalado en `mmorch/impacto.py` (commit c128059): `.ts`/`.tsx` con tree-sitter, dependencia opcional `impacto-ts`; el informe instalado es idéntico al medido en las 12 tareas. Aceptado por el usuario el 2026-09-26.
