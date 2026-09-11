# Contrato de gate
Type: grilling
Status: resolved
Blocked by: 01
Map: ../map.md
Capture: `brainstorms/2026-09-10-sdlc-6-gates.md` (Q1–Q7)

## Question

Definir el contrato de gate como código y el formato `GATE-N.md` por etapa.

## Decisions

- Un gate es función Python registrada en mmorch. Puede correr un comando del repo. Salida única: `CheckResult`.
- Campos de `GATE-N.md`: id, etapa, clase (determinista | sintetizado | juicio), entrada, checker o comando, on_fail, measured_ref, alcance (unidad | integracion | aceptacion).
- Viven en `docs/sdlc/gates/` de cada repo. El archivo nombra; no copia el checker.
- Juez LLM no decide pass/fail en build/test/PR. Reasoner propone parche solo si el oráculo falló.
- Cada fallo: job + log. Aviso a persona solo al agotar vueltas/USD, o zona roja.
- Promoción: automática si hay oráculo y evidencia 3+1. Humana si no hay oráculo. Nunca por opinión de un modelo.
- `clase: juicio` solo en spec (`measured_ref: juicio-fijo`). Fallo: stop, sin parche automático.
- 4 del A/B: determinista. test-compile, archivos del plan, firmas vs test = unidad. Baseline intacto = integracion.
- Tres contratos: pipeline = GATE-N.md; módulo = tests; producto = spec + aceptación.
- Gate `trazabilidad` (agregado 2026-09-11, idea de spec-kit `analyze` sin LLM): cada requisito de `spec.md` lleva ID `R<n>`; cada unidad de `plan.md` cita los IDs que cubre; cada test de aceptacion cita un ID en su nombre. Falla si un ID queda huerfano en cualquier direccion. Clase determinista, alcance integracion, USD 0. Generaliza `plan-allowlist`.
