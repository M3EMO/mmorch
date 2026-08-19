"""Auto-auditoria — el juez de mmorch se mira a si mismo, modulo por modulo.

Motivo: hoy (2026-08-19) el mismo bug aparecio 3 veces en modulos distintos
(auto_repair, merge_train, project_repair — los tres releian nightly.jsonl
del disco en vez de usar el record EN MEMORIA de la corrida actual, todos con
1 noche de retraso o perdiendo datos). Ninguno de los tres se detecto solo:
salio de mirar el codigo a mano, tras un sintoma. Este modulo automatiza esa
mirada: 1 modulo/noche con la MISMA rubrica de principios que ya se usa para
juzgar repos ajenos (repo_mining.py), y una sintesis semanal que compara
resumenes entre modulos buscando el patron repetido (lo que ningun audit de
un solo modulo puede ver).

Igual que repo_mining: el juez propone, el refutador cross-family filtra, y
lo que sobrevive entra como candidata al circuito de SIEMPRE — dale humano.
Este modulo NUNCA edita codigo directo."""

from __future__ import annotations

from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_RETRY_DAYS = 30  # 119 modulos / 1 por noche ~ 4 meses por vuelta completa
_SKIP = {"__init__.py"}

_AUDIT_SCHEMA = {
    "type": "object",
    "properties": {
        "resumen": {"type": "string"},
        "findings": {"type": "array", "items": {
            "type": "object",
            "properties": {"titulo": {"type": "string"},
                           "severidad": {"type": "string"},
                           "categoria": {"type": "string"},
                           "detalle": {"type": "string"}},
            "required": ["titulo", "severidad", "detalle"]}},
    },
    "required": ["resumen", "findings"],
}
# categoria "estructural" = misma forma de bug que motivo este modulo: fuente
# de verdad duplicada, releer disco cuando hay dato en memoria, config/estado
# repetido en mas de un lugar. Es la clase de finding que un audit de UN
# modulo detecta menos (se ve mejor en audit_global) pero vale marcarla desde
# aca para que la sintesis semanal la pueda agrupar por categoria.
_CATEGORIAS = ("bug", "estructural", "principio", "otro")

_GLOBAL_SCHEMA = {
    "type": "object",
    "properties": {
        "patrones_repetidos": {"type": "array", "items": {"type": "string"}},
        "riesgo_principal": {"type": "string"},
        "recomendacion": {"type": "string"},
    },
    "required": ["patrones_repetidos", "riesgo_principal", "recomendacion"],
}


def pick_module(root: Path, state: dict, *, today: str) -> str | None:
    """El .py nunca auditado primero; despues el mas viejo en la ventana de
    reintento (cobertura pareja, no repetir el mismo par de veces seguidas)."""
    mods = sorted(f"mmorch/{m.name}" for m in (root / "mmorch").glob("*.py")
                  if m.name not in _SKIP)
    nunca = [m for m in mods if m not in state]
    if nunca:
        return nunca[0]
    elegibles = [m for m in mods if today > state[m].get("retry_after", "")]
    if not elegibles:
        return None
    return min(elegibles, key=lambda m: state[m].get("retry_after", ""))


