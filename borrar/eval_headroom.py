"""eval_headroom — harness A/B: ¿headroom ahorra tokens SIN degradar calidad?

Mide AMBOS sobre TU contenido real:
  - tokens_before vs tokens_after (de headroom) -> % reducción real.
  - calidad DETERMINISTA: cada muestra trae `expected` (substring que una respuesta
    correcta DEBE contener). Se pregunta al modelo barato con contexto FULL y con
    contexto COMPRIMIDO; si full acierta y compressed FALLA = DEGRADACIÓN (lo que importa).

Fuerza la compresión con `--limit` bajo (model_limit) = stress test honesto de lo lossy.
Sin headroom: baseline identidad. Gasta $ API (deepseek), gateado por BudgetKeeper.
Uso: python eval_headroom.py [--n 6] [--limit 1200] [--model deepseek-chat] [--yes]
"""
from __future__ import annotations

import argparse
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.providers import call

ROOT = pathlib.Path(__file__).resolve().parent


def _read(rel: str, head: int = 100000) -> str:
    p = ROOT / rel
    return p.read_text(encoding="utf-8")[:head] if p.exists() else ""


def build_samples() -> list[dict]:
    """Contexto GRANDE (varios módulos) + pregunta con respuesta puntual adentro. Grande
    = la compresión engancha de verdad."""
    big = "\n\n".join(_read(f) for f in
                      ("mmorch/budget.py", "mmorch/goal.py", "mmorch/checkers.py",
                       "mmorch/evolve.py", "mmorch/prices.py", "GOAL.md"))
    return [
        {"ctx": big, "q": "¿Qué env var configura el límite mensual de gasto? Solo el nombre.",
         "expected": "MMORCH_MAX_MONTHLY_USD"},
        {"ctx": big, "q": "¿Qué clase de excepción lanza el budget check al exceder? Una palabra.",
         "expected": "BudgetExceeded"},
        {"ctx": big, "q": "¿Qué algoritmo usa el checker de determinante? Una palabra.",
         "expected": "Bareiss"},
        {"ctx": big, "q": "¿Cómo se llama la función que clasifica un cambio por zona de riesgo?",
         "expected": "zone_of"},
        {"ctx": big, "q": "¿De qué archivo JSON lee los precios el override?",
         "expected": "prices.json"},
        {"ctx": big, "q": "Editar el GOAL.md, ¿de qué zona es? Una palabra (color).",
         "expected": "roja"},
    ]


def _make_compressor(limit: int):
    """Devuelve (compress_messages_fn, disponible). compress_messages(msgs) ->
    (msgs_comprimidos, tok_before, tok_after)."""
    try:
        import headroom
    except Exception:
        return (lambda msgs: (msgs, None, None)), False

    def comp(msgs):
        try:
            r = headroom.compress(msgs, model_limit=limit)
            return r.messages, r.tokens_before, r.tokens_after
        except Exception as e:
            print("  (compress err:", str(e)[:60], "-> usa original)")
            return msgs, None, None
    return comp, True


def _ask(model: str, messages: list) -> tuple[str, int]:
    r = call(model, messages, pattern="eval_headroom", node="ask", phase="eval_headroom",
             temperature=0.0, max_tokens=120)
    return r.text, r.in_tokens


def _msgs(ctx: str, q: str) -> list:
    return [{"role": "system", "content": "Respondé SOLO con el dato pedido, breve."},
            {"role": "user", "content": f"CONTEXTO:\n{ctx}\n\nPREGUNTA: {q}"}]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=6)
    ap.add_argument("--limit", type=int, default=1200, help="model_limit (fuerza compresión)")
    ap.add_argument("--model", default="deepseek-chat")
    ap.add_argument("--yes", action="store_true")
    args = ap.parse_args()

    compress, available = _make_compressor(args.limit)
    samples = build_samples()[:args.n]
    print(f"headroom: {available} (0.20.15) | muestras: {len(samples)} | model_limit: {args.limit} | modelo: {args.model}")
    if not available:
        print("headroom NO importable -> baseline identidad.")
    if not args.yes:
        print("Corrida real gasta API (deepseek). Repetir con --yes.")
        return

    tb_sum = ta_sum = q_full = q_comp = degraded = 0
    print("\n%-44s %9s %9s  full comp" % ("pregunta", "tok_pre", "tok_post"))
    for s in samples:
        full_m = _msgs(s["ctx"], s["q"])
        comp_m, tb, ta = compress(full_m)
        a_full, _ = _ask(args.model, full_m)
        a_comp, _ = _ask(args.model, comp_m)
        ok_f = s["expected"].lower() in a_full.lower()
        ok_c = s["expected"].lower() in a_comp.lower()
        if tb and ta:
            tb_sum += tb; ta_sum += ta
        q_full += ok_f; q_comp += ok_c
        degraded += 1 if (ok_f and not ok_c) else 0
        print("%-44s %9s %9s   %s    %s" % (
            s["q"][:44], tb or "-", ta or "-", "OK" if ok_f else "x", "OK" if ok_c else "x"))

    red = 100 * (1 - ta_sum / tb_sum) if tb_sum else 0
    print("\n" + "=" * 60)
    print(f"REDUCCIÓN DE TOKENS (headroom): {red:.1f}%  ({tb_sum} -> {ta_sum})")
    print(f"CALIDAD: full {q_full}/{len(samples)} | compressed {q_comp}/{len(samples)}")
    print(f"DEGRADACIONES (full OK, compressed FALLA): {degraded}/{len(samples)}")
    if available and degraded == 0 and red > 5:
        print("\nVEREDICTO: comprime ~%.0f%% SIN degradar -> candidato fuerte." % red)
    elif available and degraded:
        print(f"\nVEREDICTO: {degraded} degradación(es) — pierde info que el modelo necesitaba. Cuidado.")


if __name__ == "__main__":
    main()
