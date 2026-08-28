"""arbitration — registro auditable de los arbitrajes del orquestador (blind-spot #2, 2026-07).

El sistema mide la tasa de falsas críticas de los REVIEWERS (~74%) pero nunca midió la tasa de
falsos DESCARTES del ÁRBITRO (el modelo caro que triagea refutaciones). Este módulo cierra eso:
cada arbitraje (crítica → veredicto valid/partial/dismissed + razón + evidencia) se appendea a un
JSONL; `pending_recheck()` devuelve los descartados viejos para re-testearlos contra evidencia
posterior — si una crítica descartada se manifestó después como bug, ESO es un falso descarte
medido, y la tasa sale de acá. Cero API; puro registro.
"""
from __future__ import annotations

import json
import pathlib
import time

from .paths import logs_dir

_LOG = logs_dir() / "arbitrations.jsonl"

VERDICTS = ("valid", "partial", "dismissed")


def log(critique: str, verdict: str, reason: str, *, source: str = "",
        evidence: str = "", path: pathlib.Path | None = None) -> dict:
    """Registra un arbitraje. `evidence` = cómo se decidió (probe corrido, lectura, medición) —
    un descarte SIN evidencia es en sí una señal de riesgo. Devuelve el registro escrito."""
    if verdict not in VERDICTS:
        raise ValueError(f"verdict must be one of {VERDICTS}")
    rec = {"ts": time.time(), "critique": critique[:400], "verdict": verdict,
           "reason": reason[:400], "source": source[:80], "evidence": evidence[:200],
           "rechecked": False}
    p = path or _LOG
    p.parent.mkdir(parents=True, exist_ok=True)
    with open(p, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    return rec


def _read(path: pathlib.Path | None = None) -> list[dict]:
    p = path or _LOG
    if not p.exists():
        return []
    out = []
    for ln in p.read_text(encoding="utf-8").splitlines():
        try:
            out.append(json.loads(ln))
        except Exception:
            continue   # una línea corrupta no rompe la auditoría
    return out


def pending_recheck(*, older_than_s: float = 7 * 86400,
                    path: pathlib.Path | None = None) -> list[dict]:
    """Descartes con edad suficiente para re-testear contra evidencia posterior (¿la crítica
    descartada se manifestó como bug después?). El re-check es juicio del orquestador; acá
    solo se surfacea la cola."""
    now = time.time()
    return [r for r in _read(path)
            if r["verdict"] == "dismissed" and not r.get("rechecked")
            and now - r["ts"] >= older_than_s]


def stats(path: pathlib.Path | None = None) -> dict:
    """Distribución de veredictos + % de descartes sin evidencia (proxy de riesgo del árbitro)."""
    rows = _read(path)
    by = {v: sum(1 for r in rows if r["verdict"] == v) for v in VERDICTS}
    dis = [r for r in rows if r["verdict"] == "dismissed"]
    no_ev = sum(1 for r in dis if not r.get("evidence"))
    return {"total": len(rows), **by,
            "dismissed_without_evidence": no_ev,
            "dismissed_without_evidence_rate": round(no_ev / len(dis), 3) if dis else None}


if __name__ == "__main__":
    import tempfile
    p = pathlib.Path(tempfile.mkdtemp()) / "arb.jsonl"
    log("shell=True es inyección", "dismissed", "cmds compuestos + worktree",
        source="review-r3", evidence="posture decidida + contención probada", path=p)
    log("commit_fn(None) explota", "dismissed", "F2 guarda `if commit_fn`",
        source="review-r4", evidence="", path=p)                       # descarte SIN evidencia
    log("integration gap real", "valid", "F2 no corría el acceptance",
        source="plan-r1", evidence="lectura de código L89", path=p)
    s = stats(p)
    assert s["total"] == 3 and s["dismissed"] == 2 and s["valid"] == 1, s
    assert s["dismissed_without_evidence"] == 1 and s["dismissed_without_evidence_rate"] == 0.5, s
    assert pending_recheck(older_than_s=0, path=p) and len(pending_recheck(older_than_s=0, path=p)) == 2
    assert pending_recheck(older_than_s=9999, path=p) == []            # muy recientes: aún no
    try:
        log("x", "maybe", "r", path=p)
        raise AssertionError("verdict inválido debe fallar")
    except ValueError:
        pass
    print("arbitration OK — log, stats (descartes sin evidencia), cola de re-check")
