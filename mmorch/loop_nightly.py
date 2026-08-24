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
        # flywheel de entrenamiento (goal 2026-08-15): persistir el I/O completo
        # de cada juicio — hoy se tiraba; es el dato crudo de un futuro
        # fine-tune (juez/refutador destilado, router). Local-only, fail-open.
        try:
            import json as _json
            import time as _time
            with open(_BUDGET_PATH.parent / "idea_loop_traces.jsonl", "a",
                      encoding="utf-8") as f:
                f.write(_json.dumps(
                    {"ts": _time.time(), "model": mdl, "temperature": temperature,
                     "prompt": prompt[:6000],
                     "output": {k: v for k, v in out.items()
                                if not k.startswith("_")}},
                    ensure_ascii=False) + "\n")
        except Exception:
            pass
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


_DESC_SCHEMA = {"type": "object", "properties": {"desc": {"type": "string"}},
                "required": ["desc"]}
_DIGEST_SCHEMA = {"type": "object", "properties": {"digest": {"type": "string"}},
                  "required": ["digest"]}
_REFLECT_SCHEMA = {
    "type": "object",
    "properties": {"diagnostico": {"type": "string"},
                   "foco_sugerido": {"type": "string"},
                   "riesgo_principal": {"type": "string"},
                   # keywords, no prosa: repo_mining manda esto a GitHub search.
                   # Antes usaba foco_sugerido[:80] y buscaba parrafos en
                   # castellano -> 0 resultados, quemando 1 de las 3 queries
                   # semanales de descubrimiento.
                   "temas_busqueda": {"type": "array",
                                      "items": {"type": "string"}}},
    "required": ["diagnostico", "foco_sugerido", "riesgo_principal"],
}


def _tag(v) -> tuple[str, str]:
    """[ok]/[skip]/[ERROR] + detalle, para un valor de subsistema del record."""
    import json
    if not isinstance(v, dict):
        return "?", json.dumps(v, ensure_ascii=False)
    if v.get("error"):
        return "ERROR", str(v["error"])
    if v.get("errors"):
        return f"ERRORES({len(v['errors'])})", "; ".join(
            str(e) for e in v["errors"][:2])
    if v.get("skipped"):
        return "skip", str(v["skipped"])
    return "ok", json.dumps(v, ensure_ascii=False)


def _facts(records: list[dict]) -> str:
    """Cifras CALCULADAS sobre la historia entera, para anclar a la reflexion.

    Sin esto el LLM no puede contar (solo ve n_nights records) y arrastraba el
    numero de su propia reflexion anterior, sumandole 5 cada noche: medido
    6 -> 8 -> 10 -> 12 -> 15 -> 20 -> 25 -> 30 -> '35+ noches' en 7 dias
    reales. Una cifra inventada que se auto-alimenta es peor que ninguna:
    el humano deja de creerle al diagnostico entero."""
    import time as _t
    from .stuck_detector import _consecutive_recent
    if not records:
        return "(sin historia)"
    dias = [_t.strftime("%Y-%m-%d", _t.localtime(r.get("ts", 0))) for r in records]
    out = [f"- corridas registradas: {len(records)}, del {dias[0]} al {dias[-1]} "
           f"({len(set(dias))} dias distintos)"]
    for k in [k for k in records[-1] if k != "ts"]:
        # subsistema AUSENTE corta la racha: no existia todavia, no fallo
        n = _consecutive_recent(
            records, lambda r, k=k: k in r and _tag(r[k])[0] != "ok")
        if n:
            out.append(f"- {k}: {n} corridas consecutivas sin [ok] "
                       f"(sobre {len(records)} registradas)")
    return "\n".join(out)


def _summarize_record(rec: dict, *, per_key: int = 140) -> str:
    """Una linea por subsistema (status + detalle corto), no el JSON crudo.

    Medido: el record completo ronda 6-7k chars; cortarlo a 1200 chars crudos
    SIEMPRE perdia los mismos campos (los que el script arma al final —
    project_health, auto_repair, slim, arxiv, repo_mining, smoke, merge_train)
    porque el orden de insercion en el dict es fijo. reflect() nunca los veia,
    noche tras noche. Con una linea pareja por subsistema, todos entran."""
    out = []
    for k, v in rec.items():
        if k == "ts":
            continue
        tag, detail = _tag(v)
        out.append(f"- {k} [{tag}]: {detail[:per_key]}")
    return "\n".join(out)