def audit_module(module_rel: str, *, orch_root: str, today: str,
                 llm_fn=None, verify_fn=None) -> dict:
    """Un modulo: juez mapea findings (rubrica = principios propios) ->
    refutador filtra -> nota vault + candidatas + logs/self_audit.jsonl."""
    root = Path(orch_root)
    src_path = root / module_rel
    try:
        source = src_path.read_text(encoding="utf-8")[:16000]
    except OSError as e:
        return {"module": module_rel, "ok": False, "error": str(e)[:150]}

    principios = ""
    try:
        principios = (root / "docs" / "coding-principles.md").read_text(
            encoding="utf-8")[:2200]
    except OSError:
        pass

    if llm_fn is None:
        from mmorch.loop_nightly import _llm_json

        def llm_fn(prompt, schema):
            return _llm_json(prompt, schema=schema)
    out = llm_fn(
        "Sos el auditor interno de mmorch (te estas mirando a VOS MISMO, no "
        "un repo ajeno). Analiza este modulo linea por linea buscando 4 "
        "categorias de finding:\n"
        "- bug: comportamiento incorrecto real, no estilo.\n"
        "- estructural: FUENTE DE VERDAD duplicada o inconsistente — releer "
        "un archivo del disco cuando el llamador YA tiene ese dato en "
        "memoria (regla: docs/adr/0001-estado-en-memoria-no-releer-disco.md "
        "— violada 3 veces esta semana antes de escribirse); config/estado "
        "que vive en mas de un lugar; un modulo que podria reventar si otro "
        "cambia de forma silenciosa (acoplamiento oculto).\n"
        "- principio: viola TUS PROPIOS principios de codigo (abajo).\n"
        "- otro: cualquier cosa real que no entre en las 3 anteriores.\n"
        "Se especifico: cada finding apunta a un comportamiento concreto, no "
        "una sugerencia vaga. Pocos findings reales > muchos superficiales.\n"
        f"PRINCIPIOS (tu propia rubrica):\n{principios}\n"
        f"MODULO {module_rel}:\n{source}\n"
        'JSON: {"resumen": str, "findings": [{"titulo", "severidad": '
        '"alta|media|baja", "categoria": "bug|estructural|principio|otro", '
        '"detalle"}]}',
        _AUDIT_SCHEMA)

    findings = out.get("findings") or []
    if verify_fn is None:
        def verify_fn(f):
            from mmorch.loop_nightly import build_judges
            _, ver = build_judges()
            # el refutador intenta tirar el finding: si sobrevive, es real
            return not ver.refute(
                {"lente": "auditoria-interna",
                 "gist": f"{f['titulo']} ({f['severidad']}): {f['detalle']} "
                         f"— ¿es un bug/violacion REAL o una sugerencia vaga?"}
            ).get("refuted", True)
    survivors = [f for f in findings if verify_fn(f)]
    # estructural primero: la categoria que motivo este modulo (fuente de
    # verdad duplicada / releer disco vs memoria) es la de mayor valor —
    # tanto en la nota como en que candidatas se arman con el cupo de 3
    orden = {"estructural": 0, "bug": 1, "principio": 2, "otro": 3}
    survivors = sorted(survivors, key=lambda f: orden.get(f.get("categoria", "otro"), 3))
    conteo = {c: sum(1 for f in survivors if f.get("categoria", "otro") == c)
             for c in _CATEGORIAS}

    nota = root / "vault" / "research" / f"auditoria-{module_rel.replace('/', '_')}-{today}.md"
    cuerpo = "\n".join(
        f"- **{f['titulo']}** [{f['severidad']}/{f.get('categoria', 'otro')}]: {f['detalle']}"
        for f in survivors)
    nota.parent.mkdir(parents=True, exist_ok=True)
    nota.write_text(
        f"---\ntitle: auditoria {module_rel} {today}\nstatus: seed\n"
        f"tags: [mmorch, self-audit]\ncreated: {today}\n---\n\n"
        f"{out.get('resumen', '')}\n\n## Findings "
        f"(sobrevivieron refutacion {len(survivors)}/{len(findings)} — "
        f"{conteo['estructural']} estructurales, {conteo['bug']} bugs, "
        f"{conteo['principio']} de principios)\n\n"
        f"{cuerpo}\n", encoding="utf-8")

    if survivors:
        from mmorch.fuel import parse_archivadas, parse_candidatos, render_candidatos
        cand_path = root / "vault" / "roadmaps" / "candidatos.md"
        md = cand_path.read_text(encoding="utf-8")
        vig, arch = parse_candidatos(md), parse_archivadas(md)
        from datetime import date, timedelta
        vence = (date.fromisoformat(today) + timedelta(days=14)).isoformat()
        existing_today = sum(1 for e in vig + arch if e["id"].startswith(today))
        for i, f in enumerate(survivors[:3], start=existing_today + 1):
            vig.append({"id": f"{today}-{i:02d}", "fecha": today,
                        "vence": vence, "lente": "self-audit",
                        "gist": f"{module_rel}: {f['titulo']} "
                                f"[{f.get('categoria', 'otro')}] — {f['detalle']} "
                                f"(ver auditoria-{module_rel.replace('/', '_')}-{today})",
                        "estado": "pendiente"})
        cand_path.write_text(render_candidatos(vig, arch), encoding="utf-8")

    result = {"module": module_rel, "ok": True, "findings": len(findings),
              "sobrevivieron": len(survivors), "resumen": out.get("resumen", ""),
              "estructurales": conteo["estructural"]}
    logs = root / "logs"
    try:
        with open(logs / "self_audit.jsonl", "a", encoding="utf-8") as fh:
            import json
            fh.write(json.dumps({"fecha": today, **result},
                                ensure_ascii=False) + "\n")
    except OSError:
        pass
    return result


