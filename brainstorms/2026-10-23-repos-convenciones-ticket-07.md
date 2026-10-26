# Repos-convenciones, ticket 07 (orden de adopción y forma de aplicar): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir en qué orden se actualizan los repos y cómo llega el cambio a cada uno (`.scratch/repos-convenciones/issues/07-orden-de-adopcion.md`, bd `orchestration-4u6`).

Contexto heredado: `doctor` es el oráculo sin escrituras y reporta qué arreglos son seguros (03); `--upgrade` aplica solo la lista cerrada (03, 04); `--todos` descubre repos por `sdlc.toml` (02); solo la laptop corre `--upgrade` (02); Adepor se adopta con `init` dentro de este ticket (02).

## Summary / key decisions

- Test de aceptación aprobado con 16 puntos (P8). Ticket 07 resuelto.
- Adoptado = `doctor` sale con "sin desvíos" o "solo avisos". Adepor además necesita `accept_cmd` verde y sin red antes del `init`. Cada repo deja en el ticket 07 tres salidas: `doctor` antes, `--upgrade`, `doctor` después (P7).
- Orden: mf-moe (piloto, control negativo real); Portfolio, ssb, ChatBot; prototipos F2 y f2-sueldo-neto; Estudio; orchestration (tras escribir `rama`); Adepor último (sonda sin red, `init`, checkout de `main`) (P6).
- Rama default: clave `rama` de `sdlc.toml` si existe; si no, `origin/HEAD`; si falta, la única rama local `main` o `master`; si no, aborta. `rama` es fila nueva de la tabla con default en código; `--upgrade` nunca la escribe. orchestration necesita `rama = "mmorch/auto"` escrita a mano (P5).
- Rama: `--upgrade` commitea solo en la rama default del repo y aborta en otra rama, nombrando la rama esperada (P4).
- Árbol sucio: `--upgrade` aborta solo si un archivo que va a tocar está sucio (P2).
- `--upgrade` commitea solo los archivos que tocó (`git commit -- <rutas>`) y pushea siempre (P2, P3). Consecuencia: para un agente, correr `--upgrade` equivale a pushear, así que entra al tier ASK.
- `--todos` solo en `doctor`; `--upgrade` actúa sobre un repo por invocación (P1).

## Q&A log

### P1 — `--upgrade --todos`
- Asked: ¿A (lote sin confirmación), B (`--todos` solo en `doctor`) o C (lote con confirmación por repo)?
- Captured: el usuario eligió B.
- Decisión: un diff por repo, fácil de revisar; sin diálogo interactivo.

### P2 — Árbol sucio
- Asked: ¿A (abortar con cualquier archivo sucio), B (abortar solo si un archivo a tocar está sucio) o C (escribir igual)?
- Captured: el usuario respondió "B pero arma commits y push".
- Decisión: B. Además, el usuario quiere que `--upgrade` commitee y pushee.
- Tensión a resolver (P3): el CLAUDE.md del repo pone el push en tier ASK para agentes ("Never auto-push"). Si un agente corre `--upgrade`, un push automático salta ese guardrail.

### P3 — Push
- Asked: ¿A (push siempre), B (push solo con `--push`) o C (nunca push)?
- Captured: el usuario eligió A.
- Decisión: push siempre tras el commit. Casos de falla: repo sin remoto -> commit local y aviso; push rechazado -> commit local queda y el reporte lo dice; pre-commit que falla -> cambios sin commitear y el reporte lo dice.
- Consecuencia con el guardrail del repo (push en tier ASK): un agente pide OK antes de correr `--upgrade`.

### P4 — Rama del commit
- Asked: ¿A (rama actual), B (rama nueva `convenciones/<fecha>`) o C (solo la rama default; aborta en otra)?
- Captured: el usuario eligió C.
- Hechos (2026-10-23): rama actual vs `origin/HEAD` por repo: orchestration `mmorch/auto` vs `master`; Estudio `init` vs `init`; Portfolio `master` vs `master`; ChatBot `main` vs `main`; Adepor `audit/prod-ready-2026-10` vs `main`; ssb `master` vs `master`; mf-moe y los F2 sin `origin/HEAD` (`master` y `main` locales).
- Problema: en orchestration la rama viva es `mmorch/auto`, y `sdlc/` no existe en `master`. Pregunta P5.

### P5 — Cómo se conoce la rama default
- Asked: ¿A (`origin/HEAD` o abortar), B (`origin/HEAD` o la única `main`/`master` local) o C (clave `rama` en `sdlc.toml`, si no B)?
- Captured: el usuario eligió C.

### P6 — Orden de adopción
- Asked: ¿confirmas el orden propuesto?
- Captured: el usuario confirma.

### P7 — Qué cuenta como adoptado
- Asked: ¿A (veredicto de `doctor`) o B (A más una corrida real del pipeline)?
- Captured: el usuario eligió A.

### P8 — Test de aceptación
- Asked: ¿apruebas el test de 11 puntos? El usuario preguntó si faltaba algo.
- Captured: Claude agregó los puntos 12 a 16 (índice staged, sin commit vacío, mensaje fijo, sin `--force`, push sin prompt y con timeout) y amplió el 8 (HEAD separado aborta). El usuario aprobó "el test con los puntos 12 a 16".
- Decisión: test aprobado. La ampliación del punto 8 iba en la misma pregunta; el usuario no la nombró aparte.

## Open flags (pending input)
- `rama = "mmorch/auto"` en el `sdlc.toml` de orchestration: la escribe el usuario a mano (o un agente con su OK) antes de su `--upgrade`.
- Punto 8 ampliado (HEAD separado aborta): aprobado junto con el test, sin mención explícita -> usuario puede objetar.