def reflect(*, logs_dir: str, today: str, n_nights: int = 7) -> dict:
    """Reflexion nocturna: mmorch lee sus PROPIAS ultimas corridas y se
    auto-evalua — diagnostico de tendencia, foco sugerido para las proximas
    noches y riesgo principal. Es la capa 'pensar sobre si mismo' del goal
    Jarvis: no reacciona a un paso, mira su propia trayectoria.

    Persistencia: logs/reflexiones.jsonl (append) — historia de que penso el
    sistema de si mismo cada noche; el digest y el humano la leen; el propio
    reflect ve su reflexion anterior (continuidad de pensamiento)."""
    import json as _json
    path = Path(logs_dir) / "nightly.jsonl"
    try:
        lines = path.read_text(encoding="utf-8").splitlines()[-n_nights:]
    except OSError:
        return {"skipped": "sin historia"}
    prev_path = Path(logs_dir) / "reflexiones.jsonl"
    prev = ""
    try:
        prev = prev_path.read_text(encoding="utf-8").splitlines()[-1]
    except (OSError, IndexError):
        pass
    # resumen parejo por subsistema en vez de JSON crudo cortado a 1200 chars:
    # el corte crudo perdia SIEMPRE los mismos campos (los ultimos del dict)
    resumenes = []
    for raw in lines:
        try:
            resumenes.append(_summarize_record(_json.loads(raw)))
        except (_json.JSONDecodeError, TypeError, AttributeError):
            resumenes.append(raw[:400])
    todos = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        try:
            todos.append(_json.loads(raw))
        except _json.JSONDecodeError:
            pass
    out = _llm_json(
        "Sos la capa de auto-reflexion de mmorch (sistema de orquestacion que "
        "se auto-mejora). Estas son tus ultimas corridas nocturnas (resumen "
        "por subsistema: [ok]/[skip]/[ERROR]/[ERRORES]) y tu reflexion "
        "anterior. Mira tu TRAYECTORIA, no una noche: ¿que tendencia hay "
        "(errores repetidos, pasos que nunca rinden, aprendizaje estancado)? "
        "¿donde deberia ir el foco de las proximas noches? ¿cual es el riesgo "
        "principal de seguir igual? Se especifico y critico — esto lo lee el "
        "humano y tu proxima reflexion.\n"
        "REGLA DURA sobre cifras: toda cantidad de noches/corridas que escribas "
        "tiene que salir TAL CUAL de HECHOS. No estimes, no redondees hacia "
        "arriba, no reuses las cifras de tu reflexion anterior (son texto, no "
        "evidencia) y no escribas 'N+'. Si una cifra no esta en HECHOS, decilo "
        "sin numero. Una cifra inflada invalida el diagnostico entero.\n"
        f"HECHOS (calculados sobre la historia completa, no estimados):\n"
        f"{_facts(todos)}\n"
        f"Reflexion anterior (prosa, NO fuente de cifras): "
        f"{prev[:1500] or '(primera reflexion)'}\n"
        f"Corridas (solo las ultimas {n_nights}):\n"
        f"{(chr(10) + '== noche ==' + chr(10)).join(resumenes)}\n"
        "temas_busqueda: 2 keywords en INGLES (2-4 palabras cada una, <=60 "
        "chars) para buscar repos en GitHub sobre lo que te falta — tecnicas, "
        "no frases. Ej: 'mutation testing python', 'llm agent memory'.\n"
        'JSON: {"diagnostico": str, "foco_sugerido": str, '
        '"riesgo_principal": str, "temas_busqueda": [str]}',
        schema=_REFLECT_SCHEMA)
    rec = {"fecha": today, "diagnostico": out.get("diagnostico", ""),
           "foco_sugerido": out.get("foco_sugerido", ""),
           "riesgo_principal": out.get("riesgo_principal", ""),
           "temas_busqueda": out.get("temas_busqueda") or []}
    with open(prev_path, "a", encoding="utf-8") as f:
        f.write(_json.dumps(rec, ensure_ascii=False) + "\n")
    apply_focus(rec, logs_dir=logs_dir)
    return rec


def apply_focus(reflexion: dict, *, logs_dir: str) -> dict:
    """Pienso->actuo con volante LIMITADO: si el foco de la reflexion nombra un
    modulo mmorch/*.py, el hardening lo prioriza esa noche. Determinista (regex
    sobre texto que la propia reflexion ya escribio), whitelist implicita (solo
    modulos existentes del repo), y solo REORDENA prioridades — jamas toca
    codigo, config ni presupuestos."""
    import re as _re
    text = f"{reflexion.get('foco_sugerido', '')} {reflexion.get('diagnostico', '')}"
    focus = {}
    for name in _re.findall(r"\b([a-z_]+)\.py\b", text.lower()):
        cand = Path(logs_dir).parent / "mmorch" / f"{name}.py"
        if cand.exists():
            focus = {"hardening_module": f"mmorch/{name}.py",
                     "fecha": reflexion.get("fecha", "")}
            break
    atomic_write_json(Path(logs_dir) / "focus.json", focus)
    return focus


