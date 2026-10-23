# Lenguajes, dependencia y determinismo
Type: grilling
Status: open
Blocked by:
Map: ../map.md

## Question

¿En qué lenguajes entran los tests de propiedades, cómo llega la dependencia al repo destino y cómo queda determinista el oráculo?

Hechos medidos el 2026-10-23:

- Hypothesis no está instalado en el venv de orchestration ni en el extra `dev` de `pyproject.toml`.
- Los comandos del SDLC corren en un contenedor sin red (`sdlc.toml contenedor`). El contenedor no instala paquetes: un paquete ausente es un error del entorno (`builder-e2e` ticket 07).
- El SDLC corre sobre repos Python, TypeScript (F2), Java (ChatBot) y otros (`_mutantes_texto`).
- Hypothesis genera entradas al azar por default. Un oráculo con azar puede dar verde y rojo en dos corridas del mismo código. `gate_suite_total` ya trata un fallo no repetido como flaky.

Opciones a decidir sobre lenguajes:

- A. Solo Python con Hypothesis. Un repo de otro lenguaje sigue el camino de hoy.
- B. Python con Hypothesis y TypeScript con fast-check. El autor del test elige la librería por `_lenguaje()`.

Opciones a decidir sobre la dependencia:

- C. `sdlc init` agrega la librería al extra de tests del repo destino y a la imagen del contenedor.
- D. El SDLC exige la librería y corta con un error del entorno si falta, sin tocar el repo.

Opciones a decidir sobre el determinismo:

- E. `derandomize=True` (o `--hypothesis-seed` fijo) dentro de los gates. La búsqueda es la misma en cada corrida.
- F. Azar con la base de ejemplos de Hypothesis persistida. Un contraejemplo hallado queda guardado y se repite.

## Test de aceptación propuesto

`tests/test_sdlc_propiedades.py::test_propiedad_determinista`: un test de propiedades que falla solo con una entrada rara. El gate corre dos veces sobre el mismo código y da el mismo veredicto las dos veces. Un repo sin la librería instalada recibe el diagnóstico de error del entorno de la etapa 4, no un fallo del código. Con la opción A, un repo TypeScript no recibe tests de propiedades y su corrida no cambia.
