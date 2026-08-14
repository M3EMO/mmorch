"""F5 loop-cerrado: orquestador nightly del loop de ideas (spec .scratch/loop-cerrado/spec.md).

Encadena los modulos F1-F4 en el orden del spec, fail-soft (un paso que explota
queda en errors y el nightly sigue). Guardrails: kill-switch logs/loop_paused,
budget por contador mensual de llamadas LLM, jueces cross-family (DeepSeek
propone / Gemini refuta) via el unico seam _llm_json.
"""

from __future__ import annotations

import time
from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

CAP_CALLS_PER_MONTH = 2000
_EST_CALLS_PER_RUN = 40
FUEL_PATHS = ("logs/workflow_obs.jsonl", "logs/feedback.jsonl", "vault/research")
CANDIDATOS = "vault/roadmaps/candidatos.md"
ROADMAP = "vault/roadmaps/roadmap.md"

_JUDGE_SCHEMA = {
    "type": "object",
    "properties": {
        "score": {"type": "number"},
        "justification": {"type": "string"},
        # sin "type": el validate() de mmorch no soporta union ["string","null"]
        "cited_file": {},
        "gist": {},
    },
    "required": ["justification"],
}
_REFUTE_SCHEMA = {
    "type": "object",
    "properties": {"refuted": {"type": "boolean"}, "reason": {"type": "string"}},
    "required": ["refuted"],
}


def _llm_json(prompt: str, *, schema: dict, model: str | None = None) -> dict:
    """Unico seam de LLM del modulo (los tests lo monkeypatchean)."""
    from mmorch.config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
    from mmorch.schema import gated_json

    mdl = model or DEFAULT_GENERATOR
    # el refutador va cross-family por contrato: schema de refutacion -> verifier
    if schema is _REFUTE_SCHEMA and model is None:
        mdl = DEFAULT_VERIFIER
    return gated_json(mdl, [{"role": "user", "content": prompt}],
                      schema=schema, pattern="loop_propuestas", phase="idea_loop")


class _Judge:
    """generator.propose para adjudicacion (payload con note/project) y candidatas (lente)."""

    def propose(self, payload: dict) -> dict:
        if "madurar" in payload:
            prompt = (
                "Sos el madurador de candidatas de mmorch. Candidata: {gist}\n"
                "Otras candidatas vigentes (buscá cruces/sinergias): {otras}\n"
                "Expansiones YA usadas hoy en otras candidatas (PROHIBIDO repetir "
                "el mismo cruce o plantilla): {usadas}\n"
                "Proponé UNA expansión corta, concreta y ESPECÍFICA de esta "
                "candidata (máx 30 palabras), o null si no agrega valor real. "
                "JSON: {{\"gist\": str|null, \"justification\": str}}"
            ).format(gist=payload["madurar"][:1500],
                     otras=str(payload.get("otras"))[:3000],
                     usadas=str(payload.get("usadas_hoy"))[:1500])
            out = _llm_json(prompt, schema=_JUDGE_SCHEMA)
            return {"gist": out.get("gist"),
                    "justification": out.get("justification", "")}
        if "lente" in payload:
            prompt = (
                "Sos el ideador nocturno de mmorch. Lente: {lente}. Contexto de lo que "
                "cambio: {context}\nYa visto (NO repetir nada de esto): {visto}\n"
                "Proponé UNA direccion de mejora concreta para este lente, o null si no "
                "hay nada genuinamente nuevo. JSON: {{\"gist\": str|null, "
                "\"justification\": str}}"
            ).format(lente=payload["lente"], context=str(payload.get("context"))[:2000],
                     visto=str(payload.get("ya_visto"))[:4000])
            out = _llm_json(prompt, schema=_JUDGE_SCHEMA)
            return {"gist": out.get("gist"), "justification": out.get("justification", "")}
        readme = ""
        try:
            for name in ("README.md", "CLAUDE.md"):
                p = Path(str(payload.get("project_path") or "")) / name
                if p.exists():
                    readme = p.read_text(encoding="utf-8", errors="ignore")[:1500]
                    break
        except OSError:
            pass
        prompt = (
            "Sos el juez de adjudicacion de mmorch. ¿Esta nota de research aplica a este "
            "proyecto? Nota: {note}\nProyecto: {project} ({path})\n"
            "Descripcion del proyecto: {readme}\n"
            "Archivos del proyecto (si hay): {cg}\n"
            "JSON: {{\"score\": 0..1, \"justification\": str breve en espanol, "
            "\"cited_file\": str|null (archivo concreto donde aplica, solo si hay lista)}}"
        ).format(note=str(payload.get("note"))[:3000], project=payload.get("project"),
                 path=payload.get("project_path"), readme=readme or "(sin README)",
                 cg=str(payload.get("codegraph"))[:1500])
        out = _llm_json(prompt, schema=_JUDGE_SCHEMA)
        return {"score": float(out.get("score") or 0.0),
                "justification": out.get("justification", ""),
                "cited_file": out.get("cited_file")}


