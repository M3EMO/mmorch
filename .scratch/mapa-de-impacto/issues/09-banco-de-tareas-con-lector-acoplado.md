# Banco de tareas de cambio con lector acoplado
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Un informe de impacto con aristas de datos evita el error del video? El error: un agente cambia un escritor y no actualiza al lector acoplado que no referencia el código.

Aprobado por el usuario el 2026-09-25, después del resultado negativo del ticket 01.

## Diseño (pre-registro, antes de correr)

- **Repos:** mini-repos sintéticos en Python, de 4 a 8 módulos cada uno. El lector acoplado no importa al escritor; comparten un almacén (sqlite, jsonl, json o variable de entorno).
- **Tareas:** 12 en total. 8 acopladas: el cambio correcto exige tocar el lector. 4 de control: el lector NO debe cambiar.
- **Tests:** visibles solo del escritor, como un repo real con cobertura incompleta. Ocultos de escritor y lector: son el oráculo.
- **Agente:** loop con herramientas `list_files`, `read_file`, `grep`, `write_file`, `run_tests` (solo visibles) y `done`. Máximo 15 pasos. Modelo: `deepseek-v4-pro` (coder medido).
- **Condición A:** solo herramientas.
- **Condición B:** herramientas más un informe de impacto al inicio. El informe sale de un extractor estático automático: literales de almacén del módulo escritor (rutas, tablas SQL, claves de entorno) y los otros módulos que contienen el mismo literal.
- **Repeticiones:** 3 por tarea y condición, 72 corridas en total.
- **Métrica:** tasa de éxito (todos los ocultos verdes), separada en acopladas y en control.
- **Criterio:** B sirve si supera a A por al menos 25 puntos en acopladas, sin perder más de 10 puntos en control.
- **Sesgo conocido:** Claude escribe las tareas y también el extractor de B. Mitigación: tareas, tests ocultos y extractor se congelan con un commit antes de la primera corrida; el informe de B es automático, nunca a mano; un modelo de otra familia revisa que ningún enunciado mencione al lector.
- **Costo estimado:** menos de US$10 en deepseek. Se avisa antes de superar US$15.

## Comments

**2026-09-25 — enmiendas antes de la corrida principal (Claude):**

- Oráculo validado: en las 12 tareas, la referencia completa pasa los ocultos; la referencia "solo escritor" falla en las 8 acopladas y pasa en las 4 de control.
- Extractor: se quitó un filtro por frecuencia que borraba lectores reales en repos chicos (t03, t05, t08 salían vacíos). El cambio es genérico, pero se hizo después de ver las tareas.
- Piloto (n=8, costo ~US$0.02 por corrida): con 5 módulos, la condición A lee todos los archivos y acierta. Enmienda: cada repo recibe 30 módulos señuelo automáticos con almacenes propios y campos comunes (`total`, `status`, `name`...). El informe de B también lista señuelos.
- Con señuelos, A sigue acertando en t01 y t05 (grep del literal). El revisor cross-family (deepseek-chat) marcó dos frases que insinuaban al lector: "El sistema tiene que seguir reportando bien" (t01) y "para todo el sistema" (t05). Se borraron.
- Brazo exploratorio (no entra al criterio): mismo banco con `deepseek-chat`, el modelo barato de generación masiva.

## Answer

Positivo; cumple el criterio pre-registrado. Con `deepseek-v4-pro`, las tareas acopladas pasan de 42% (10/24) sin informe a 96% (23/24) con informe; control 100% en ambas; p = 4.4e-05 (Fisher, una cola); costo US$0.81. Brazo exploratorio `deepseek-chat`: 42% → 79%, p = 8.6e-03, US$0.03. Mecanismo: las 14 fallas sin informe de cada modelo son el error del video (cambió el escritor y nunca tocó al lector); con informe, v4-pro tocó al lector 24/24. Las 5 fallas de deepseek-chat con informe ignoraron el informe; en t01 el ruido de señuelos (12 líneas junto a `total`) probablemente ahoga la señal. Dato del piloto: dos frases que insinuaban al lector subían A a acierto casi total; sin ellas, t01 A = 0/3. Límites: repos y tareas escritos por Claude; el extractor solo ve literales compartidos; control fácil. Código y datos: `../orch-spike`, rama `spike/impacto`, `scripts/tareas/`. Aceptado por el usuario el 2026-09-25.
