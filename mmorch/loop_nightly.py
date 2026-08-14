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
CAP_USD_PER_MONTH = 3.0
_EST_CALLS_PER_RUN = 40
_BUDGET_PATH: Path | None = None  # seteado por run_idea_loop; _llm_json acumula USD reales
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


def _llm_json(prompt: str, *, schema: dict, model: str | None = None,
              temperature: float = 0.0) -> dict:
    """Unico seam de LLM del modulo (los tests lo monkeypatchean)."""
    from mmorch.config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
    from mmorch.schema import gated_json

    mdl = model or DEFAULT_GENERATOR
    # el refutador va cross-family por contrato: schema de refutacion -> verifier
    if schema is _REFUTE_SCHEMA and model is None:
        mdl = DEFAULT_VERIFIER
    out = gated_json(mdl, [{"role": "user", "content": prompt}],
                     schema=schema, pattern="loop_propuestas", phase="idea_loop",
                     temperature=temperature)
    # budget en USD REALES (gated_json reporta _cost_usd), no solo conteo de calls
    if _BUDGET_PATH is not None and isinstance(out, dict):
        try:
            state = load_json_tolerant(_BUDGET_PATH, {})
            state["usd"] = round(state.get("usd", 0.0)
                                 + float(out.get("_cost_usd") or 0.0), 6)
            atomic_write_json(_BUDGET_PATH, state)
        except Exception:
            pass  # side-channel de contabilidad: jamas rompe la llamada
    return out


_PROMPTS_DIR = Path(__file__).resolve().parents[1] / "prompts"
# temperatura de ideacion: 0.0 colapsa en el atractor mas saliente (medido
# 2026-08-14: la misma expansion pegada a las 7 candidatas). Adjudicacion y
# refutacion quedan en 0.0 — ahi el determinismo es virtud.
_IDEATE_TEMP = 0.8
_IDEATE_SAMPLES = 2


def _prompt_file(name: str, default: str) -> str:
    """Prompts de ideacion externalizados -> hillclimbeables por autoresearch."""
    try:
        return (_PROMPTS_DIR / name).read_text(encoding="utf-8")
    except OSError:
        return default


def _tokens(text: str) -> set:
    return {t for t in text.lower().split() if len(t) > 3}


def _novelty(text: str, seen: list[str]) -> float:
    """1 - max Jaccard contra lo ya visto (mas alto = mas nuevo)."""
    tt = _tokens(text)
    if not tt or not seen:
        return 1.0
    worst = 0.0
    for s in seen:
        st = _tokens(s)
        if st:
            worst = max(worst, len(tt & st) / len(tt | st))
    return 1.0 - worst


class _Judge:
    """generator.propose para adjudicacion (payload con note/project) y candidatas (lente)."""

    def propose(self, payload: dict) -> dict:
        if "madurar" in payload:
            tpl = _prompt_file("idea_madurar.txt",
                               "Candidata: {gist}\nOtras: {otras}\nUsadas: {usadas}\n"
                               "JSON: {{\"gist\": str|null, \"justification\": str}}")
            prompt = tpl.format(gist=payload["madurar"][:1500],
                                otras=str(payload.get("otras"))[:3000],
                                usadas=str(payload.get("usadas_hoy"))[:1500])
            # sampling con temperatura + seleccion por NOVEDAD (anti-colapso
            # estructural, no solo la prohibicion en el prompt)
            seen = [payload["madurar"]] + list(payload.get("usadas_hoy") or [])
            best, best_nov = None, -1.0
            for _ in range(_IDEATE_SAMPLES):
                out = _llm_json(prompt, schema=_JUDGE_SCHEMA,
                                temperature=_IDEATE_TEMP)
                gist = (out.get("gist") or "").strip()
                if not gist:
                    continue
                nov = _novelty(gist, seen)
                if nov > best_nov:
                    best, best_nov = out, nov
            if best is None:
                return {"gist": None, "justification": ""}
            return {"gist": best.get("gist"),
                    "justification": best.get("justification", "")}
        if "lente" in payload:
            prompt = (
                "Sos el ideador nocturno de mmorch. Lente: {lente}. Contexto de lo que "
                "cambio: {context}\nYa visto (NO repetir nada de esto): {visto}\n"
                "Proponé UNA direccion de mejora concreta para este lente, o null si no "
                "hay nada genuinamente nuevo. JSON: {{\"gist\": str|null, "
                "\"justification\": str}}"
            ).format(lente=payload["lente"], context=str(payload.get("context"))[:2000],
                     visto=str(payload.get("ya_visto"))[:4000])
            out = _llm_json(prompt, schema=_JUDGE_SCHEMA, temperature=_IDEATE_TEMP)
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
        # guard determinista: sin codegraph NO hay lista de archivos -> el juez
        # no puede citar (medido 2026-08-14: invento RefinementManager.java en
        # un proyecto sin indice)
        cited = out.get("cited_file") if payload.get("codegraph") else None
        return {"score": float(out.get("score") or 0.0),
                "justification": out.get("justification", ""),
                "cited_file": cited}


class _Refuter:
    """verifier.refute — cross-family, refuta por default (duda = refuted)."""

    def refute(self, payload: dict) -> dict:
        if "lente" in payload:
            # modo IDEA: screening, no verificacion de hechos — que la idea sea
            # imperfecta o tenga limites NO es razon (medido 2026-08-14: el modo
            # estricto mataba toda maduracion con objeciones perfeccionistas)
            tpl = _prompt_file("idea_screener.txt",
                               "Refutá solo redundante/incoherente/dañina.\n"
                               "Idea: {item}\nJSON: {{\"refuted\": bool, \"reason\": str}}")
            prompt = tpl.format(item=str(payload)[:3000])
            out = _llm_json(prompt, schema=_REFUTE_SCHEMA)
            return {"refuted": bool(out.get("refuted", True)),
                    "reason": out.get("reason", "")}
        if "note" in payload:
            # modo ADJUDICACION: un match nota->proyecto no es un teorema — refutar
            # solo si NO hay relacion real o la justificacion es inventada (medido
            # 2026-08-14: el modo estricto mato los 16 matches >=0.85 del juez,
            # todos sensatos; 2 corridas de 144 pares -> 0 strong)
            prompt = (
                "Sos el auditor de adjudicaciones de mmorch. El juez propuso que esta "
                "nota aplica a este proyecto. Refutá si: (1) la nota NO tiene relación "
                "real con el proyecto; (2) la justificación inventa hechos o cita "
                "archivos que no están en la lista; (3) la justificación habla de OTRO "
                "proyecto distinto al adjudicado (ej: dice 'aplica a mmorch' cuando el "
                "proyecto es Minecraft). Que el beneficio sea parcial o requiera "
                "trabajo NO es razón para refutar.\n"
                "Item: {item}\nJSON: {{\"refuted\": bool, \"reason\": str}}"
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
        state = {"month": month, "calls": 0, "usd": 0.0}
    if state["calls"] + n_calls > CAP_CALLS_PER_MONTH:
        return False
    if state.get("usd", 0.0) >= CAP_USD_PER_MONTH:   # tope en plata REAL
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

    global _BUDGET_PATH
    _BUDGET_PATH = repo / "logs" / "loop_budget.json"

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
