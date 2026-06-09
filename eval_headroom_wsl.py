"""eval_headroom_wsl — A/B real en WSL (headroom._core funciona acá). Mide reducción de
tokens Y degradación de calidad sobre contexto REAL (no duplicado), preguntando a deepseek
con contexto FULL vs COMPRIMIDO. Calidad = substring esperado en la respuesta.

Corre con el venv WSL que tiene headroom: ~/hrvenv/bin/python eval_headroom_wsl.py
Lee la DEEPSEEK_API_KEY del .env del repo (montado en /mnt/c/...).
"""
import os
import pathlib
import headroom
from openai import OpenAI

REPO = pathlib.Path("/mnt/c/Users/map12/.claude/orchestration")


def _key():
    for ln in (REPO / ".env").read_text().splitlines():
        if ln.startswith("DEEPSEEK_API_KEY"):
            return ln.split("=", 1)[1].strip()
    raise SystemExit("no DEEPSEEK_API_KEY en .env")


client = OpenAI(api_key=_key(), base_url="https://api.deepseek.com/v1")


def _read(rel, n=18000):
    return (REPO / rel).read_text()[:n]


def samples():
    # contexto REAL no duplicado: un tool-output voluminoso + pregunta con respuesta puntual adentro
    metrics = _read("logs/metrics.jsonl", 18000)
    evolve = _read("mmorch/evolve.py", 18000)
    checkers = _read("mmorch/checkers.py", 18000)
    goal = _read("GOAL.md", 8000)
    return [
        {"ctx": evolve, "q": "Como se llama la funcion que clasifica un cambio por zona de riesgo? Solo el nombre.", "exp": "zone_of"},
        {"ctx": checkers, "q": "Que algoritmo usa el checker de determinante? Una palabra.", "exp": "Bareiss"},
        {"ctx": goal, "q": "Editar el GOAL es de que zona? Una palabra (color).", "exp": "roja"},
        {"ctx": metrics, "q": "En estos logs JSON, que familia de modelo aparece ademas de google? Un nombre.", "exp": "deepseek"},
        {"ctx": metrics, "q": "En estos logs JSON, nombra un modelo concreto que aparezca. Ej formato 'x-y'.", "exp": "deepseek-chat"},
        {"ctx": metrics, "q": "En estos logs JSON, cual es el nombre del campo que guarda el costo en dolares?", "exp": "cost_usd"},
    ]


def msgs(ctx, q):
    # contexto como TOOL output (headroom lo comprime; el user message lo protege)
    return [
        {"role": "tool", "content": ctx, "tool_call_id": "t1", "name": "read_file"},
        {"role": "user", "content": q + " Responde SOLO el dato, breve."},
    ]


def _flatten(messages):
    # deepseek no acepta role 'tool' suelto -> aplanar a un solo user message preservando contenido
    parts = []
    for m in messages:
        c = m.get("content")
        if isinstance(c, list):
            c = " ".join(str(x.get("text", x)) for x in c)
        parts.append(f"[{m.get('role')}] {c}")
    return [{"role": "user", "content": "\n\n".join(parts)}]


def ask(messages):
    r = client.chat.completions.create(model="deepseek-chat", messages=_flatten(messages),
                                       temperature=0.0, max_tokens=80)
    return r.choices[0].message.content, r.usage.prompt_tokens


def main():
    limit = int(os.environ.get("HR_LIMIT", "1500"))
    print(f"A/B headroom (WSL, _core OK) | model_limit={limit}\n")
    tb = ta = qf = qc = deg = 0
    print("%-52s %8s %8s  full comp" % ("pregunta", "tok_pre", "tok_post"))
    for s in samples():
        full = msgs(s["ctx"], s["q"])
        cr = headroom.compress(full, model_limit=limit)
        comp = cr.messages
        a_full, pf = ask(full)
        a_comp, pc = ask(comp)
        okf = s["exp"].lower() in a_full.lower()
        okc = s["exp"].lower() in a_comp.lower()
        tb += cr.tokens_before; ta += cr.tokens_after
        qf += okf; qc += okc
        deg += 1 if (okf and not okc) else 0
        print("%-52s %8d %8d   %s    %s" % (s["q"][:52], cr.tokens_before, cr.tokens_after,
              "OK" if okf else "x", "OK" if okc else "x"))
    red = 100 * (1 - ta / tb) if tb else 0
    print("\n" + "=" * 64)
    print(f"REDUCCION TOKENS: {red:.1f}%  ({tb} -> {ta})")
    n=len(samples())
    print(f"CALIDAD: full {qf}/{n} | compressed {qc}/{n}   DEGRADACIONES: {deg}/{n}")
    if deg == 0 and red > 5:
        print(f"VEREDICTO: comprime {red:.0f}% SIN degradar (en estas muestras).")
    elif deg:
        print(f"VEREDICTO: {deg} degradacion(es) -> pierde info que el modelo necesitaba.")


if __name__ == "__main__":
    main()
