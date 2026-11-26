---
title: "Agent distraction y contexto viejo: cifras confirmadas, inventario local y propuesta sweep (2026-11-26)"
created: 2026-11-26
tags: [research, orchestration, agent-distraction, contexto-viejo, sweep]
status: seed
confidence: alta en cifras de fuentes primarias; media en la propuesta (sin medir en este setup)
sources: ["https://developers.googleblog.com/agent-distraction/", "https://arxiv.org/abs/2610.30571", "https://arxiv.org/abs/2611.09802", "https://arxiv.org/abs/2611.02383", "https://arxiv.org/abs/2611.19752", "https://lacuna.tiptreesystems.com/work/stalebench-isolating-stale-retrieval-in-agent-memory", "https://infoq.com/news/2026/11/google-agent-distraction/"]
---
## Pregunta

Esta nota busca un sistema mínimo para que los agentes de código no sigan contexto viejo.

## Cifras de fuentes primarias

### Google, "Agent Distraction" (Saccone y Garikapati, 2026-11-06)

- El 13% de las sesiones ve un TODO o FIXME con un bug ID. Confirmado.
- Solo el 6% de esas sesiones actúa sobre el bug. El 94% lo ignora.
- El 83% de esas acciones no tenía pedido. Corrección: el 83% cuenta sesiones con acción, no todos los side quests.
- El 41% de los desvíos no pedidos va a tickets cerrados: 32% arreglados, 9% obsoletos, 2% duplicados.
- Del desvío sobre bugs abiertos, el 61% solo busca y el 39% edita código.
- Una sesión con side quest usa 70% más pasos, 77% más tokens y 105% más espera. La cifra sale de una regresión con efectos fijos, p<0.0001.
- La muestra cubre miles de ingenieros y cientos de millones de pasos. El post no da un n exacto.
- La mitigación revisa la historia del TODO antes de borrarlo y manda cada borrado a revisión humana. La aceptación humana supera el 95%.
- El estudio mide solo TODOs. Specs y docs viejos aparecen nombrados, sin números.

### Xu y Wu, arXiv 2610.30571 (2026-10-22)

- El diseño usa 320 corridas, 40 proyectos Python sintéticos, Claude Code y Codex CLI.
- Sin la oración, el 90.1% de las elecciones sigue al archivo viejo. Solo el 4.2% sigue al código. Confirmado.
- Con la oración, el 91.4% de las elecciones sigue al código. Confirmado.
- Codex reescribió código existente en 20 de 80 corridas, y los tests fallaron en 17.
- Claude Code notó el conflicto en 79 de 80 corridas y siguió igual al archivo viejo.
- La oración dice, en inglés: "When a repository's documentation and its existing code disagree, follow the conventions used in the existing code".
- El mecanismo propuesto es simple. El archivo de instrucciones llega como instrucción y el código llega como salida de tool.
- El límite es fuerte: proyectos sintéticos, un tipo de conflicto, preprint sin revisión.

### Otros trabajos con números

- Mehtiyev et al., arXiv 2611.09802: 732 trayectorias, 22 tareas. Un comando de test viejo suma unas 2 llamadas por corrida, un tercio más. Ningún agente reparó el archivo de instrucciones.
- Shanto et al., arXiv 2611.02383: el 40.4% de los archivos de instrucciones tiene una sola edición. El 16.1% de las secciones pareadas entre formatos diverge.
- StaleBench, arXiv 2611.19752: con 892 hechos que cambian, la búsqueda por coseno devuelve la versión vieja el 53% de las veces.

## Inventario local (2026-11-26, solo lectura)

El chequeo determinista busca rutas `mmorch|sdlc|tools|tests/*.py` que un `.md` nombra y que no existen.

- `.scratch/`: 11 archivos nombran módulos borrados. `.scratch/vault-global/spec.md` sigue en "Status: ready-for-agent" y describe babel.
- `.scratch/`: 6 esfuerzos tienen todos sus tickets resueltos y siguen al alcance del agente. Son builder-e2e, cursor-trabajador, mapa-de-impacto, repos-convenciones, sdd-fase-2 y sdlc-6-gates.
- `docs/production-readiness/`: 14 archivos fechan 2026-08-27 a 2026-08-30. Cinco nombran babel y dos nombran módulos borrados.
- `vault/`: 9 notas nombran módulos borrados. `vault/roadmaps/candidatos.md` lista candidatos sobre ablation, babel y bucketrank, que ya no existen.
- `vault/`: 136 de 175 notas con status siguen en `status: seed`.
- Memoria: `sdlc-es-builder.md` nombra `mmorch/sdlc.py`. El código vive en el paquete `sdlc/`.
- `.claude/worktrees/`: 3 worktrees viejos (2026-10-26 a 2026-11-06) guardan 32k archivos con `vault/lexicon.md` y babel. `.git/info/exclude` los saca de rg, pero `grep -r` y `find` los leen.
- bd: 6 issues abiertos de 2026-10-26 y 2026-10-27. Ninguno contradice el código a simple vista.
- Skills globales: ninguna nombra un módulo borrado.

## Cobertura actual

- La oración de precedencia existe en `~/.claude/CLAUDE.md`, sección "Fuente de verdad".
- `never-edit-guard.js` bloquea escrituras. Su self-test afirma que Read nunca se bloquea.
- `impacto_hook.py` avisa literales compartidos antes de editar `.py`. Está registrado dos veces en PreToolUse y dos en PostToolUse.
- `mmorch.docgen --check` detecta solo catálogo y README viejos.
- `/verify-cross` da un verificador independiente con razonamiento.
- Ningún hook mira lecturas. Ningún chequeo cubre referencias muertas en `.md`.

## Propuesta, ordenada por valor medido sobre costo

1. Oración de precedencia. Existe. Medido: 4.2% a 91.4% de seguimiento del código.
2. Chequeo determinista de referencias muertas. Falta construir. Un script de unas 30 líneas lista cada `.md` que nombra una ruta o módulo inexistente. Corre en `docgen --check` y en un hook PostToolUse de Read.
3. Carpeta `archive/` con bloqueo de lectura. Falta construir. Recibe esfuerzos resueltos y `docs/production-readiness/`. Un `.ignore` la saca de rg, y un hook PreToolUse sobre Read, Grep y Glob la bloquea.
4. Skill sweep. Falta construir; sus piezas existen. Por cada ítem viejo revisa `git log`, `bd show` y el código actual. Después pide veredicto a `/verify-cross` y propone archivar. El humano aprueba cada movimiento.
5. Sesión nueva por tarea. Existe a medias: `session-handoff` y `hand_off_to_session`. Falta una regla de una línea.
6. Aviso por fecha contra el último commit del código nombrado. Conviene postergarlo. Da falsos positivos en notas de research, y el chequeo 2 ya cubre el caso grave.

## Riesgos y huecos

- Un hook sobre Read no frena `cat` ni `grep` por Bash. Hace falta un matcher de Bash para cerrar el hueco.
- Falta verificar que el Grep de Claude Code respeta `.ignore`.
- El chequeo 2 ve rutas con prefijo. No ve nombres sueltos como `babel.py`.
- Ninguna de estas medidas tiene línea base en este setup. Medir antes de escalar.
