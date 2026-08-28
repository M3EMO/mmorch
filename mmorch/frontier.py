"""Frontera de temas — rompe el círculo cerrado del auto-descubrimiento.

Problema: las queries de discovery salen del roadmap, de intereses.txt y del
foco de la reflexión. Las tres son la cabeza del sistema: ninguna puede
nombrar un tema que el sistema no conoce. Un LLM tampoco (su prior es otro
círculo cerrado, y no sabe nada acuñado después de su cutoff).

Solución: estructura EXÓGENA. Cada resultado de la GitHub Search API trae
`topics[]` — etiquetas puestas por miles de maintainers ajenos. Acumulando
co-ocurrencias se arma un grafo cuya frontera (vecinos a 1 salto de lo que ya
conocemos, que NO conocemos) contiene temas reales que nadie de acá escribió.

Ranking por PMI, no por frecuencia: la frecuencia cruda devuelve "python" y
"deep-learning"; PMI alto con frecuencia moderada devuelve el nicho específico
adyacente. Los datos ya pasaban por el pipeline y se tiraban.

Ref: co-word analysis (Callon 1983); frontier expansion sobre grafo de tags.
"""

from __future__ import annotations

import math
from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_STORE = "topic_graph.json"
_MIN_DF = 3          # menos que esto es ruido/typo de un solo repo
_MAX_DF_FRAC = 0.25  # más que esto es un tema paraguas ("python")
_STOP = {"python", "machine-learning", "deep-learning", "ai", "python3",
         "hacktoberfest", "awesome", "awesome-list", "llm", "pytorch"}


def _key(a: str, b: str) -> str:
    return "|".join(sorted((a, b)))


def absorb(items: list[dict], *, logs_dir: str, own: bool = False) -> dict:
    """Suma los `topics[]` de resultados de búsqueda al grafo.

    `own=True` marca esos temas como propios (repos que pasaron los filtros y
    fueron encolados) — el archivo contra el cual se define la frontera."""
    path = Path(logs_dir) / _STORE
    g = load_json_tolerant(path, {"nodes": {}, "pairs": {}, "docs": 0, "own": []})
    own_set = set(g.get("own", []))
    for item in items:
        tops = [t for t in (item.get("topics") or []) if t and t not in _STOP]
        if not tops:
            continue
        g["docs"] = g.get("docs", 0) + 1
        for i, a in enumerate(tops):
            g["nodes"][a] = g["nodes"].get(a, 0) + 1
            if own:
                own_set.add(a)
            for b in tops[i + 1:]:
                k = _key(a, b)
                g["pairs"][k] = g["pairs"].get(k, 0) + 1
    g["own"] = sorted(own_set)
    atomic_write_json(path, g)
    return {"docs": g["docs"], "nodes": len(g["nodes"]), "own": len(g["own"])}


def frontier(*, logs_dir: str, k: int = 5, exclude: set[str] | None = None) -> list[str]:
    """Temas a 1 salto de lo propio que NO son propios, rankeados por PMI.

    PMI(a,b) = log( p(a,b) / (p(a)·p(b)) ). Alto = co-ocurren mucho más de lo
    que su frecuencia individual explicaría, o sea: adyacencia real, no moda.
    """
    g = load_json_tolerant(Path(logs_dir) / _STORE, {})
    nodes, pairs = g.get("nodes", {}), g.get("pairs", {})
    docs = g.get("docs", 0)
    own = set(g.get("own", ()))
    if docs < 5 or not own:
        return []  # grafo tibio: mejor callarse que inventar
    skip = own | (exclude or set()) | _STOP
    cap = max(_MIN_DF + 1, int(docs * _MAX_DF_FRAC))

    best: dict[str, float] = {}
    for key, n_ab in pairs.items():
        a, b = key.split("|")
        for cand, anchor in ((a, b), (b, a)):
            if cand in skip or anchor not in own:
                continue
            df = nodes.get(cand, 0)
            if df < _MIN_DF or df > cap:
                continue
            pmi = math.log((n_ab / docs) / ((df / docs) * (nodes[anchor] / docs)))
            if pmi > best.get(cand, -1e9):
                best[cand] = pmi
    return [t for t, _ in sorted(best.items(), key=lambda kv: -kv[1])[:k]]


def _demo() -> None:
    """Self-check: un tema adyacente frecuente-pero-no-propio debe emerger."""
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        propios = [{"topics": ["rlhf", "dpo"]} for _ in range(4)]
        absorb(propios, logs_dir=d, own=True)
        # 'reward-modeling' co-ocurre con lo propio; 'webdev' no toca nada nuestro
        vecinos = [{"topics": ["dpo", "reward-modeling"]} for _ in range(4)]
        vecinos += [{"topics": ["webdev", "css"]} for _ in range(4)]
        absorb(vecinos, logs_dir=d)
        out = frontier(logs_dir=d, k=3)
        assert "reward-modeling" in out, out
        assert "webdev" not in out, out
        assert frontier(logs_dir=d, k=3, exclude={"reward-modeling"}) == [], "exclude"
        print("frontier ok:", out)


if __name__ == "__main__":
    _demo()