def write_local_digest(rec: dict, *, logs_dir: str) -> dict:
    """Digest LOCAL sin depender de la app de Claude: DeepSeek redacta el
    resumen de la corrida nocturna y queda en logs/digest_last.md. La app a
    las 09:10 sigue siendo la capa interactiva (dale/no/ampliá)."""
    import json as _json
    out = _llm_json(
        "Sos el redactor del digest matutino de mmorch. Resumí esta corrida "
        "nocturna en español, corto y accionable (max 20 líneas de markdown): "
        "qué corrió, tarjetas/candidatas nuevas o maduradas, salud (componentes "
        "muertos, suites rojas de proyectos), branches esperando merge, errores. "
        "Sin relleno; si algo requiere acción humana, marcalo con ⚠️ o 🛡️.\n"
        f"Record JSON:\n{_json.dumps(rec, ensure_ascii=False, default=str)[:6000]}\n"
        'JSON: {"digest": str markdown}', schema=_DIGEST_SCHEMA)
    text = out.get("digest", "")
    path = Path(logs_dir) / "digest_last.md"
    path.write_text(text + "\n", encoding="utf-8")
    return {"path": str(path), "chars": len(text)}


def describe_projects(projects: dict, *, logs_dir: str, today: str) -> dict:
    """Registry enriquecido: descripcion 2-3 frases por proyecto (1 call c/u,
    solo los que faltan — incremental). El juez de adjudicacion la usa en vez
    del README-hack. Store: logs/projects_meta.json {name: {desc, updated}}."""
    meta_path = Path(logs_dir) / "projects_meta.json"
    meta = load_json_tolerant(meta_path, {})
    new = 0
    for name, path in sorted(projects.items()):
        if meta.get(name, {}).get("desc"):
            continue
        excerpt = ""
        for fn in ("README.md", "CLAUDE.md"):
            p = Path(path) / fn
            if p.exists():
                try:
                    excerpt = p.read_text(encoding="utf-8", errors="ignore")[:2500]
                except OSError:
                    pass
                break
        try:
            listing = ", ".join(x.name for x in sorted(Path(path).iterdir())[:25])
        except OSError:
            listing = ""
        out = _llm_json(
            f"Proyecto: {name} ({path})\nArchivos: {listing}\n"
            f"README/CLAUDE.md: {excerpt or '(no tiene)'}\n"
            "Describí en 2-3 frases QUÉ es este proyecto y qué tecnologías/temas "
            'toca (para que un juez decida si una nota de research le aplica). '
            'JSON: {"desc": str}', schema=_DESC_SCHEMA)
        meta[name] = {"desc": out.get("desc", ""), "updated": today}
        new += 1
    if new:
        atomic_write_json(meta_path, meta)
    # `pendientes` explicito: `described` es un contador INCREMENTAL (los que
    # faltaban esa noche) y se leia como "solo describio 1 de 12, esta
    # estancado" — la reflexion del 2026-08-24 lo diagnostico asi estando
    # 12/12 cubierto. Un 0 en pendientes cierra la lectura.
    return {"described": new, "total": len(meta),
            "pendientes": sum(1 for n in projects if not meta.get(n, {}).get("desc"))}


def _project_desc(project: str) -> str:
    """Desc del registry enriquecido (via _BUDGET_PATH -> logs dir); '' si no hay."""
    if _BUDGET_PATH is None:
        return ""
    meta = load_json_tolerant(_BUDGET_PATH.parent / "projects_meta.json", {})
    return meta.get(project, {}).get("desc", "")


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
        # registry enriquecido primero; README crudo como fallback
        readme = _project_desc(str(payload.get("project") or ""))
        if not readme:
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

    # jueces inyectados = modo test/mock: NADA del loop debe tocar la API real
    # (medido: un test corrio describe_projects contra el registry real y gasto
    # USD 0.000358 de verdad)
    real_mode = generator is None or verifier is None
    if real_mode:
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

    if real_mode:
        _step("describe_projects",
              lambda: describe_projects(_load_projects(), logs_dir=logs_dir,
                                        today=today))
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
