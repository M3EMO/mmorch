"""session_skills — mina playbooks reusables de sesiones de Claude. De segmentos con
outcome conocido (label EXTERNO, via sessions.outcome_of), extrae
(tarea, secuencia de tool-calls, dominio observado, reward) y agrega entre sesiones:
que secuencias de tools RECURREN y con que tasa de exito por dominio.

100% LOCAL: no manda nada a ninguna API (a diferencia de la calibracion del router).
Distinto de trajectory.distill_skill, que es para tareas de CODIGO red->green con
oraculo de ejecucion; aca el insumo es la secuencia heterogenea de tool-calls de una
sesion. Library-only.
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from pathlib import Path

from .sessions import (_ledger_seen, _resolve_latest, _session_id,
                       observed_domain, outcome_of, parse_session)

_STORE = Path(__file__).resolve().parent.parent / "logs" / "workflow_obs.jsonl"
_LEDGER = Path(__file__).resolve().parent.parent / "logs" / "workflow_sessions.txt"


@dataclass
class WorkflowObs:
    task: str            # request recortado
    tools: tuple         # nombres de tool-calls EN ORDEN
    domain: str          # dominio Cynefin observado
    reward: float        # outcome externo (0..1)


@dataclass
class Playbook:
    domain: str
    tool_sequence: tuple
    n_observed: int
    n_success: int
    success_rate: float


def extract_workflows(path) -> list[WorkflowObs]:
    """Segmentos CON outcome y CON tool-calls -> observaciones de workflow. 100% local."""
    p = _resolve_latest() if path == "latest" else Path(path)
    return _extract(parse_session(p), start=0)


def _extract(segs, start: int = 0) -> list[WorkflowObs]:
    """Extrae observaciones de segs[start:]. next_request usa la lista COMPLETA para que
    el limite con el segmento siguiente sea correcto. start>0 = ingesta incremental."""
    out: list[WorkflowObs] = []
    for i in range(start, len(segs)):
        seg = segs[i]
        next_req = segs[i + 1].request if i + 1 < len(segs) else ""
        o = outcome_of(seg, next_request=next_req)
        if o is None:
            continue                       # sin label externo -> no se aprende
        tools = tuple(tc.get("name", "") for tc in seg.tool_calls if tc.get("name"))
        if not tools:
            continue                       # sin tool-calls no hay workflow
        out.append(WorkflowObs(task=seg.request[:200], tools=tools,
                               domain=observed_domain(seg), reward=o.reward))
    return out


def mine_playbooks(obs, *, min_observed: int = 2) -> list[Playbook]:
    """Agrupa por (dominio, secuencia de tools) -> playbook con tasa de exito REAL.
    Solo secuencias RECURRENTES (n_observed >= min_observed): un one-off no es playbook."""
    agg: dict[tuple, list[int]] = {}       # (domain, tools) -> [n, n_success]
    for w in obs:
        a = agg.setdefault((w.domain, tuple(w.tools)), [0, 0])
        a[0] += 1
        if w.reward >= 0.5:
            a[1] += 1
    books = [Playbook(domain=d, tool_sequence=t, n_observed=n, n_success=s,
                      success_rate=round(s / n, 3))
             for (d, t), (n, s) in agg.items() if n >= min_observed]
    books.sort(key=lambda b: (b.success_rate, b.n_observed), reverse=True)
    return books


def ingest_workflows(path, *, store: Path = _STORE, ledger: Path = _LEDGER) -> int:
    """Extrae + persiste observaciones (append-only). Idempotencia INCREMENTAL por
    sessionId: el ledger guarda cuantos segmentos se vieron, y solo se procesan los
    NUEVOS (segs[start:]). Asi una sesion que crece o se reanuda no duplica lo previo ni
    pierde lo nuevo. Devuelve cuantas observaciones nuevas se guardaron. 100% local.

    Recomendado: ingerir sesiones SETTLED (terminadas) — ahi el outcome de cada segmento
    es final. ponytail: en una ingesta MID-sesion, el label del segmento limite se computa
    con el next_request disponible en ese momento y no se revisa luego; afecta solo labels
    user-based sobre ese unico segmento, edge acotado que el flujo settled evita."""
    p = _resolve_latest() if path == "latest" else Path(path)
    sid = _session_id(p)
    segs = parse_session(p)
    start = _ledger_seen(ledger).get(sid, 0)
    if start >= len(segs):
        return 0                           # nada nuevo desde la ultima ingesta
    obs = _extract(segs, start=start)
    store.parent.mkdir(parents=True, exist_ok=True)
    with open(store, "a", encoding="utf-8") as f:
        for w in obs:
            f.write(json.dumps({**asdict(w), "tools": list(w.tools)}, ensure_ascii=False) + "\n")
    ledger.parent.mkdir(parents=True, exist_ok=True)
    with open(ledger, "a", encoding="utf-8") as f:
        f.write(f"{sid}\t{len(segs)}\n")    # marca cuantos segmentos vimos
    return len(obs)


def load_observations(store: Path = _STORE) -> list[WorkflowObs]:
    if not store.exists():
        return []
    out: list[WorkflowObs] = []
    for ln in store.read_text(encoding="utf-8").splitlines():
        if not ln.strip():
            continue
        d = json.loads(ln)
        out.append(WorkflowObs(task=d["task"], tools=tuple(d["tools"]),
                               domain=d["domain"], reward=d["reward"]))
    return out


def top_playbooks(*, store: Path = _STORE, domain: str | None = None,
                  min_observed: int = 2, limit: int = 10) -> list[Playbook]:
    """Lee observaciones persistidas y devuelve los mejores playbooks (opcional por dominio)."""
    obs = load_observations(store)
    if domain:
        obs = [w for w in obs if w.domain == domain]
    return mine_playbooks(obs, min_observed=min_observed)[:limit]
