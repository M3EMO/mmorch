"""rubric_loop — LOOP DE AUTOCORRECCION CON VERIFICADOR INDEPENDIENTE (spec del usuario).

Roles:
  PLANEADOR  = quien define task+criteria (humano o Fable en sesion). No vive aca.
  GERENTE    = ESTE MOTOR: Python determinista. Decide a quien le toca, corre checkers,
               aplica K/parada/escalada, cierra el lazo (bandit+memoria). No es un LLM.
  EJECUTOR   = produce el intento. Transporte PLUGGABLE (ver abajo).
  JUEZ       = evalua SOLO criterios subjetivos, contexto separado, refuta por default.
               Los criterios CHECKABLES los juzga un checker determinista ($0, sin LLM):
               medimos ~74% false-refute de jueces LLM en checkable dificil — donde hay
               evidencia ejecutable, el LLM sobra.

Transporte (la respuesta a "gastar del plan y no de API"):
  - MODO API : run_rubric_loop() llama DeepSeek (gen) + Gemini (juez) via providers.call.
               Par SIEMPRE cross-family (OneFlow).
  - MODO PLAN: el motor es una MAQUINA DE ESTADOS con estado JSON-serializable.
               start() -> next_action() -> submit() -> ... La sesion Claude (plan, cupo)
               ejecuta cada accion con subagentes y devuelve el output. mmorch conduce,
               el plan genera. CERO API. (MCP: mmorch_rubric_start/next/submit.)
  En ambos modos los checkers corren server-side: la evidencia es EJECUCION local,
  jamas el reporte del ejecutor (anti-engano: el juez/checker re-ejecuta, no lee claims).

Cierre del lazo (al terminar, done o escalate):
  reward = fraccion de rubrica cumplida -> record_outcome(pattern='rubric_loop',
  context=task) — alimenta bandit + ShadowPrior (Fase 5). Si hubo correcciones y
  termino verde, DESTILA una regla verificada a memoria semantica (write_note):
  la progresion FALLAR->INVESTIGAR->VERIFICAR->DESTILAR->CONSULTAR del spec.
"""
from __future__ import annotations

import json
import re
import uuid

from .checkers import check
from .config import family_of

from .textutil import extract_fence as _extract_block  # dedup of the local fence helper


# --------------------------------------------------------------------------- #
# Estado (dict JSON-serializable: viaja por MCP, resumible entre turnos)
# --------------------------------------------------------------------------- #
def start_rubric(task: str, criteria: list[dict], *, K: int = 5, arm: str = "",
                 gen_model: str | None = None,
                 judge_model: str | None = None,
                 scout: bool = False, scout_llm: bool = False,
                 enrich: bool = False) -> dict:
    """Crea el estado del loop. criteria = lista de:
      {"id": str, "desc": str, "kind": "checkable", "checker": str, "ctx": {...}}
        — ctx admite placeholders "{attempt}" (texto crudo) y "{attempt_code}"
          (bloque de codigo extraido del intento).
      {"id": str, "desc": str, "kind": "subjective"}
    Regla dura OneFlow: en modo API gen y judge deben ser cross-family."""
    from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
    gen_model = gen_model or DEFAULT_GENERATOR
    judge_model = judge_model or DEFAULT_VERIFIER
    # Enrich (Fable 5): completar el prompt infiriendo intent, con guard cross-family.
    enriched_flag = False
    if enrich:
        try:
            from .enrich import enrich_prompt
            task = enrich_prompt(task, gen_model=gen_model, judge_model=judge_model)["enriched"]
            enriched_flag = True
        except Exception:
            pass
    for c in criteria:
        if c.get("kind") not in ("checkable", "subjective"):
            raise ValueError(f"criterio {c.get('id')}: kind invalido")
        if c["kind"] == "checkable" and not c.get("checker"):
            raise ValueError(f"criterio {c.get('id')}: checkable sin checker")
    if family_of(gen_model) == family_of(judge_model):
        raise ValueError("OneFlow: gen y judge deben ser de familias DISTINTAS "
                         f"({gen_model} vs {judge_model})")
    # Scout entorno-primero (Fable 5): brief de grounding como prefijo estable del ejecutor.
    # Determinista por default ($0); scout_llm=True suma una call barata de exploracion.
    scout_brief = ""
    if scout:
        try:
            from .scout import scout as _scout
            scout_brief = _scout(task, criteria, use_llm=scout_llm, gen_model=gen_model).get("brief", "")
        except Exception:
            scout_brief = ""
    return {
        "id": uuid.uuid4().hex[:10], "task": task, "criteria": criteria,
        "K": int(K), "iteration": 0, "attempt": "", "phase": "executor",
        "results": {}, "history": [], "arm": arm,
        "gen_model": gen_model, "judge_model": judge_model,
        "scout_brief": scout_brief, "enriched": enriched_flag,
    }


