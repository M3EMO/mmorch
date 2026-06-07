# mmorch — Innovation roadmap (auto-generado) — 2026-06-07

mmorch se innovó a sí mismo usándose a sí mismo. No es auto-fix (parchar bugs) —
es ideación de capacidades NUEVAS, filtradas adversarialmente.

**Pipeline:** Opus (observar) → `fan_out` DeepSeek 5 lentes (divergir, ~25 ideas) →
`adversarial_verify` Gemini cross-family (filtrar/refutar) → Opus (tie-break, rankear).
**Costo externo:** ~USD 0.006. **Cero cupo** en gen/verify. Gate: el test suite (18 tests).

## Cómo el filtro cross-family agregó señal
- **DISTILL (generate-and-filter) → REFUTADA** (Gemini conf 0.90): "es solo fan_out+verify
  renombrado" (redundante). Descartada. Un modelo solo la habría aceptado.
- **AUTO-TUNER → refutada parcial** (conf 0.80): el quality-proxy podría no ser robusto →
  auto-elegir barato que degrada calidad. Sobrevive PERO como recomendación gated, no
  auto-switch ciego. (Esta crítica moldeó el diseño de `learn`.)

## Roadmap rankeado (post-filtro)

| # | Capacidad | Estado | Valor | Ambición |
|---|---|---|---|---|
| **I-1** | `learn` — aprende de su propio metrics.jsonl, recomienda defaults más baratos (gated) | **LANDED** (read-only) | inteligencia (mejora con el uso) | rasputin-lite |
| **I-2** | `route` — barato responde + self-score; escala (escalate=True) solo si baja confianza | **LANDED** | ahorra cupo directo | incremental |
| **I-3** | `ensemble_verify` — K escépticos cross-family + voto mayoría (empate→falla) | **LANDED** | atrapa lo que 1 verificador no ve | media |
| **I-4** | `memo` — cache content-hash (Memo + memoized_verify); salta re-verify idéntico | **LANDED** | ahorra cupo + $ | media |
| **I-5** | `innovate` — motor ideate→screen→rank reutilizable (este mismo loop, productizado) | **LANDED** | auto-evolución | rasputin |
| ✗ | `distill` (generate-and-filter) | descartado | redundante (refutado cross-family) | — |
| ✗ | echo_chamber_break / drift_detector | aplazado | necesitan infra embeddings, ROI bajo | — |

## LANDED este ciclo (I-1 `learn` + cierre de gap auto-detectado)

**`mmorch/learn.py`** — `analyze()` (costo/latencia/uso por modelo×patrón) + `recommend()`
(acciones gated). Corrido sobre las 154 calls reales del log, descubrió:
1. **gemini-2.5-flash usado como generador fan_out cuesta 35.3x/call vs deepseek-chat** —
   inefficiency real (bulk mal-ruteado a Gemini). Recomendación: deepseek para bulk.
2. gemini fan_out p95 = 44.5s (alto).
3. **GAP auto-detectado:** `adversarial_verify` no loggeaba el verdict → sin proxy de
   calidad → I-1 no podía auto-tunear con fundamento.

**Evolución auto-referencial:** el sistema identificó qué le faltaba para evolucionar más,
y se cerró ese gap en el mismo ciclo: `adversarial_verify` ahora loggea
`adversarial_verify_verdict` (passed/confidence/n_refutations). Con uso, `learn` tendrá
el proxy de calidad por verificador → habilita I-1 completo (tuning fundamentado).

## Red de seguridad (prerequisito de cualquier auto-evolución)
Test suite nuevo (`tests/`, 18 tests, API mockeada) cubre invariantes: OneFlow (cross-family
raise), anti-sicofancia (`passed:"false"`→False), fan_out graceful, observabilidad de errores
y verdict, cost math, key-gating. **Sin este gate no se promueve código nuevo a vivo.**

## Próximo paso sugerido
I-2 `route` (mayor ROI de cupo, incremental) o I-4 `memo` (cache). Ambos aterrizables sobre
el core actual. El loop de innovación (I-5) se puede correr periódicamente — cada vez que
corra, `learn` tendrá más datos y el roadmap se afina solo.
