# Repo ajeno sin registro previo
Type: prototype
Status: resolved
Blocked by: 05 (resuelto)
Map: ../map.md

## Question

¿La herramienta de impacto funciona en un repo que Claude no conoce y que no tiene contratos ni registro tipado?

Candidato: Portfolio (Python, tiene tests). Protocolo: generar mutantes sobre sus almacenes compartidos, medir el recall de impacto con y sin la herramienta, y contar cuántas aristas de datos encuentra el trazado frente al análisis estático. Estudio (TS) queda para cuando exista un trazador no Python (ver Not yet specified).

## Comments

**2026-09-25 — método nuevo, aprobado por el usuario, pre-registro antes de medir:**

La métrica original (recall de impacto) quedó descartada por el ticket 01. Nuevo método, local y sin API: el historial de git de Portfolio como verdad.

- Un commit "toca" el literal `t` en el archivo X si una línea agregada o borrada de X contiene `t` dentro de un literal de texto con forma de dato (mismo filtro que `mmorch/impacto.py`).
- Co-cambio real: un commit que toca `t` en dos archivos X e Y (ambos existen hoy, fuera de tests).
- Recall: de los co-cambios reales (X, Y, t), qué fracción lista el informe actual de X o de Y.
- Precisión: de los pares listados (X, Y, t) cuyo `t` cambió alguna vez en X, qué fracción cambió alguna vez junto con Y.
- Línea base al azar: la misma precisión con un módulo Y' elegido al azar entre los que existen hoy.
- Criterio: el informe sirve en un repo real si recall >= 0.5 y la precisión supera a la línea base al azar por al menos 3 veces.

**2026-09-25 — resultado del método pre-registrado (Portfolio, 415 commits, 327 módulos):** recall 0.13 (criterio >= 0.5: NO cumple); precisión 0.18 contra 0.002 al azar, lift 104x (criterio >= 3x: cumple). Diagnóstico posterior (no cambia el resultado): 3058 de 3457 fallos son literales comunes resumidos por diseño (más de 5 módulos) y 399 los corta el tope de 10 líneas; en literales específicos el recall es 0.57. Se descartó una lista de exclusión (`__main__` y similares son ruido de la verdad, no del informe; el orden ya los deja al final).

**2026-09-25 — pre-registro del informe que mira la edición (aprobado por el usuario):**

- Regla: el hook recibe el cambio (`old_string`/`new_string` en Edit, `edits` en MultiEdit, `content` en Write). Los literales que la edición toca van primero, cada uno con hasta 10 módulos listados aunque sea común; después sigue el informe actual; tope total 10 líneas.
- Simulación sobre el historial: cada par (commit, archivo X) es una edición; los literales tocados salen de las líneas cambiadas de X; la verdad son los otros módulos que tocaron ese literal en el mismo commit. Módulos y literales tomados de HEAD (limitación: el repo cambió desde cada commit).
- Métricas por evento: recall de la verdad; precisión de los pares listados por literales tocados, contra un módulo al azar.
- La idea nació en Portfolio. Criterio: recall >= 0.5 y precisión >= 3x el azar en Portfolio Y en dos repos que no se miraron para diseñarla (historial de orchestration y Proyecto_Adepor). También se reporta el informe actual con la misma simulación.

**2026-09-26 — resultado del informe que mira la edición:** recall (verdad completa) Portfolio 0.12 → 0.18, orchestration 0.10 → 0.20, Proyecto_Adepor 0.02 → 0.04; lift 45x, 12x y 33x. Criterio (recall >= 0.5 en los tres): NO cumple. Diagnóstico posterior: con la verdad restringida a literales vivos hoy, recall 0.18 / 0.44 / 0.04 (commits de hasta 5 archivos: 0.31 / 0.56 / 0.22). Límite del método: un co-cambio en git no implica "el lector tenía que actualizarse". Conclusión: el informe tiene señal (lift 12-104x) pero cubre una minoría de los co-cambios; el co-cambio es una verdad débil.