def _pending(state: dict, kind: str | None = None) -> list[dict]:
    out = []
    for c in state["criteria"]:
        r = state["results"].get(c["id"])
        if r and r.get("cumple"):
            continue
        if kind is None or c["kind"] == kind:
            out.append(c)
    return out


def _rubric_text(state: dict) -> str:
    lines = []
    for c in state["criteria"]:
        r = state["results"].get(c["id"])
        mark = "OK" if (r and r.get("cumple")) else "PENDIENTE"
        lines.append(f"- [{c['id']}] ({mark}) {c['desc']}")
        if r and not r.get("cumple") and r.get("correccion"):
            lines.append(f"    correccion requerida: {r['correccion']}")
        if r and not r.get("cumple") and r.get("evidencia"):
            lines.append(f"    evidencia del fallo: {str(r['evidencia'])[:300]}")
    return "\n".join(lines)


def next_action(state: dict) -> dict:
    """Que toca ahora. {"role": "executor"|"judge", "prompt": ...} o
    {"role": "done"|"escalate", ...}. El GERENTE es esta funcion: deterministico."""
    if state["phase"] == "done":
        return {"role": "done", "summary": _summary(state)}
    if state["phase"] == "escalate":
        return {"role": "escalate", "summary": _summary(state)}
    if state["phase"] == "executor":
        # brief de scout PRIMERO (prefijo estable cacheable + grounding entorno-primero)
        ground = (state.get("scout_brief", "") + "\n\n") if state.get("scout_brief") else ""
        prompt = (
            ground
            + f"TAREA:\n{state['task']}\n\nRUBRICA (todos los criterios deben cumplirse):\n"
            f"{_rubric_text(state)}\n\n"
            + ("Tu intento anterior:\n```\n" + state["attempt"][:4000] + "\n```\n"
               "Corregi PRIORIZANDO los criterios PENDIENTES.\n" if state["attempt"] else "")
            + "Devolve SOLO el artefacto (codigo en un bloque ```), sin explicacion.")
        return {"role": "executor", "prompt": prompt}
    # phase == judge: solo criterios subjetivos pendientes
    crits = _pending(state, "subjective")
    listado = "\n".join(f"- id={c['id']}: {c['desc']}" for c in crits)
    prompt = (
        "Sos un JUEZ independiente y ESCEPTICO. NO generaste este trabajo. Evalua el "
        "intento SOLO contra los criterios listados. Refuta por default: si la evidencia "
        "no es clara, el criterio NO se cumple. NO relajes ni reinterpretes criterios.\n\n"
        f"TAREA original:\n{state['task']}\n\nINTENTO:\n```\n{state['attempt'][:6000]}\n```\n\n"
        f"CRITERIOS a evaluar:\n{listado}\n\n"
        'Devolve SOLO JSON: [{"id": "...", "cumple": true|false, '
        '"evidencia": "...", "correccion": "..."}]')
    return {"role": "judge", "prompt": prompt}


