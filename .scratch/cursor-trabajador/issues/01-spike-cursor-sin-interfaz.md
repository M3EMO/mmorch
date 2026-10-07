# Spike: cursor-agent sin interfaz en un worktree sobre 3 tareas del banco
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿`cursor-agent -p --force --workspace <worktree>` resuelve tareas de nuestros bancos sin intervención, y qué deja medible?

Medir en 3 tareas acopladas del banco del ticket 09 del mapa de impacto (`orch-spike/scripts/tareas`), con el informe de impacto al inicio del prompt:

- Acierto con el test oculto (el mismo oráculo del banco).
- Tiempo por tarea y código de salida.
- Qué archivos tocó y si salió del worktree.
- Qué informa la salida JSON: pasos, modelo usado, consumo si lo reporta.
- Si hace preguntas o se traba, y cómo termina.

Es AFK: lo corre el agente solo. No es un banco comparativo; decide si vale la pena el banco del ticket 04.

## Answer

Resuelto (2026-09-29): sí, vale la pena el banco. Código y resultados en `orch-spike/scripts/cursor_spike` (commit 522e24d). `cursor-agent` 2026.08.25, `composer-2.5` (pool "Cursor Models"), sesión iniciada con la cuenta del usuario.

- Corre desatendido: salida 0 en las 6 corridas, sin preguntas ni trabas; termina con una línea de qué cambió.
- Acierto 2/3 en las dos tandas: t02 (latencia en ms) y t05 (ruta del flag) bien, tocando escritor y lectores; t01 (total neto) mal las dos veces: cambió solo `orders.py` y no `reports/nightly.py`, aunque el informe de impacto estaba en `TAREA.md`. Es la debilidad de t01 vista en el ticket 14 del mapa de impacto: el informe resume `total` como literal común.
- Tiempo: unos 95 s por tarea con el entorno corregido (58-316 s en la primera tanda).
- Consumo por tarea: 15-18 mil tokens de entrada, 1-1.4 mil de salida y 95-190 mil leídos de cache. El JSON final trae `usage`, no dólares.
- Se quedó en la carpeta: solo cambió archivos del repo; ningún commit (siguió las reglas). No se puede probar del todo que no escribió afuera.
- **Hallazgo: `cursor-agent` carga los hooks de `~/.claude/settings.json`.** Arma su comando en sintaxis PowerShell y lo ejecuta con `$SHELL`. Lanzado desde Git Bash (`SHELL=bash.exe`) el hook falla, Cursor lo toma como bloqueo y el agente edita por terminal: esquiva el `never-edit-guard` y el informe de impacto. Sin `SHELL` en el entorno, la herramienta de edición funciona (verificado). El despachador tiene que lanzarlo sin `SHELL`, `BASH`, `BASH_ENV` ni `MSYSTEM`.
