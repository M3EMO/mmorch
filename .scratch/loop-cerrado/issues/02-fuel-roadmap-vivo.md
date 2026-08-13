# 02 — Fuel: roadmap vivo

Type: grilling
Status: resolved
Blocked by: 01

## Question

Cómo se renueva la fuente de ideas para que innovate no idee contra un roadmap
muerto: ¿el roadmap se DERIVA automáticamente (sesiones ingestadas + open loops
+ notas del vault + beads abiertos) o se cura a mano con asistencia? ¿Formato y
dónde vive? ¿Cadencia de refresh (nightly)? ¿Qué pasa con el roadmap de junio y
evolve_open_prs vacío — migrar o deprecar?

## Answer

Grilling 2026-08-12. Modelo: **curado con asistencia, dos artefactos**:

- **`roadmap.md`** (curado): solo cambia con OK del usuario. El ARCHIVO es la
  fuente de verdad; cualquier vía que lo mueva vale (edición manual, "dale la 2"
  en sesión). El nightly detecta diffs y registra outcomes por comparación —
  no hay un único camino de promoción obligatorio.
- **`candidatos.md`** (derivado, versionado): el nightly genera direcciones EN
  BATCH — innovate no va de a una idea. Sesión/digest solo AVISAN ("hay N
  nuevas"), el batch entero vive en el archivo.

Generación: **lentes fijos gateados por fuel nuevo** — cada candidata sale de
un lente distinto (deuda técnica / nueva capacidad / integración entre
proyectos / notas huérfanas), máx 5 por ciclo, y solo corre si hubo material
nuevo desde el último ciclo (sesiones ingestadas / notas / open loops / beads);
noches sin fuel = 0 candidatas, sin ruido. Dedup contra TODO lo visto
(candidatas vigentes + rechazadas + roadmap).

Higiene: `INNOVATION_ROADMAP_2026-06-07.md` se **archiva como histórico** y
`roadmap.md` arranca de cero. Candidatas sin tocar **expiran a los 14 días** →
se archivan como rechazo blando y alimentan el bandit.