def submit(state: dict, output: str) -> dict:
    """Entrega el output del rol actual al GERENTE. Muta y devuelve el estado."""
    from .events import emit
    jid = state.get("id", "")
    if state["phase"] == "executor":
        state["attempt"] = output
        state["iteration"] += 1
        state["history"].append({"iter": state["iteration"], "role": "executor",
                                 "chars": len(output)})
        emit("step", "running", job_id=jid, node="checkers",
             detail=f"iter {state['iteration']}: ejecutando checkers")
        _run_checkables(state)
        for c in state["criteria"]:
            if c.get("kind") == "checkable":
                r = state["results"].get(c["id"], {})
                emit("step", "done" if r.get("cumple") else "error", job_id=jid,
                     node=f"check:{c.get('checker')}", detail=str(r.get("evidencia", ""))[:120])
        # traza pal flywheel (idea Hermes trajectory-compression): codigo del paso +
        # que criterios fallaban EN ese paso. Cada paso = ejemplo (code, label=ejecucion).
        failed = [c["id"] for c in state["criteria"]
                  if not state["results"].get(c["id"], {}).get("cumple", False)]
        state.setdefault("trace", []).append(
            {"iter": state["iteration"], "code": _extract_block(output)[:4000],
             "failed": failed})
        if _pending(state, "subjective"):
            state["phase"] = "judge"
        else:
            _maybe_finish(state)
        return state
    if state["phase"] == "judge":
        emit("step", "running", job_id=jid, node=f"judge:{state.get('judge_model')}",
             detail="evaluando criterios subjetivos")
        _apply_judge(state, output)
        _maybe_finish(state)
        return state
    raise ValueError(f"submit en phase terminal: {state['phase']}")


def _run_checkables(state: dict) -> None:
    """JUEZ determinista: re-EJECUTA. La evidencia sale del sandbox/checker local,
    nunca del reporte del ejecutor."""
    raw = state["attempt"]
    code = _extract_block(raw)
    for c in _pending(state, "checkable"):
        ctx = {}
        for k, v in (c.get("ctx") or {}).items():
            if isinstance(v, str):
                v = v.replace("{attempt_code}", code).replace("{attempt}", raw)
            ctx[k] = v
        try:
            r = check(c["checker"], **ctx)
            state["results"][c["id"]] = {
                "cumple": bool(r.passed), "evidencia": r.detail,
                "correccion": "" if r.passed else f"hacer pasar el checker {c['checker']}",
                "juez": f"checker:{c['checker']}"}
        except Exception as e:
            state["results"][c["id"]] = {
                "cumple": False, "evidencia": f"checker error: {str(e)[:200]}",
                "correccion": "intento invalido para el checker",
                "juez": f"checker:{c['checker']}"}


def _apply_judge(state: dict, output: str) -> None:
    try:
        verdicts = json.loads(_extract_block(output))
        assert isinstance(verdicts, list)
    except Exception:
        # juez ilegible = nadie aprobado (refute by default), reintenta proxima vuelta
        state["history"].append({"iter": state["iteration"], "role": "judge",
                                 "error": "json ilegible"})
        state["phase"] = "executor"
        return
    valid = {c["id"] for c in state["criteria"] if c["kind"] == "subjective"}
    for v in verdicts:
        cid = v.get("id")
        if cid not in valid:
            continue
        state["results"][cid] = {
            "cumple": bool(v.get("cumple")), "evidencia": str(v.get("evidencia", ""))[:500],
            "correccion": str(v.get("correccion", ""))[:500], "juez": state["judge_model"]}
    # subjetivo sin veredicto = no cumple (el juez no lo confirmo)
    for cid in valid:
        if cid not in state["results"]:
            state["results"][cid] = {"cumple": False, "evidencia": "sin veredicto del juez",
                                     "correccion": "re-evaluar", "juez": state["judge_model"]}
    state["history"].append({"iter": state["iteration"], "role": "judge",
                             "ok": sum(1 for r in state["results"].values() if r["cumple"])})


def _maybe_finish(state: dict) -> None:
    from .events import emit
    jid = state.get("id", "")
    if not _pending(state):
        state["phase"] = "done"
        emit("job", "done", job_id=jid, detail=f"rubrica 100% en {state['iteration']} iter")
        _close_loop(state)
    elif state["iteration"] >= state["K"]:
        state["phase"] = "escalate"
        emit("job", "gate", job_id=jid, detail=f"K={state['K']} agotado -> escala a humano")
        _close_loop(state)
    else:
        state["phase"] = "executor"
        emit("step", "running", job_id=jid, node="executor", detail="corrigiendo pendientes")