def run_one(orch_root: str, *, today: str, llm_fn=None, verify_fn=None) -> dict:
    """Vuelta nocturna: elige el proximo modulo en la rotacion y lo audita."""
    root = Path(orch_root)
    if (root / "logs" / "loop_paused").exists():
        return {"skipped": "paused"}
    state_path = root / "logs" / "self_audit_state.json"
    state = load_json_tolerant(state_path, {})
    target = pick_module(root, state, today=today)
    if target is None:
        return {"skipped": "sin modulo elegible (ventana de reintento)"}

    from datetime import date, timedelta
    retry_after = (date.fromisoformat(today) + timedelta(days=_RETRY_DAYS)).isoformat()
    res = audit_module(target, orch_root=orch_root, today=today,
                       llm_fn=llm_fn, verify_fn=verify_fn)
    state[target] = {"retry_after": retry_after,
                     "findings": res.get("sobrevivieron", 0)}
    atomic_write_json(state_path, state)
    return res


def audit_global(orch_root: str, *, today: str, n_modules: int = 10,
                 llm_fn=None) -> dict:
    """Sintesis semanal: lee los ultimos N resumenes de auditoria y busca el
    patron que NINGUN audit de un solo modulo puede ver — el mismo bug
    repetido en modulos distintos, convenciones inconsistentes, capas que se
    pisan. Ejemplo real que motivo esto: 3 modulos con la misma forma de bug
    (releer disco en vez de recibir el estado en memoria)."""
    import json
    root = Path(orch_root)
    logs = root / "logs"
    try:
        lines = (logs / "self_audit.jsonl").read_text(
            encoding="utf-8").strip().splitlines()[-n_modules:]
    except OSError:
        return {"skipped": "sin auditorias todavia"}
    if len(lines) < 3:
        return {"skipped": f"solo {len(lines)} auditorias (< 3, sin señal de patron)"}

    resumenes = []
    for ln in lines:
        try:
            r = json.loads(ln)
            estr = r.get("estructurales", 0)
            marca = f", {estr} ESTRUCTURALES" if estr else ""
            resumenes.append(f"- {r['module']} ({r['sobrevivieron']} findings{marca}): "
                             f"{r['resumen'][:200]}")
        except (json.JSONDecodeError, KeyError):
            continue

    if llm_fn is None:
        from mmorch.loop_nightly import _llm_json

        def llm_fn(prompt, schema):
            return _llm_json(prompt, schema=schema)
    out = llm_fn(
        "Sos la capa de sintesis de la auto-auditoria de mmorch. Cada noche "
        "un modulo distinto se audita solo; vos ves el resumen de los "
        "ultimos modulos auditados JUNTOS. Buscar: el mismo tipo de bug en "
        "mas de un modulo (convenciones inconsistentes, acoplamiento "
        "repetido, un patron que un audit de un solo archivo no puede ver). "
        "Se concreto: nombra los modulos involucrados.\n"
        f"Auditorias recientes:\n{chr(10).join(resumenes)}\n"
        'JSON: {"patrones_repetidos": [str], "riesgo_principal": str, '
        '"recomendacion": str}',
        _GLOBAL_SCHEMA)

    rec = {"fecha": today, **out}
    try:
        with open(logs / "self_audit_global.jsonl", "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec, ensure_ascii=False) + "\n")
    except OSError:
        pass
    return rec