**2026-09-26 — decisión del usuario: opción 3, banco de tareas sobre Portfolio real.** Mismo protocolo que el ticket 09 (tareas acopladas y de control, tests ocultos, oráculo validado con referencia completa y "solo escritor", agente deepseek-v4-pro con y sin informe, criterio +25 puntos en acopladas sin perder más de 10 en control), pero sobre una copia de Portfolio y con escritores y lectores reales. Aviso dado al usuario: partes del código viajan a la API de DeepSeek.

**2026-09-26 — pre-registro del banco sobre Portfolio (antes de correr agentes):**

- 12 tareas (8 acopladas, 4 de control) sobre escritores y lectores reales de Portfolio, redactadas por un subagente a partir de 16 candidatos (`research/07-candidatos-portfolio.md`). Oráculo verificado por Claude: en las 12, HEAD falla el test oculto y la referencia completa pasa; "solo escritor" falla en las 8 acopladas y pasa en las 4 de control.
- Revisor cross-family (deepseek-chat): ningún enunciado insinúa al lector.
- El informe instalado (`mmorch/impacto.py`) menciona al lector real en 7 de 8 acopladas; en t08 no (acople de dos saltos, recortado por el tope de 10 líneas). No se ajusta.
- Copia por corrida: `git archive` de HEAD (scripts, tests, config, reports, frontend); tests con el .venv de Portfolio. `run_tests` del agente corre los tests existentes que importan el escritor.
- Condiciones A (sin informe) y B (informe instalado); deepseek-v4-pro; 3 repeticiones; 72 corridas.
- Criterio: B supera a A por >= 25 puntos en acopladas sin perder más de 10 en control.

**2026-09-26 — resultado del banco v1 sobre Portfolio: INVÁLIDO por efecto piso.** Acopladas A 0/24, B 2/24; control A 8/12, B 8/12 (US$4.71). En 27 de 48 corridas acopladas el agente ni tocó el escritor; en 33 de 48 agotó los 15 pasos. Causas del instrumento: pocos pasos para archivos largos, y `write_file` reescribe el archivo entero mientras `read_file` recorta a 6000 caracteres. El criterio pre-registrado no se cumple, pero la medición no responde la pregunta.

**2026-09-26 — pre-registro del agente v2 (aprobado por el usuario), antes de correr:** `read_file` con rango de líneas (`start`/`end`) y tope de observación de 20 mil caracteres; herramienta nueva `edit_file` (reemplazo puntual: `old` debe aparecer una sola vez); tope de 30 pasos. Tareas, tests ocultos, informe y criterio sin cambios. Condición de validez: si el control de A queda debajo de 75%, el instrumento sigue inválido y no se concluye nada sobre el informe.

**2026-09-26 — agente v2: control A 12/12 (condición de validez cumplida, US$0.02 por corrida). Corrida completa BLOQUEADA:** la cuenta de DeepSeek se quedó sin saldo (errores 402 "Insufficient Balance" y 429 con límite de concurrencia atado al saldo). 46 de 72 corridas murieron sin empezar; se descartan (respaldo en `results_portfolio_v2_*.jsonl.con-errores.bak`). Quedan 26 corridas válidas. Pendiente: el usuario recarga saldo y se corren las 46 faltantes (el runner se reanuda solo). Los bancos anteriores tienen cero errores de API.

## Answer

Aprobado (2026-09-26), con el banco de tareas sobre Portfolio real (opción 3). Agente v2, deepseek-v4-pro, 72 corridas, cero errores de API, US$5.18, copia fija en af949ba: acopladas A 54% (13/24) → B 96% (23/24), +42 puntos, p = 0.0009; control A 100% (12/12), B 92% (11/12), dentro del margen de 10. En t01, t04 y t07, A falló 9/9 y B acertó 9/9; en esas fallas A nunca tocó al lector (el error del video). En t08 el informe no nombraba al lector (dos saltos, recortado) y el agente lo encontró igual. Antecedentes: la métrica por co-cambios en git no cumplió (recall 0.12-0.20, lift 12-104x; el co-cambio es una verdad débil); el banco v1 fue inválido por efecto piso (15 pasos y reescritura de archivo entero); una corrida v2 murió por falta de saldo en DeepSeek y se repitió. Hallazgo lateral: 4 acoples ya rotos en Portfolio (2 verificados), derivados a una tarea aparte. Aceptado por el usuario el 2026-09-26.
