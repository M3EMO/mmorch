# Wiring de /new-project y CLAUDE.md global al vault

Type: task
Status: resolved
Blocked by: 03

## Question

Con el contrato de escritura decidido: actualizar el skill /new-project (los
proyectos nuevos nacen apuntando al vault, no a research/ local) y agregar la
convención de lectura/escritura al CLAUDE.md global. Task, no decisión — pero
bloqueada hasta que el contrato exista.

## Answer

Task ejecutada 2026-08-03:

1. **Skill /new-project** (`~/.claude/skills/new-project/SKILL.md`): sección nueva
   "Research → vault global (NO local)" — proyectos nuevos sin carpeta de research;
   escritura vía `mmorch_vault_write` (fallback path directo + template), lectura
   híbrida; el CLAUDE.md del proyecto nuevo lleva una línea recordatoria.
2. **CLAUDE.md global** (`~/.claude/CLAUDE.md`): sección "Knowledge vault global" —
   la convención completa en 8 líneas (escritura, lectura, babel/lexicon, link a
   las decisiones del mapa).

Nota: `mmorch_vault_write` aún no existe (es ítem de la spec); el fallback de path
directo documentado funciona HOY. Cuando la spec se implemente, el tool pasa a ser
el camino primario sin tocar estos docs.
