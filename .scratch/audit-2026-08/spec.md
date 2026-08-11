# Auditoría mmorch 2026-08-10 — backlog consolidado (4 ejes)

Origen: 4 sesiones paralelas de auditoría read-only (robustez, seguridad, eficiencia,
coherencia), protocolo gates→greps→lectura dirigida, todo hallazgo verificado con
`mmorch_adversarial_verify` (cross-family, refutar por default) + 2 rondas secas por eje.

Informes completos por eje: `.scratch/audit-{robustez,seguridad,eficiencia,coherencia}-2026-08-10.md`
Handoffs: `.scratch/audit-handoff-{eje}.md`

## Resultado global

- **BLOCKER: 0** · **IMPORTANTE: 13** · **NICE-TO-HAVE: 9** (10 hallazgos, 2 mergeados en ticket 15) · Descartados: 8 (ver apéndices de cada informe)
- Gates ruff/mypy en 0 en los 4 ejes (sin regresión).

## Tickets (ranking: severidad primero, a igual severidad menor esfuerzo primero)

IMPORTANTE — esfuerzo S:
- 01 readers .jsonl sin tolerancia por línea (robustez)
- 02 verifier legacy en cache/ensemble defaults (eficiencia)
- 03 ensemble_verify serial (eficiencia)
- 04 exec_embedder sin enforce_policy (seguridad)
- 05 CLAUDE.md drift del contrato (coherencia)
- 06 doble fuente de verdad de issues (coherencia)

IMPORTANTE — esfuerzo M:
- 07 budget_policy falla abierto (robustez)
- 08 lock de PRs nocturno se pierde ante corrupción (robustez)
- 09 watermark distill_upto no atómico, 2 escritores (robustez)
- 10 estado de bandits: reset silencioso + carrera (robustez)
- 11 vault.write_note sobreescribe en silencio (robustez)
- 12 test_cmd del planner LLM a shell=True sin allowlist (seguridad)
- 13 hot-path re-parsea metrics.jsonl completo por call (eficiencia)

NICE-TO-HAVE:
- 14 checkpoints best-effort sin señal (robustez)
- 15 memo cache: reset silencioso + rewrite por put + sin singleton (robustez+eficiencia, R7+E5)
- 16 DuckDB re-migra schema por operación (eficiencia)
- 17 hook Stop parsea transcript entero por turno (eficiencia)
- 18 handlers sync bloquean event loop (eficiencia)
- 19 projects.json contaminado, sin GC (coherencia)
- 20 Lotus sin CLAUDE.md de contrato (coherencia)
- 21 recall rerank sin matmul vectorizado (eficiencia)
- 22 regenerate_moc relee todo el vault por write (eficiencia)

## Señales operativas (fuera de tickets, ver handoffs)

- Task `mmorch-nightly` caída hace 4 noches (0x800710E0), sin alerta por ausencia.
- Verificador adversarial (gemini-flash-lite) alucinó código inexistente en 3/11
  verificaciones de robustez — reconfirma: review LLM necesita ground-truth en código.
- Una línea truncada en metrics.jsonl bloquearía toda call API con budget seteado
  (fail-closed de budget.check) — cubierto por tickets 01 y 07.
