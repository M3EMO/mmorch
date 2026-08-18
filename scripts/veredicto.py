"""Veredicto humano con un comando — la señal GOLD del flywheel.

Uso (desde el repo, con el venv):
  python scripts/veredicto.py            -> lista visual (colores, wrap)
  python scripts/veredicto.py -i         -> modo interactivo (d/n/enter/q)
  python scripts/veredicto.py cand <id> dale|no
  python scripts/veredicto.py card <#|id> dale|no   (# = numero de la lista)
"""

import io
import os
import pathlib
import sys
import textwrap

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

C = {"reset": "\x1b[0m", "bold": "\x1b[1m", "dim": "\x1b[2m",
     "cyan": "\x1b[96m", "green": "\x1b[92m", "yellow": "\x1b[93m",
     "red": "\x1b[91m", "mag": "\x1b[95m", "blue": "\x1b[94m"}
LENTE_COLOR = {"deuda": "yellow", "capacidad": "green",
               "integracion": "cyan", "notas-huerfanas": "mag"}


def _wrap(text: str, width: int = 100, indent: str = "    ") -> str:
    return "\n".join(textwrap.wrap(text, width=width, initial_indent=indent,
                                   subsequent_indent=indent))


def _bar(score: float) -> str:
    n = round(score * 10)
    color = "green" if score >= 0.9 else ("yellow" if score >= 0.8 else "dim")
    return C[color] + "█" * n + C["dim"] + "░" * (10 - n) + C["reset"]


def _items():
    from mmorch.curation import pending
    p = pending()
    return p["candidatas"], sorted(p["cards"], key=lambda x: -x["score"])


def listar() -> None:
    os.system("")  # habilita ANSI en la consola de Windows
    cands, cards = _items()
    print(f"\n{C['bold']}{C['blue']}══ CANDIDATAS ({len(cands)}) "
          f"{'═' * 50}{C['reset']}")
    for e in cands:
        lc = C[LENTE_COLOR.get(e["lente"], "dim")]
        mad = (f" {C['dim']}(madurada x{e['maduraciones']}){C['reset']}"
               if e.get("maduraciones") else "")
        print(f"\n{C['bold']}{e['id']}{C['reset']}  {lc}● {e['lente']}{C['reset']}"
              f"  {C['dim']}vence {e['vence']}{C['reset']}{mad}")
        print(_wrap(e["gist"]))
    print(f"\n{C['bold']}{C['blue']}══ TARJETAS nota→proyecto ({len(cards)}) "
          f"{'═' * 42}{C['reset']}")
    for n, m in enumerate(cards, 1):
        nota = pathlib.Path(m["note"]).stem[:50]
        print(f"\n{C['bold']}#{n}{C['reset']}  {_bar(m['score'])} {m['score']:.2f}"
              f"  {C['cyan']}{m['project']}{C['reset']} ← {C['dim']}{nota}{C['reset']}")
        print(_wrap(m["justification"][:300]))
    print(f"\n{C['dim']}veredictos: cand <id> dale|no · card <#> dale|no · "
          f"interactivo: -i{C['reset']}\n")


def interactivo() -> None:
    os.system("")
    from mmorch.curation import verdict_candidata, verdict_card
    cands, cards = _items()
    total = {"dale": 0, "no": 0, "skip": 0}
    prompt = f"  {C['bold']}[d]ale / [n]o / enter=saltar / q=salir >{C['reset']} "

    def _run(items, header, apply_fn):
        for it in items:
            print(header(it))
            r = input(prompt).strip().lower()
            if r == "q":
                return False
            if r == "d":
                print("  ", apply_fn(it["id"], "dale"))
                total["dale"] += 1
            elif r == "n":
                print("  ", apply_fn(it["id"], "no"))
                total["no"] += 1
            else:
                total["skip"] += 1
        return True

    def _cand_header(e):
        lc = C[LENTE_COLOR.get(e["lente"], "dim")]
        return (f"\n{C['bold']}CANDIDATA {e['id']}{C['reset']}  "
                f"{lc}● {e['lente']}{C['reset']}\n" + _wrap(e["gist"]))

    def _card_header(m):
        return (f"\n{C['bold']}TARJETA{C['reset']}  {_bar(m['score'])} "
                f"{m['score']:.2f}  {C['cyan']}{m['project']}{C['reset']} "
                f"← {m['note'][:50]}\n" + _wrap(m["justification"][:300]))

    if _run(cands, _cand_header, verdict_candidata):
        _run(cards, _card_header, verdict_card)
    print(f"\n{C['green']}dale: {total['dale']}{C['reset']} · "
          f"{C['red']}no: {total['no']}{C['reset']} · saltadas: {total['skip']}\n")


def cand(cid: str, verdict: str) -> None:
    from mmorch.curation import verdict_candidata
    print(verdict_candidata(cid, verdict))


def card(pid: str, verdict: str) -> None:
    from mmorch.curation import verdict_card
    if pid.isdigit():  # atajo por numero de la lista visual
        _, cards = _items()
        k = int(pid) - 1
        if 0 <= k < len(cards):
            pid = cards[k]["id"]
    print(verdict_card(pid, verdict))


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] == "listar":
        listar()
    elif sys.argv[1] == "-i":
        interactivo()
    elif sys.argv[1] == "cand" and len(sys.argv) == 4:
        cand(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == "card" and len(sys.argv) == 4:
        card(sys.argv[2], sys.argv[3])
    else:
        print(__doc__)


if __name__ == "__main__":
    main()
