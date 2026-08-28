"""Bursts de arXiv — temas recién acuñados, que ningún tag todavía nombra.

Punto ciego del grafo de topics (mmorch/frontier.py): las etiquetas de GitHub
existen solo para lo ya establecido, así que el grafo encuentra
"establecido-pero-desconocido-para-vos" y es sordo a lo que nació el mes
pasado. Los bursts tienen el perfil de falla inverso: detectan que un término
aparece MUY por encima de su propia línea de base, sin saber nada de nuestros
intereses. Exógeno y sin cutoff — la única fuente que puede nombrar algo
acuñado después del training de cualquier LLM.

Mecanismo: bigramas de títulos recientes de cs.LG/cs.AI/cs.CL/cs.NE/stat.ML,
contados en cubetas semanales; un término estalla si su cuenta de la semana
supera N× la media de sus semanas previas. Se exige persistencia (elevado
también la semana anterior) para no morder picos de deadline de conferencia.

# ponytail: razón simple (cuenta/media), no Kleinberg. Si da falsos positivos,
# el upgrade es pybursts sobre las mismas cubetas — los datos ya quedan.

arXiv API: gratis, sin key, límite duro 1 request / 3 s.
Ref: https://info.arxiv.org/help/api/user-manual.html · Kleinberg 2002.
"""

from __future__ import annotations

import re
import time
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_CATS = ("cs.LG", "cs.AI", "cs.CL", "cs.NE", "stat.ML")
_STORE = "arxiv_terms.json"
_KEEP_WEEKS = 16
_STOP = {
    "a", "an", "the", "of", "for", "with", "and", "or", "to", "in", "on", "via",
    "is", "are", "we", "our", "this", "that", "from", "by", "as", "at", "it",
    "using", "towards", "toward", "can", "be", "new", "novel", "based", "study",
    "approach", "method", "methods", "model", "models", "learning", "neural",
    "networks", "network", "deep", "data", "large", "language", "results",
}
_WORD = re.compile(r"[a-z][a-z0-9-]{2,}")


def _week(ts: float) -> str:
    return time.strftime("%Y-W%V", time.gmtime(ts))


def _bigrams(title: str) -> list[str]:
    words = [w for w in _WORD.findall(title.lower()) if w not in _STOP]
    return [f"{a} {b}" for a, b in zip(words, words[1:], strict=False)]


def harvest(*, logs_dir: str, fetch_fn=None, now: float | None = None) -> dict:
    """Baja títulos recientes de arXiv y suma sus bigramas a la cubeta de esta
    semana. Idempotente por semana en el sentido útil: correrlo dos veces
    infla las cuentas, pero el burst es un RATIO contra semanas propias, así
    que la inflación se cancela mientras el ritmo de corridas sea parejo."""
    path = Path(logs_dir) / _STORE
    store = load_json_tolerant(path, {"weeks": {}})
    wk = _week(now if now is not None else time.time())
    bucket = store["weeks"].setdefault(wk, {})

    if fetch_fn is None:
        def fetch_fn(cat):
            url = ("http://export.arxiv.org/api/query?search_query=cat:"
                   f"{cat}&sortBy=submittedDate&sortOrder=descending"
                   "&max_results=150")
            req = urllib.request.Request(url, headers={"User-Agent": "mmorch-miner"})
            return urllib.request.urlopen(req, timeout=30).read().decode()

    titles = 0
    for i, cat in enumerate(_CATS):
        if i:
            time.sleep(3)   # límite duro de arXiv: 1 req / 3 s
        try:
            xml = fetch_fn(cat)
            root = ET.fromstring(xml)
        except Exception:
            continue
        for entry in root.iter("{http://www.w3.org/2005/Atom}entry"):
            node = entry.find("{http://www.w3.org/2005/Atom}title")
            if node is None or not node.text:
                continue
            titles += 1
            for term in set(_bigrams(node.text)):
                bucket[term] = bucket.get(term, 0) + 1

    for old in sorted(store["weeks"])[:-_KEEP_WEEKS]:
        del store["weeks"][old]
    atomic_write_json(path, store)
    return {"semana": wk, "titulos": titles, "terminos": len(bucket)}


def bursting(*, logs_dir: str, k: int = 2, ratio: float = 3.0,
             min_count: int = 4, now: float | None = None) -> list[str]:
    """Términos cuya cuenta de esta semana supera `ratio`× su propia media
    previa, y que ya venían elevados la semana pasada (persistencia = no es
    un pico de deadline). Devuelve los k más explosivos."""
    store = load_json_tolerant(Path(logs_dir) / _STORE, {})
    weeks = store.get("weeks", {})
    if len(weeks) < 4:
        return []   # sin línea de base no hay burst, hay ruido
    orden = sorted(weeks)
    actual, previa, base = weeks[orden[-1]], weeks[orden[-2]], orden[:-2]

    scores: dict[str, float] = {}
    for term, n in actual.items():
        if n < min_count:
            continue
        hist = [weeks[w].get(term, 0) for w in base]
        media = sum(hist) / len(hist)
        if n < ratio * (media + 1):
            continue
        if previa.get(term, 0) < media + 1:   # persistencia ≥2 ventanas
            continue
        scores[term] = n / (media + 1)
    return [t for t, _ in sorted(scores.items(), key=lambda kv: -kv[1])[:k]]


def _demo() -> None:
    """Self-check: término que explota y persiste sale; pico de una semana no."""
    import json
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        weeks = {f"2026-W{n:02d}": {"latent reasoning": 1, "vision transformer": 8}
                 for n in range(20, 24)}
        weeks["2026-W24"] = {"latent reasoning": 6, "vision transformer": 8,
                             "flash crash": 9}
        weeks["2026-W25"] = {"latent reasoning": 12, "vision transformer": 9,
                             "flash crash": 1}
        Path(d, _STORE).write_text(json.dumps({"weeks": weeks}), encoding="utf-8")
        out = bursting(logs_dir=d, k=5)
        assert "latent reasoning" in out, out          # explota y persiste
        assert "vision transformer" not in out, out    # alto pero plano
        assert "flash crash" not in out, out           # pico sin persistencia
        assert _bigrams("Deep Learning for Latent Reasoning") == ["latent reasoning"]
        print("bursts ok:", out)


if __name__ == "__main__":
    _demo()
