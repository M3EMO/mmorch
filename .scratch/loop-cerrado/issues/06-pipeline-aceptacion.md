# 06 — Pipeline de idea aceptada

Type: grilling
Status: resolved
Blocked by: 04

## Question

Criterio y mecánica de "chica → task chip inmediato / grande-foggy → issue
beads + nudge wayfinder": quién clasifica el tamaño (¿el mismo modelo barato en
la adjudicación, con override del usuario?), qué campos lleva el issue beads
(prompt autónomo con paths), y cómo se linkea de vuelta a la nota origen para
que el outcome final (se hizo / se abandonó) cierre el ciclo.

Restricción fijada por el usuario (2026-08-12): la ejecución de una idea
aceptada corre SIEMPRE en sandbox — worktree/branch aislado, nunca sobre main
ni sobre el working tree del usuario. Merge solo con revisión humana. Queda por
decidir acá la mecánica exacta (git worktree vs branch, cómo se presenta el
diff, quién limpia sandboxes abandonados).

## Answer

Grilling 2026-08-12.

- **Clasificación de tamaño: la sesión decide al "dale"** — Claude de la sesión
  evalúa con `mmorch_cynefin` (cero cupo) y contexto fresco; nada pre-etiquetado.
- **Sandbox: reusar el mecanismo del engine /project** — worktree aislado +
  review branch. Chica→chip: la sesión spawneada corre en su worktree, entrega
  review branch + resumen de diff; merge solo humano. Grande→beads: el issue
  lleva prompt autónomo; al agarrarlo, mismo mecanismo (wayfinder si foggy →
  /project → worktree).
- **Limpieza**: nightly poda worktrees/branches abandonados a los 14 días,
  registrando rechazo blando si nunca mergeó.
- **Issue beads**: título imperativo + prompt autónomo (paths, link a nota
  origen, score y justificación) + id de propuesta.
- **Doble señal al bandit**: "dale" = aceptación (1.0); al TERMINAR segundo
  evento — mergeada 1.0 `source=merge` (patrón reap_merged_prs), abandonada/
  podada 0.2. Distingue "me entusiasmó" de "realmente sirvió".
- **Nota origen** pasa a `status: applied` en frontmatter cuando mergea.
