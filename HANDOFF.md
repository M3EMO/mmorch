# Handoff — 2026-09-17

## Goal

mmorch = orquestador determinista multi-modelo que cuida el **cupo** del plan Claude. La etapa actual es el **SDLC de 6
etapas con gates**: el engine de `/project` que construye features en repos reales, en un worktree aislado, y deja una
review branch. Repo PUBLICO: `github.com/M3EMO/mmorch` (rama principal `mmorch/auto`; `origin/master` se avanza por
fast-forward). Nunca subir ahi memoria, config personal ni datos del usuario.

## Estado (2026-09-17)

- Suite: 1220 tests verdes; ruff y mypy en 0; el ratchet de capas y el gate anti-museo, verdes.
- Mapa wayfinder `.scratch/sdlc-6-gates/`: **sin tickets abiertos**. Cerrados 01..17; el 18 se cerro sin construir
  porque la medicion refuto su premisa.
- Repos adoptados y con feature mergeada: Estudio (`init`, TS/vitest), Portfolio (`master`, pytest). ChatBot (`main`,
  Maven) entra y tiene su feature verificada en `mmorch/wt-f8223a1e`, pendiente de merge; no tiene remoto.
  Proyecto_Adepor quedo fuera por decision del usuario.

## Gates del pipeline hoy

| Gate | Que hace |
|---|---|
| suite total | Sin fallos nuevos; el baseline se mide sin los tests de aceptacion (Java y TS no compilaban en la base) |
| lint y tipos | Sin hallazgos nuevos POR ARCHIVO contra la base |
| codigo muerto | Fraccion de un `.py` NUEVO que corre la aceptacion; `cobertura_min` 0.8 |
| mutacion | Solo sobre las lineas que cambio la feature; `mutation_min` bloquea, sin clave observa |
| revision | Claude bloquea con un test; en repos no Python ese test tambien se corre |
| sprites | Capa determinista de assets que bloquea; el juez visual solo observa (`[sprites]` en sdlc.toml) |

`reviewer_cmd` en `sdlc.toml` reemplaza a `claude -p` por cualquier comando (prompt por stdin, `{modo}`).
Cada job del server corre `build_feature` en su propio proceso, asi dos features avanzan en paralelo.

## Medido esta semana

- 4 corridas reales por el server: Estudio US$0.018 / 6 min; Portfolio shortfall US$0.067 / 41 min; Portfolio leadlag
  US$0.42 / 67 min; ChatBot US$0.014 + US$0.027. Cero intervenciones humanas dentro de las corridas.
- Refutador de tests ANTES de la aprobacion humana: 3 versiones, 25 corridas, **0 aciertos**. Los defectos reales se
  atraparon con el codigo delante. Banco reusable en `mmorch/refutacion.py` (`mmorch cli refutacion`).
- Juez visual VLM: ordena (Pearson 0.459) pero puntua mal (32.1% exacto) y pierde acierto con imagenes chicas. Por eso
  compara de a pares, usa rubrica binaria y el sprite viaja agrandado x8.
- Capa determinista de sprites: 10 defectos inyectados, 10 atrapados, 0 falsos positivos.

## Lo que sigue (niebla del mapa)

- Etiquetar 50 sprites reales para medir kappa del juez visual: espera el primer juego.
- Ejecucion en contenedor declarada por repo en `sdlc.toml`, nunca por RAM libre.
- Canal movil para aprobar tests desde el telefono; volveria si esperar el veredicto frena corridas.
- Checker sintetizado de aceptacion: `logs/sdlc/veredictos.jsonl` tiene 8 veredictos reales, con solo 2 negativos.

## Pendiente del usuario

- Rotar `MMORCH_SERVER_TOKEN`: estuvo escrito en `HANDOFF.md` y `SETUP-HOST.md` de este repo publico hasta hoy.
- Mergear la rama `mmorch/wt-f8223a1e` de ChatBot, y decidir si ese repo tiene remoto.
- Copiar a mano a la otra PC: `.env` y `logs/refutacion/banco.json` (no van al repo).
