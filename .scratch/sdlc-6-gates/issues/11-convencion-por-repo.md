# Convención por repo (la arquitectura de proyectos)
Type: grilling
Status: open
Blocked by: 02
Map: ../map.md

## Question

El usuario quiere que TODOS sus proyectos corran con el pipeline, no solo mmorch. Decidir qué lleva cada repo para ser "un proyecto robusto de una": `docs/sdlc/` con plantillas de intent/spec/plan, los `GATE-N.md` con sus chequeos y comandos, el comando de aceptación declarado (dónde: CLAUDE.md, un `sdlc.toml`, o el registro de proyectos de mmorch), los hooks que aplican (never-edit, ETS, gates pre-commit), y cómo lo scaffoldea `/new-project` para un repo nuevo. ¿Qué es obligatorio y qué opcional? ¿Cómo se versiona el contrato del repo contra el engine de mmorch? ¿Qué pasa con un repo sin test de aceptación ejecutable (¿entra o no)? Salida: la convención escrita como spec del scaffold, con un ejemplo real.
