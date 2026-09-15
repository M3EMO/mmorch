---
title: "Lotus: cierre del repo y migracion a mmorch — 2026-09-15"
created: 2026-09-15
tags: [research, mmorch, lotus, poda, decision]
status: applied
confidence: 0.9
applies_to:
- orchestration
sources: [Desktop/Claude/Lotus (45 commits, ultimo 2026-09-04, sin remoto), logs/metrics.jsonl, mmorch/server.py]
---

# Lotus: cierre del repo y migracion a mmorch — 2026-09-15

## Decision

El usuario decidio no usar Lotus (cliente Tauri/JS vanilla del server de mmorch), migrar lo que
se use a mmorch y borrar el repo. Respaldo: `~/.claude/backups/lotus-2026-09-15.bundle` (git bundle,
45 commits, restaurable con `git clone <bundle>`).

## Medicion previa (que se usaba)

- `logs/metrics.jsonl`, 90 dias: patron `chat` 20 llamadas, todas del dogfood del 2026-08-31. Ningun
  otro patron de uso propio de Lotus.
- Sin log de accesos del server, sin tarea programada, sin remoto.
- La funcionalidad ya vivia en mmorch; Lotus solo era la interfaz (~6k lineas JS).

## Que queda en mmorch (tiene otro consumidor)

- `curation` con `/pending` y `/verdict`: scripts manana, smoke y veredicto; alimenta el flywheel.
- `transcript_store`: server_engine escribe y `feedback_trace.record_vote` lee. Se borro solo su GET.
- `/jobs/outcomes`, checkpoints, resume/pause, `/run/workflow`: sostienen produccion sin cliente.

## Que se borro (unico consumidor = Lotus)

- Modulos: `chat_store`, `minds`, `gate_policy`, `pty_session`, `server_pty` (430 lineas + tests).
- Rutas: `/chat`, `/chat/history`, `/minds`, `/benchmarks`, `/transcript/{id}`, `/jobs/{id}/gate`,
  `/jobs/{id}/gate/advance`, `/pty/*` (5), montaje estatico `/lotus`.
- `_GATES` de server_core.

## Decisiones del roadmap H1 que siguen valiendo (ya sin UI)

- "Nunca aprobar a ciegas: gate sin diff renderizable es bug de UX" -> hoy lo cumple el review de
  Claude del SDLC (bloquea con test, diff visible en la rama) y `curation` (veredicto con evidencia).
- Los tres campos que faltaban en el gate (diff, veredicto, costo estimado) los produce el driver
  del SDLC en `docs/sdlc/run-log.json` de cada corrida.
- Push accionable (ntfy) y digest matinal: siguen sin emisor; si vuelven, van como cableo cerrado
  al nightly (regla de poda: cerrado, automatico, recurrente), no como app.
- Sin app mobile nativa: decision que se mantiene por ausencia de uso.

## Relacion con el mapa SDLC

El ticket 06 del roadmap de Lotus (Inbox de decisiones) queda cubierto por el ticket 08 del mapa
SDLC (subjetivo -> sintetizado): el juicio humano se captura como ejemplos etiquetados, no como
pantalla.
