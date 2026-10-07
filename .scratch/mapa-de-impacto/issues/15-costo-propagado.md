# Costo propagado: anotar el cuadrático oculto antes de editar
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Una anotación estática del costo evita que el agente agregue un cuadrático oculto (pilar 1 del video)? Un ejemplo es llamar dentro de un bucle a una función que ya recorre todo el almacén.

Relación con lo que existe: el banco de escala de `mmorch/auto` (commit 2f14d4f) subió el gate de 26/40 a 33/40 con registro tipado y dueño único. Esa medición mide diseño, no una anotación en el momento de editar.

Método: tareas donde la solución natural llama en bucle a una función de otro módulo que recorre un almacén; test oculto con n grande y tope de tiempo, más la corrección funcional. Condiciones: A (sin anotación) y C (el informe agrega, por función llamada desde el archivo, "recorre `X` completo: O(n) por llamada"). La anotación sale del AST: bucles sobre lecturas de un almacén.

Criterio: C supera a A por >= 25 puntos en las tareas con cuadrático sin perder más de 10 en control.

## Answer

C2 pasa el banco (2026-09-28) y quedó instalado refinado (2026-09-29). Aceptado por el usuario el 2026-09-29. Banco de 12 tareas, deepseek-v4-pro, 72 corridas, US$0.51, cero errores de API (commits 95902a7, 0b2bf38 y el de resultados en `orch-spike`): trampas B 8/24 (33%) → C2 24/24 (100%), +67 puntos, p = 3.3e-07; control 12/12 en las dos. Las 16 fallas de B son por tiempo (cuadrático oculto).

- C1 (anotar al inicio) no sirve por construcción: la función trampa vive en un módulo que el escritor todavía no importa. El aviso útil llega después de escribir, cuando la llamada en bucle ya existe.
- Sin aviso, el agente cae en 6 de 8 trampas en las 3 repeticiones; evita solas `price_of` y `is_known` (3/3) y casi `find` (2/3).
- Condición de instalación (etiqueta manual con el código de cada lectura, 28 líneas sobre `mmorch/` y `scripts/`): 14 relecturas del mismo almacén en un bucle (por ejemplo `_lexicon_text()`, `projects._load`, `load_population`, `adjudicate` que relista la carpeta del mismo proyecto por cada nota) y 14 falsas alarmas (lee un archivo distinto por vuelta: `sha_file(rel)`, `_first_doc_line(py)`, `load_manifest(d)`). 50%: cumple por el mínimo.
- Refinamiento (decisión del usuario, después de etiquetar): una lectura cuya ruta sale de un parámetro no cuenta. El aviso queda idéntico en las 24 referencias del banco, así que el resultado medido vale. En el repo real baja a 8 avisos, 7 relecturas reales (88%); pierde 7 de las 14 relecturas reales, las que leen una ruta que llega por parámetro.
- Instalado (commit 6d2099c): `mmorch/impacto_costo.py` con cache de hechos por archivo (4.7 s en frío, 0.2 s con cache); hook `PostToolUse` nuevo en `~/.claude/settings.json` (`impacto_hook.py post`, 0.3-0.4 s), que avisa solo cuando el aviso cambia para ese archivo en la sesión; el modo `cursor` suma el mismo chequeo. Igual al detector v2 en 150 módulos reales. Suite completa: 1287 tests en verde.

## Notes

**2026-09-28 — detector C1 y su negativo de diseño (sin correr):**

- C1 (`orch-spike/scripts/tareas_costo/extractor_costo.py`, commit 95902a7, congelado antes de las tareas): marca funciones que leen un almacén entero por llamada (abrir para leer, cargar JSON/CSV, listar carpeta, SELECT sin WHERE), con un salto; el informe lista las del archivo y las de los módulos que importa.
- En el banco (8 tareas trampa + 4 control, oráculo: corrección con n grande y tope de 2 s), C1 no marca ninguna función trampa en las 8 tareas: el escritor todavía no importa el módulo con la trampa; el agente lo descubre al resolver. Por construcción C1 = B. Se registra como negativo de diseño y no se corre (decisión del usuario).
- En el repo real, C1 marca casi todas las funciones que leen un archivo: son marcas ciertas, pero sin saber dónde se llaman no dicen nada útil.

**2026-09-28 — cambio de método (decisión del usuario): C2, chequeo después de escribir.** Riesgo declarado: C2 se diseña después de ver las tareas.

**2026-09-28 — pre-registro de C2 (Claude, antes de correr):**

- C2 (`extractor_costo_post.py`): después de cada `write_file` del agente, busca en ese archivo llamadas dentro de un bucle (cuerpo de for/while, elemento o condición de comprehension salvo su primer iterable, función o lambda de `key=`/`map`/`filter`) a funciones que recorren un almacén según C1 (un salto). Avisa en la observación del `write_file`, solo cuando el aviso cambia.
- Banco (commit del congelado): 8 repos Python con 30 señuelos, 8 tareas trampa (la solución natural llama en bucle a `get_customer`, `qty`, `find`, `for_file`, `price_of`, `count_for`, `get_rate` —un salto— o `is_known`) y 4 controles. Oráculo: corrección con n grande y tope de 2 s (la versión ingenua tarda unos 10 s; la eficiente, decenas de ms). Validado en las 12: ingenua falla en las 8 trampas; eficiente pasa; controles pasan con las dos.
- Sobre las referencias, C2 marca las 8 ingenuas y ninguna eficiente ni control (esperable: se diseñó después de ver las tareas).
- Condiciones: B (informe instalado al inicio) y C2 (B + chequeo después de escribir). deepseek-v4-pro, 3 repeticiones, 72 corridas, 3 workers; errores de API aparte.
- Criterio: C2 supera a B por >= 25 puntos en trampas sin perder más de 10 en control.
- Instalación: además del banco, al menos la mitad de las líneas de C2 sobre `mmorch/` y `scripts/` tienen que ser relecturas del mismo almacén en un bucle (etiqueta manual). Dato visto antes de registrar: 28 líneas, cerca de la mitad relecturas del mismo almacén y la otra mitad lecturas de un archivo distinto por vuelta. Está en el límite.
