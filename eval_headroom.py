"""eval_headroom — harness A/B: ¿headroom ahorra tokens SIN degradar calidad?

La gente mide la reducción de tokens (fácil) y NO mide la degradación de calidad (lo que
importa). Esto mide AMBOS sobre TU contenido real:
  - tokens(full) vs tokens(compressed)  -> % reducción.
  - calidad DETERMINISTA: cada muestra trae un `expected` (substring que una respuesta
    correcta DEBE contener). Se le pregunta al modelo barato con contexto FULL y con
    contexto COMPRIMIDO; se chequea si `expected` aparece en cada respuesta. Si full
    acierta y compressed falla = DEGRADACIÓN (lo que querés cazar).

Sin headroom instalado: corre el baseline (compresión = identidad) y muestra el setup.
Con headroom: comprime de verdad y compara. Gasta $ API (deepseek, barato), gateado por
BudgetKeeper. Uso: python eval_headroom.py [--n 5] [--model deepseek-chat] [--yes]
"""
from __future__ import annotations

import argparse
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.providers import call

ROOT = pathlib.Path(__file__).resolve().parent


def _read(rel: str, head: int = 4000) -> str:
    p = ROOT / rel
    return p.read_text(encoding="utf-8")[:head] if p.exists() else ""


def build_samples() -> list[dict]:
    """(contexto real, pregunta, expected substring que una respuesta correcta DEBE tener)."""
    s = []
    budget = _read("mmorch/budget.py")
    if budget:
        s.append({"ctx": budget, "q": "¿Qué env var configura el límite mensual? Solo el nombre.",
                  "expected": "MMORCH_MAX_MONTHLY_USD"})
        s.append({"ctx": budget, "q": "¿Qué clase de excepción lanza check() al exceder el límite? Una palabra.",
                  "expected": "BudgetExceeded"})
    goal = _read("GOAL.md")
    if goal:
        s.append({"ctx": goal, "q": "¿Editar el GOAL es de qué zona? Responde una palabra.",
                  "expected": "roja"})
    checkers = _read("mmorch/checkers.py", 6000)
    if checkers:
        s.append({"ctx": checkers, "q": "¿Qué algoritmo usa el checker de determinante? Una palabra.",
                  "expected": "Bareiss"})
    prices = _read("mmorch/prices.py")
    if prices:
        s.append({"ctx": prices, "q": "¿De qué archivo lee los precios el override? Solo el nombre.",
                  "expected": "prices.json"})
    return s


def _compressor():
    """Devuelve (compress_fn, disponible). Intenta varias APIs de headroom; si no, identidad."""
    try:
        import headroom  # noqa
        for name in ("compress", "compress_text"):
            fn = getattr(headroom, name, None)
            if callable(fn):
                return (lambda t: str(fn(t))), True
        # API por submódulo
        try:
            from headroom import api  # type: ignore
            if hasattr(api, "compress"):
                return (lambda t: str(api.compress(t))), True
        except Exception:
            pass
    except Exception:
        pass
    return (lambda t: t), False     # baseline: sin compresión


def _ask(model: str, ctx: str, q: str) -> tuple[str, int]:
    r = call(model, [{"role": "system", "content": "Respondé SOLO con el dato pedido, breve."},
                     {"role": "user", "content": f"CONTEXTO:\n{ctx}\n\nPREGUNTA: {q}"}],
             pattern="eval_headroom", node="ask", phase="eval_headroom",
             temperature=0.0, max_tokens=120)
    return r.text, r.in_tokens


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=5)
    ap.add_argument("--model", default="deepseek-chat")
    ap.add_argument("--yes", action="store_true")
    args = ap.parse_args()

    compress, available = _compressor()
    samples = build_samples()[:args.n]
    print(f"headroom instalado: {available} | muestras: {len(samples)} | modelo: {args.model}")
    if not available:
        print("\n[BASELINE] headroom NO instalado -> compresión = identidad (reducción 0%).")
        print("Para comparar de verdad:")
        print("  pip install \"headroom-ai[all]\"   (luego re-corré este harness)")
    if not args.yes:
        print("\nCorrida real gasta API (deepseek, barato). Repetir con --yes.")
        return

    tok_full = tok_comp = 0
    q_full = q_comp = degraded = 0
    print("\n%-40s %8s %8s  full comp" % ("pregunta", "tok_full", "tok_comp"))
    for s in samples:
        comp_ctx = compress(s["ctx"])
        a_full, tf = _ask(args.model, s["ctx"], s["q"])
        a_comp, tc = _ask(args.model, comp_ctx, s["q"])
        ok_f = s["expected"].lower() in a_full.lower()
        ok_c = s["expected"].lower() in a_comp.lower()
        tok_full += tf; tok_comp += tc
        q_full += ok_f; q_comp += ok_c
        if ok_f and not ok_c:
            degraded += 1
        print("%-40s %8d %8d   %s    %s" % (s["q"][:40], tf, tc,
              "OK" if ok_f else "x", "OK" if ok_c else "x"))

    red = 100 * (1 - tok_comp / tok_full) if tok_full else 0
    print("\n" + "=" * 56)
    print(f"REDUCCIÓN DE TOKENS: {red:.1f}%  ({tok_full} -> {tok_comp})")
    print(f"CALIDAD: full {q_full}/{len(samples)} | compressed {q_comp}/{len(samples)}")
    print(f"DEGRADACIONES (full OK, compressed FALLA): {degraded}/{len(samples)}")
    if not available:
        print("\n(baseline sin headroom: reducción 0%, calidad = referencia. Instalá headroom y re-corré.)")
    elif degraded == 0:
        print("\nVEREDICTO: comprime sin degradar en estas muestras -> candidato a quedarse.")
    else:
        print(f"\nVEREDICTO: {degraded} degradación(es) -> headroom pierde info que el modelo necesitaba. Cuidado.")


if __name__ == "__main__":
    main()
