# Sync multi-máquina del vault

Type: grilling
Status: resolved

## Question

El vault viaja hoy dentro del repo git de mmorch. ¿Alcanza (push/pull manual
como ahora), o el vault necesita sync propio — p.ej. vía `sync.py` (GitHub como
bus, ya existe) o el patrón `portability.py`? Decidir para el caso real: pc-mateo
always-on + notebook. Incluye qué pasa con conflictos de edición Obsidian.

## Answer

Grilling 2026-08-03:

1. **sync.py tal cual** — el vault viaja por el bus existente: nightly (y/o el write
   async) hace `commit_push` del vault a la branch `mmorch/auto`; las demás máquinas
   auto-pull ff-only con árbol limpio; el humano mergea en lote. Sin infraestructura
   nueva, invariante un-escritor/humano-mergea intacto. La fricción (notas esperan
   merge) se acepta.

2. **Conflictos: ff-only avisa** — lo ya codificado en sync.py: divergencia no se
   mergea a ciegas, se reporta y se resuelve a mano. Caso raro con un solo humano;
   no se construye auto-merge por nota.

Para la spec: agregar el vault al alcance del commit_push nocturno (hoy el nightly no
pushea el vault explícitamente) — un llamado a sync.commit_push con paths del vault.