class _Refuter:
    """verifier.refute — cross-family, refuta por default (duda = refuted)."""

    def refute(self, payload: dict) -> dict:
        if "lente" in payload:
            # modo IDEA: screening, no verificacion de hechos — que la idea sea
            # imperfecta o tenga limites NO es razon (medido 2026-08-14: el modo
            # estricto mataba toda maduracion con objeciones perfeccionistas)
            prompt = (
                "Sos el screener de ideas de mmorch. Refutá SOLO si la idea es "
                "redundante con lo existente, incoherente, o dañina. Que sea "
                "imperfecta, parcial o tenga casos límite NO es razón para "
                "refutar.\nIdea: {item}\nJSON: {{\"refuted\": bool, \"reason\": str}}"
            ).format(item=str(payload)[:3000])
            out = _llm_json(prompt, schema=_REFUTE_SCHEMA)
            return {"refuted": bool(out.get("refuted", True)),
                    "reason": out.get("reason", "")}
        prompt = (
            "Sos el refutador de mmorch. REFUTA por default: solo si el match/idea es "
            "solido e inatacable respondé refuted=false. Ante la duda, refuted=true.\n"
            "Item: {item}\nJSON: {{\"refuted\": bool, \"reason\": str}}"
        ).format(item=str(payload)[:3000])
        out = _llm_json(prompt, schema=_REFUTE_SCHEMA)
        return {"refuted": bool(out.get("refuted", True)),
                "reason": out.get("reason", "")}


def build_judges() -> tuple:
    return _Judge(), _Refuter()


def _check_and_count_budget(n_calls: int, *, logs_dir: str, month: str) -> bool:
    path = Path(logs_dir) / "loop_budget.json"
    state = load_json_tolerant(path, {})
    if state.get("month") != month:
        state = {"month": month, "calls": 0}
    if state["calls"] + n_calls > CAP_CALLS_PER_MONTH:
        return False
    state["calls"] += n_calls
    atomic_write_json(path, state)
    return True


def _fuel_context(repo: Path, since_ts: float) -> str:
    changed = []
    for rel in FUEL_PATHS:
        p = repo / rel
        try:
            if p.is_dir():
                for f in sorted(p.glob("*.md")):
                    if f.stat().st_mtime > since_ts:
                        changed.append(f.name)
            elif p.stat().st_mtime > since_ts:
                changed.append(rel)
        except OSError:
            continue
    return "archivos con actividad nueva: " + (", ".join(changed) or "ninguno")


def run_idea_loop(*, repo_dir: str, today: str, generator=None, verifier=None,
                  record_fn=None, now_ts: float | None = None) -> dict:
    repo = Path(repo_dir)
    logs_dir = str(repo / "logs")

    if (repo / "logs" / "loop_paused").exists():
        return {"skipped": "paused"}
    if not _check_and_count_budget(_EST_CALLS_PER_RUN, logs_dir=logs_dir,
                                   month=today[:7]):
        return {"skipped": "budget"}

    if generator is None or verifier is None:
        generator, verifier = build_judges()

    steps: dict = {}
    errors: list[str] = []

    def _step(name, fn):
        try:
            steps[name] = fn()
        except Exception as e:  # fail-soft: el nightly jamas muere por este loop
            steps[name] = None
            errors.append(f"{name}: {type(e).__name__}: {str(e)[:200]}")

    candidatos_path = str(repo / CANDIDATOS)
    roadmap_path = str(repo / ROADMAP)

    from mmorch import adjudicate, fuel, outcomes, proposals
    from mmorch.projects import _load as _load_projects

    _step("expire_ignored",
          lambda: outcomes.expire_ignored(logs_dir=logs_dir, record_fn=record_fn))
    _step("expire_candidates",
          lambda: fuel.expire_candidates(candidatos_path=candidatos_path,
                                         today=today, record_fn=record_fn))
    _step("detect_promotions",
          lambda: fuel.detect_promotions(candidatos_path=candidatos_path,
                                         roadmap_path=roadmap_path,
                                         record_fn=record_fn))
    _step("adjudicate",
          lambda: adjudicate.run_incremental(str(repo / "vault" / "research"),
                                             _load_projects(), generator, verifier,
                                             logs_dir=logs_dir))

    state_path = repo / "logs" / "loop_state.json"
    state = load_json_tolerant(state_path, {})
    since_ts = float(state.get("last_run_ts", 0.0))

    def _candidatas():
        abs_fuel = [str(repo / rel) for rel in FUEL_PATHS]
        if not fuel.has_new_fuel(since_ts, abs_fuel):
            return {"nuevas": 0, "sin_fuel": True}
        return fuel.generate_candidates(_fuel_context(repo, since_ts), generator,
                                        verifier, candidatos_path=candidatos_path,
                                        roadmap_path=roadmap_path, today=today)

    _step("candidatas", _candidatas)
    _step("madurar",
          lambda: fuel.mature_candidates(generator, verifier,
                                         candidatos_path=candidatos_path,
                                         today=today))
    atomic_write_json(state_path,
                      {"last_run_ts": now_ts if now_ts is not None else time.time()})

    _step("compose_cards", lambda: proposals.compose_cards(logs_dir=logs_dir))

    adj = load_json_tolerant(Path(logs_dir) / "adjudications.json", {})
    counts: dict = {}
    for matches in (adj.get("by_project") or {}).values():
        for m in matches:
            st = m.get("status", "?")
            counts[st] = counts.get(st, 0) + 1
    try:
        vigentes = len(fuel.parse_candidatos(
            Path(candidatos_path).read_text(encoding="utf-8")))
    except OSError:
        vigentes = 0
    metrics = {"por_status": counts, "candidatas_vigentes": vigentes}

    return {"steps": steps, "metrics": metrics, "errors": errors}
