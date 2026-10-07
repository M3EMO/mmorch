# Aristas indirectas por análisis estático
Type: research
Status: resolved
Blocked by: 02
Map: ../map.md

## Question

¿Cómo se detectan las aristas indirectas sin ejecutar el código, y cuáles ya cubre codegraph?

Candidatos a evaluar con los casos del ticket 02:

- AST: nombre de función usado como valor (argumento, default, elemento de dict o lista).
- Decoradores que registran la función en una tabla (`@_tool`).
- Strings que coinciden con un nombre de función o de script (`getattr`, `subprocess` con ruta).

Criterio: recall sobre los casos del inventario y tasa de falsos positivos. Un grep por palabra (juez v2 del banco) es la línea base barata.

## Answer

Aprobado (2026-09-27). Banco de aristas indirectas (8 repos Python, 12 tareas, deepseek-v4-pro, 108 corridas, US$1.11, cero errores de API; commits 6bf14a5, 0f66b1c y el de resultados en `orch-spike`): acopladas A 46% (11/24), L 38% (9/24), I 92% (22/24); I contra L +54 puntos, p = 8.9e-05; control 12/12 en las tres condiciones.

- Sin aristas indirectas fallan 3 de 3: t01 (kwarg), t03 (`getattr` con prefijo), t07 (atributo asignado + async). t05 (default con alias) acierta 0 y 1 de 3. Con I, t03, t05 y t07 aciertan 3 de 3, y t01 acierta 2 de 3.
- El agente ya resuelve solo lista de hooks (t04), dict de funciones (t06) y `key=` (t08): el nombre de la función aparece al lado del contenedor, y grep lo encuentra.
- El informe de literales instalado no cubre este acople (L no supera a A).
- Límite previsto en el pre-registro: t02 (registro por decorador) queda en 2 de 3 en las tres condiciones; el salto llega a `run_all` y no a `health.py`.
- Instalado (commit eab979b): `mmorch/impacto_indirecto.py`, sumado a `impacto.report` en `.py`, con cache de hechos por archivo. El detector medido tardaba 14-24 s por archivo; el instalado tarda 4.7 s en frío y 0.6 s con cache en Portfolio, y el hook 3.5 s en este repo. El informe instalado es igual al medido en las 12 tareas y en 15 módulos reales. La comparación encontró un bug del cache (contenedores locales como `stages`), corregido con su test. Aceptado por el usuario el 2026-09-27.

## Notes

**2026-09-27 — pre-registro (Claude, antes de correr; método elegido por el usuario: banco de tareas):**

- Pregunta operativa: ¿el informe instalado necesita aristas indirectas? Se mide con el protocolo de los tickets 09, 11 y 12.
- Detector (`scripts/tareas_indirectas/extractor_indirectas.py` en `orch-spike`, commit 6bf14a5): AST, clases del inventario del ticket 02. 2a función como valor (kwarg, argumento, default, contenedor, atributo asignado) con un salto hasta la invocación; 2b decorador que guarda en un registro global; 2c string dentro de `getattr`/`import_module` que es prefijo de una función del escritor; 2d alias de import. Congelado ANTES de escribir las tareas; probado solo sobre este repo (`auto_apply_nightly.py`, `checkers.py`, `sdlc.py`).
- Banco: 8 repos Python con 30 señuelos cada uno (los del ticket 09), 8 tareas acopladas (una por mecanismo: kwarg, registro por decorador, `getattr` con prefijo, lista de hooks, default con alias, dict de funciones, atributo asignado, `key=` de `sorted`) y 4 de control. Oráculo validado: referencia completa pasa en las 12; "solo escritor" falla en las 8 acopladas y pasa en las 4 de control.
- Condiciones: A (sin informe), L (informe de literales instalado, `mmorch.impacto`), I (L + aristas indirectas). deepseek-v4-pro, 3 repeticiones, 108 corridas, 3 workers; errores de API contados aparte y repetidos.
- Dato previo: L sale vacío en 11 de 12 tareas (en t04 solo muestra `kind`, literal común). Límite visible: en t02 el salto llega a `run_all` y no a `health.py`, que interpreta el retorno.
- Criterio: I pasa si supera a L por >= 25 puntos en acopladas sin perder más de 10 en control. Si pasa, las aristas indirectas entran al informe (Python primero). Si no pasa, el ticket se cierra: el informe no las necesita.