def _summary(state: dict) -> dict:
    total = len(state["criteria"])
    ok = sum(1 for r in state["results"].values() if r.get("cumple"))
    pend = [{"id": c["id"], "desc": c["desc"],
             **state["results"].get(c["id"], {"evidencia": "nunca evaluado"})}
            for c in _pending(state)]
    return {"cumplidos": ok, "total": total, "iteraciones": state["iteration"],
            "pendientes": pend, "attempt": state["attempt"]}


def _close_loop(state: dict) -> None:
    """Bandit + memoria. reward = fraccion cumplida (verificada, no autoreporte)."""
    total = len(state["criteria"]) or 1
    ok = sum(1 for r in state["results"].values() if r.get("cumple"))
    reward = ok / total
    arm = state["arm"] or f"rubric:{state['gen_model']}"
    try:
        from .feedback import record_outcome
        record_outcome(arm, reward, pattern="rubric_loop", source="rubric_evidence",
                       context=state["task"][:2000])
    except Exception:
        pass
    # DESTILAR: termino verde DESPUES de corregir => hay una leccion verificada
    if state["phase"] == "done" and state["iteration"] > 1:
        try:
            from .memory import write_note
            fixed = [f"{c['id']}: {state['results'][c['id']].get('correccion') or c['desc']}"
                     for c in state["criteria"]
                     if any(h.get("role") == "judge" for h in state["history"])]
            write_note(
                "rubric_loop",
                f"[regla verificada] tarea='{state['task'][:120]}' "
                f"costo {state['iteration']} iteraciones. Criterios que fallaron primero y "
                f"como se corrigieron: {('; '.join(fixed))[:600]}",
                verified=True)
        except Exception:
            pass
    # Captura/compresion de trayectoria (idea Hermes) -> dataset del flywheel + skill.
    try:
        from .trajectory import record_trajectory
        record_trajectory(state)
    except Exception:
        pass
    # nudge (idea Hermes): cada N cierres, mantenimiento de memoria automatico.
    try:
        from .nudge import tick
        tick()
    except Exception:
        pass


# --------------------------------------------------------------------------- #
# MODO API: loop completo automatico (DeepSeek genera, Gemini juzga, centavos)
# --------------------------------------------------------------------------- #
def run_rubric_loop(task: str, criteria: list[dict], *, K: int = 5,
                    gen_model: str | None = None,
                    judge_model: str | None = None,
                    gen_fn=None, judge_fn=None, arm: str = "",
                    gen_for_round=None) -> dict:
    """Modo API (o cualquier transporte via gen_fn/judge_fn inyectados — asi un test
    o el modo plan reusan el MISMO gerente). Devuelve el estado final.

    lcw (loop-conditioned specialization): `gen_for_round(round:int) -> gen_fn` permite
    ESCALAR el nodo ejecutor por ronda (round 1-based) en vez de un solo modelo fijo —
    ej. ronda 1 modelo barato/amplio, ronda 3 modelo fuerte/correctivo. Si se pasa, pisa
    a gen_fn/gen_model; si no, comportamiento single-node de siempre (fallback). Cada
    modelo que devuelva DEBE ser cross-family vs el judge (responsabilidad del caller —
    el gate OneFlow de start_rubric solo valida el par base)."""
    from .providers import call as _call
    from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
    gen_model = gen_model or DEFAULT_GENERATOR
    judge_model = judge_model or DEFAULT_VERIFIER

    def _api(model):
        def fn(prompt):
            r = _call(model, [{"role": "user", "content": prompt}],
                      pattern="rubric_loop", node=model)
            return r.text
        return fn

    gen_fn = gen_fn or _api(gen_model)
    judge_fn = judge_fn or _api(judge_model)
    state = start_rubric(task, criteria, K=K, arm=arm,
                         gen_model=gen_model, judge_model=judge_model)
    while True:
        act = next_action(state)
        if act["role"] in ("done", "escalate"):
            return state
        if act["role"] == "executor":
            # round = iteration+1 (submit incrementa DESPUES de generar)
            g = gen_for_round(state["iteration"] + 1) if gen_for_round else gen_fn
            out = g(act["prompt"])
        else:
            out = judge_fn(act["prompt"])
        submit(state, out)
