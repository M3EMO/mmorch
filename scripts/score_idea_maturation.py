"""Scorer CONGELADO del madurador de candidatas (eval para autoresearch/hillclimb).

Corre mature_candidates sobre un fixture FIJO (embebido aca — jamas cambiarlo:
cambiarlo invalida toda comparacion historica) con los jueces vivos, y puntua
propiedades ESTRUCTURALES deterministas de los outputs:

  - cobertura:     fraccion de candidatas maduradas (la trampa-sin-valor no cuenta)
  - unicidad:      1 - fraccion de expansiones casi-duplicadas entre si
  - especificidad: fraccion cuyos tokens pisan mas su propia candidata que las otras
  - trampa:        1.0 si la candidata sin-valor-posible quedo SIN madurar (null correcto)

score = promedio de las 4. Uso como target de autoresearch:
  MMORCH_AR_TARGET=mmorch/prompts/idea_madurar.txt
  MMORCH_AR_SCORER=scripts/score_idea_maturation.py
"""

import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from dotenv import load_dotenv  # noqa: E402

load_dotenv(ROOT / ".env")

from mmorch.fuel import mature_candidates, parse_candidatos, render_candidatos  # noqa: E402
from mmorch.loop_nightly import build_judges  # noqa: E402

# ── fixture congelado (4 reales de 2026-08-14 + 1 trampa sin valor posible) ──
_FROZEN = [
    {"id": "fx-01", "fecha": "2026-08-14", "vence": "2099-01-01", "lente": "deuda",
     "gist": "rollback estructural de refinements en evolve — before/after snapshot "
             "por edit + inversion mecanica sin LLM", "estado": "pendiente"},
    {"id": "fx-02", "fecha": "2026-08-14", "vence": "2099-01-01", "lente": "capacidad",
     "gist": "playbooks ejecutables — campo reference {module, callable, args_schema} "
             "validado en session_skills", "estado": "pendiente"},
    {"id": "fx-03", "fecha": "2026-08-14", "vence": "2099-01-01", "lente": "integracion",
     "gist": "review-gate barato pre-persistencia del ingest — 1 call shouldRefine "
             "antes de escribir memoria", "estado": "pendiente"},
    {"id": "fx-04", "fecha": "2026-08-14", "vence": "2099-01-01", "lente": "capacidad",
     "gist": "router aprendido — clasificador chico sobre feedback.jsonl que "
             "complemente el ThompsonBandit", "estado": "pendiente"},
    # trampa: ya cerrada/completa — la maduracion correcta es NO madurarla (null)
    {"id": "fx-trap", "fecha": "2026-08-14", "vence": "2099-01-01", "lente": "deuda",
     "gist": "actualizar la version de pytest en requirements (ya hecho, sin nada "
             "que agregar, cerrado y mergeado)", "estado": "pendiente"},
]

_TODAY = "2099-01-01"  # fijo: el marker no debe chocar con corridas reales


def _toks(t):
    return {w for w in t.lower().split() if len(w) > 3}


def main() -> None:
    with tempfile.TemporaryDirectory() as td:
        path = Path(td) / "candidatos.md"
        path.write_text(render_candidatos([dict(e) for e in _FROZEN], []),
                        encoding="utf-8")
        gen, ver = build_judges()
        mature_candidates(gen, ver, candidatos_path=str(path), today=_TODAY)
        entries = parse_candidatos(path.read_text(encoding="utf-8"))

    marker = f">> {_TODAY}:"
    ext = {}
    for e in entries:
        if marker in e["gist"]:
            base, _, extra = e["gist"].partition(marker)
            ext[e["id"]] = (base.strip(), extra.strip())

    reales = [e for e in _FROZEN if e["id"] != "fx-trap"]
    cobertura = sum(1 for e in reales if e["id"] in ext) / len(reales)
    trampa = 0.0 if "fx-trap" in ext else 1.0

    extras = [x[1] for i, x in ext.items() if i != "fx-trap"]
    dup = 0
    for i, a in enumerate(extras):
        for b in extras[i + 1:]:
            ta, tb = _toks(a), _toks(b)
            if ta and tb and len(ta & tb) / len(ta | tb) > 0.5:
                dup += 1
                break
    unicidad = 1.0 - (dup / len(extras)) if extras else 0.0

    especificas = 0
    for cid, (base, extra) in ext.items():
        if cid == "fx-trap":
            continue
        otras = " ".join(x["gist"] for x in _FROZEN if x["id"] not in (cid, "fx-trap"))
        te = _toks(extra)
        propia = len(te & _toks(base))
        ajena = len(te & _toks(otras))
        if propia >= ajena:
            especificas += 1
    especificidad = especificas / len(extras) if extras else 0.0

    score = (cobertura + unicidad + especificidad + trampa) / 4
    print(f"cobertura={cobertura:.2f} unicidad={unicidad:.2f} "
          f"especificidad={especificidad:.2f} trampa={trampa:.2f}")
    print(f"score: {score:.3f}")


if __name__ == "__main__":
    main()
