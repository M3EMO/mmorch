# Extractor y banco para Java
Type: prototype
Status: resolved
Blocked by: 11 (resuelto)
Map: ../map.md

## Question

Lo mismo que el ticket 11 para Java (repo de referencia: ChatBot), reutilizando la tabla de tipos de nodo de tree-sitter. Oráculo por tests ocultos con el build del repo. Solo si pasa, el hook se activa para `.java`.

## Answer

Aprobado (2026-09-27). Banco Java (8 repos, 12 tareas, deepseek-v4-pro, 72 corridas, US$0.87, cero errores de API; commits 2b9faaa y 7d71b69 en `orch-spike`): acopladas A 38% (9/24) → B 96% (23/24), +58 puntos, p = 1.4e-05; control A 12/12, B 12/12. Única falla con informe: t06 r2, el agente cambió `Exporter.java` y no tocó `Reconcile.java` aunque el informe lo nombraba. Sin informe, t01, t02, t05 y t06 fallan 3 de 3.

Instalado en `mmorch/impacto.py` (commit af714fc): `.java` con tree-sitter-java, dependencia opcional `impacto-java`; excluye `*Test.java` y `*Tests.java`. El informe instalado es idéntico al medido en las 12 tareas, y el hook de punta a punta lo entrega sobre `.java`.

Límite conocido, no medido: si el escritor no tiene literales (todo detrás de una clase de acceso a datos), el informe sale vacío.

Arreglo lateral (commit 015d849): sin `.git` en ningún padre, el hook no informa; antes, un archivo en `C:\` hacía recorrer el disco hasta agotar el timeout. Aceptado por el usuario el 2026-09-27.

## Notes

**2026-09-27 — pre-registro (Claude, antes de correr):**

- Banco: los 8 repos del ticket 09 portados a Java 17 sin dependencias, 12 tareas, 30 señuelos Java por repo. Diferencias forzadas por Java: almacenes en CSV o líneas clave=valor (no SQLite ni JSON), configuración por propiedades de sistema (Java no cambia variables de entorno desde un test), tests como clases con `main`. Oráculo validado: referencia completa pasa en las 12; "solo escritor" falla en las 8 acopladas y pasa en las 4 de control.
- Corrección del puerto antes de medir: en r1 y r8 el nombre del archivo había quedado en una clase auxiliar (`Store`, `Db`), no en el escritor. En Python el escritor tiene el SQL con el nombre de la tabla. Lo moví al escritor para copiar la lógica del ticket 09. El caso "escritor sin literales, todo detrás de una clase de acceso a datos" queda fuera de esta medición; es un límite conocido del informe.
- Banco sintético, no el repo ChatBot que nombra la pregunta: mismo diseño que los tickets 09 y 11, para comparar lenguajes con el mismo oráculo.
- Extractor: tree-sitter-java 0.23.5, nodos `string_fragment` con forma de dato; mismo formato que `mmorch/impacto.py` y que la v1 del ticket 11. Excluye `tests/` y `*Test.java`.
- Condiciones: A (sin informe), B (informe). deepseek-v4-pro, 3 repeticiones, 72 corridas, 3 workers. Las corridas con error de API se cuentan aparte y se repiten.
- Criterio (el del ticket 11): B pasa si supera a A por >= 25 puntos en acopladas sin perder más de 10 en control. Si pasa, el hook se activa para `.java`.
