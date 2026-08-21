"""Provenance de branches — outcomes retroactivos por verdad de ejecución.

Respuesta a "¿cómo sé que una idea/cambio es bueno?": la única medida robusta
es SUPERVIVENCIA en la cadena — se construyó → pasó el gate → se mergeó → no
se revirtió. Cada eslabón ya existía; lo que se cortaba era la atribución:
la branch se mergeaba (o moría) y el brazo que la produjo nunca se enteraba.
Con 35/35 "dale" humanos sin discriminación, estas señales implícitas son
las que alimentan al bandit sin pedirle un solo clic más al humano:

  merge (humano via manana.py, o automerge verde) → reward 1.0  al brazo
  branch viva sin merge > _EXPIRE_DAYS            → reward 0.2  (rechazo blando)

El "no" explícito (0.125) queda para veredictos humanos reales — una branch
ignorada no es un rechazo fuerte, es tibieza, y la escala ya existente de
outcomes.py (1.0 / 0.2 / 0.125) se respeta tal cual.

Registro append-only en logs/branch_provenance.jsonl:
  {ts, branch, arm, origin, target, status: pendiente|merged|expirado}
El estado NO se muta in-place: merged/expirado se re-appendean (mismo patrón
ledger de automerge). El último registro de una branch manda.
"""

from __future__ import annotations

import json
import time
from pathlib import Path

_LEDGER = "branch_provenance.jsonl"
_EXPIRE_DAYS = 14


def _append(logs_dir: str, rec: dict) -> None:
    p = Path(logs_dir) / _LEDGER
    p.parent.mkdir(parents=True, exist_ok=True)
    with open(p, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(rec, ensure_ascii=False) + "\n")


def _latest(logs_dir: str) -> dict[str, dict]:
    """Último registro por branch (el ledger es append-only)."""
    p = Path(logs_dir) / _LEDGER
    out: dict[str, dict] = {}
    try:
        for ln in p.read_text(encoding="utf-8").splitlines():
            try:
                r = json.loads(ln)
                out[r["branch"]] = r
            except (json.JSONDecodeError, KeyError):
                continue
    except OSError:
        pass
    return out


def record_branch(branch: str, *, arm: str, origin: str, target: str = "",
                  logs_dir: str = "logs") -> None:
    """Al NACER una branch gateada (sandbox verde que queda esperando merge):
    quién la produjo (arm = modelo#origen) y de qué paso salió."""
    _append(logs_dir, {"ts": time.time(), "branch": branch, "arm": arm,
                       "origin": origin, "target": target,
                       "status": "pendiente"})


def on_merge(branch: str, *, logs_dir: str = "logs", record_fn=None) -> bool:
    """Al mergearse una branch (manana.py, automerge): reward 1.0 retroactivo
    al brazo que la produjo. Devuelve True si habia provenance registrada.
    Fail-soft: una branch sin provenance (previa a este modulo, o manual) no
    rompe nada — simplemente no enseña."""
    reg = _latest(logs_dir).get(branch)
    if reg is None or reg.get("status") != "pendiente":
        return False
    if record_fn is None:
        from mmorch.feedback import record_outcome

        def record_fn(arm, reward, **kw):
            record_outcome(arm, reward, **kw)
    record_fn(reg["arm"], 1.0, pattern=reg.get("origin", ""),
              source="merge", context=reg.get("target", ""))
    _append(logs_dir, {**reg, "ts": time.time(), "status": "merged"})
    return True


def sweep_expired(*, logs_dir: str = "logs", now: float | None = None,
                  record_fn=None) -> list[str]:
    """Nocturno: branches 'pendiente' con mas de _EXPIRE_DAYS sin merge →
    rechazo blando (0.2) UNA vez, y quedan marcadas 'expirado' (idempotente:
    la proxima corrida no las vuelve a castigar)."""
    now = now if now is not None else time.time()
    if record_fn is None:
        from mmorch.feedback import record_outcome

        def record_fn(arm, reward, **kw):
            record_outcome(arm, reward, **kw)
    swept = []
    for branch, reg in _latest(logs_dir).items():
        if reg.get("status") != "pendiente":
            continue
        if now - reg.get("ts", now) < _EXPIRE_DAYS * 86400:
            continue
        record_fn(reg["arm"], 0.2, pattern=reg.get("origin", ""),
                  source="branch_expirada", context=reg.get("target", ""))
        _append(logs_dir, {**reg, "ts": now, "status": "expirado"})
        swept.append(branch)
    return swept


def _demo() -> None:
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        rewards: list = []

        def rec(arm, reward, **kw):
            rewards.append((arm, reward, kw.get("source")))

        record_branch("mmorch-sbx-abc", arm="deepseek-chat#evolve",
                      origin="evolve", target="mmorch/x.py", logs_dir=d)
        assert on_merge("mmorch-sbx-abc", logs_dir=d, record_fn=rec)
        assert rewards == [("deepseek-chat#evolve", 1.0, "merge")]
        assert not on_merge("mmorch-sbx-abc", logs_dir=d, record_fn=rec)  # idempotente
        assert not on_merge("sin-provenance", logs_dir=d, record_fn=rec)  # fail-soft

        record_branch("mmorch-sbx-vieja", arm="glm-4.6#slim", origin="slim",
                      logs_dir=d)
        futuro = time.time() + 15 * 86400
        assert sweep_expired(logs_dir=d, now=futuro, record_fn=rec) == ["mmorch-sbx-vieja"]
        assert rewards[-1] == ("glm-4.6#slim", 0.2, "branch_expirada")
        assert sweep_expired(logs_dir=d, now=futuro, record_fn=rec) == []  # una sola vez
        print("provenance ok")


if __name__ == "__main__":
    _demo()
