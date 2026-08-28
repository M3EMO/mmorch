"""wayfinder-prep — investigación autónoma de tickets, decisión humana.

Regla dura (CLAUDE.md global): resolver tickets es HITL — jamás responder el
propio grilling. Un wayfinder que se contesta solo produce un mapa de los
priors del modelo disfrazado de decisión del dueño: deriva de objetivo con
papeles en regla. Este módulo automatiza SOLO lo anterior a decidir:

  por ticket → evidencia real del repo (grep-lite, sin API) → 2-3 opciones
  con costo/riesgo + recomendación (generador barato) → el refutador
  cross-family marca la opción floja → dossier a .scratch/<mapa>/prep.md

El output JAMÁS es la respuesta del ticket: es el terreno explorado para que
el humano conteste en minutos lo que hoy toma una sesión. El footer de cada
dossier lo dice explícito.

Presupuesto: usa _llm_json de loop_nightly (mismo tope USD/calls del loop —
un mapa de 7 tickets son ~14 llamadas baratas, no un agujero).
"""

from __future__ import annotations

import re
from pathlib import Path

_PREP_SCHEMA = {
    "type": "object",
    "properties": {
        "contexto": {"type": "string"},
        "opciones": {"type": "array", "items": {
            "type": "object",
            "properties": {"titulo": {"type": "string"},
                           "como": {"type": "string"},
                           "costo": {"type": "string"},
                           "riesgo": {"type": "string"}},
            "required": ["titulo", "como", "costo", "riesgo"]}},
        "recomendacion": {"type": "string"},
        "pregunta_abierta": {"type": "string"},
    },
    "required": ["contexto", "opciones", "recomendacion", "pregunta_abierta"],
}

_STOP = {"para", "como", "esto", "esta", "este", "donde", "cual", "cuales",
         "entre", "sobre", "deberia", "podria", "hacer", "tener", "queremos",
         "mmorch", "sistema"}


def _evidence(root: Path, question: str, *, max_files: int = 5,
              max_chars: int = 7000) -> str:
    """Grep-lite sin API: palabras clave de la pregunta contra mmorch/*.py,
    docs/ y vault/roadmaps — los archivos con mas hits aportan su cabeza.
    Suficiente para anclar las opciones en el codigo REAL (una opcion que
    ignora lo ya construido es la clase de error mas cara del grilling)."""
    words = {w for w in re.findall(r"[a-záéíóúñ_]{4,}", question.lower())
             if w not in _STOP}
    if not words:
        return ""
    scored: list[tuple[int, Path]] = []
    for pat in ("mmorch/*.py", "docs/**/*.md", "vault/roadmaps/*.md"):
        for f in root.glob(pat):
            try:
                text = f.read_text(encoding="utf-8", errors="ignore").lower()
            except OSError:
                continue
            hits = sum(text.count(w) for w in words)
            if hits:
                scored.append((hits, f))
    scored.sort(key=lambda t: -t[0])
    parts = []
    total = 0
    for hits, f in scored[:max_files]:
        head = f.read_text(encoding="utf-8", errors="ignore")[:1500]
        parts.append(f"== {f.relative_to(root)} ({hits} hits) ==\n{head}")
        total += len(head)
        if total >= max_chars:
            break
    return "\n\n".join(parts)[:max_chars]


def prep_ticket(question: str, *, orch_root: str, llm_fn=None,
                refute_fn=None) -> dict:
    """Dossier de UN ticket: contexto + opciones con costo/riesgo +
    recomendacion + la pregunta que queda genuinamente abierta. El refutador
    cross-family intenta tirar cada opcion; las que no sobreviven quedan
    marcadas 'sospechosa' (no se borran: que el humano vea el descarte)."""
    root = Path(orch_root)
    ev = _evidence(root, question)

    if llm_fn is None:
        from mmorch.loop_nightly import _llm_json

        def llm_fn(prompt, schema):
            return _llm_json(prompt, schema=schema)
    out = llm_fn(
        "Sos el investigador de un wayfinder (mapa de decisiones) de mmorch. "
        "NO respondas la pregunta — prepara el terreno para que el DUEÑO "
        "decida: contexto de que existe hoy en el codigo, 2-3 opciones "
        "REALES con costo y riesgo concretos, una recomendacion con su "
        "porque, y la pregunta que solo el dueño puede contestar (la que "
        "depende de SU intencion, no de evidencia).\n"
        f"TICKET: {question}\n"
        f"EVIDENCIA DEL REPO:\n{ev}\n"
        'JSON: {"contexto": str, "opciones": [{"titulo","como","costo",'
        '"riesgo"}], "recomendacion": str, "pregunta_abierta": str}',
        _PREP_SCHEMA)

    opciones = out.get("opciones") or []
    if refute_fn is None:
        from mmorch.loop_nightly import build_judges
        _, ver = build_judges()

        def refute_fn(o):
            return ver.refute({"lente": "wayfinder-prep",
                               "gist": f"{o['titulo']}: {o['como']} "
                                       f"(costo {o['costo']}, riesgo {o['riesgo']})"
                              }).get("refuted", False)
    for o in opciones:
        o["sospechosa"] = bool(refute_fn(o))
    return {"ticket": question, "contexto": out.get("contexto", ""),
            "opciones": opciones,
            "recomendacion": out.get("recomendacion", ""),
            "pregunta_abierta": out.get("pregunta_abierta", "")}


def prep_map(nombre: str, preguntas: list[str], *, orch_root: str,
             llm_fn=None, refute_fn=None) -> str:
    """Dossier completo del mapa: un prep.md en .scratch/<nombre>/ con una
    seccion por ticket. Devuelve el path escrito."""
    root = Path(orch_root)
    out_dir = root / ".scratch" / nombre
    out_dir.mkdir(parents=True, exist_ok=True)

    secciones = []
    for i, q in enumerate(preguntas, 1):
        d = prep_ticket(q, orch_root=orch_root, llm_fn=llm_fn,
                        refute_fn=refute_fn)
        ops = "\n".join(
            f"- **{o['titulo']}**{' ⚠ sospechosa (el refutador la tira)' if o.get('sospechosa') else ''}\n"
            f"  - cómo: {o['como']}\n  - costo: {o['costo']}\n  - riesgo: {o['riesgo']}"
            for o in d["opciones"])
        secciones.append(
            f"## Ticket {i}: {d['ticket']}\n\n{d['contexto']}\n\n"
            f"### Opciones\n{ops}\n\n"
            f"### Recomendación del investigador\n{d['recomendacion']}\n\n"
            f"### ⟶ Lo que SOLO VOS podés contestar\n{d['pregunta_abierta']}\n")

    path = out_dir / "prep.md"
    path.write_text(
        f"# Prep del mapa `{nombre}` — dossier, NO decisiones\n\n"
        "Investigación autónoma (evidencia del repo + opciones refutadas "
        "cross-family). **Ningún ticket está respondido**: la regla es HITL — "
        "las respuestas salen de vos, esto solo te ahorra la exploración.\n\n"
        + "\n---\n\n".join(secciones)
        + "\n\n---\n*Generado por wayfinder_prep — decidir es tuyo.*\n",
        encoding="utf-8")
    return str(path)
