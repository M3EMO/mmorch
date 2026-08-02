"""babel — capa comprimida model-native del vault (paper 2606.19857).

El original es SIEMPRE la fuente de verdad (humano lee original). El `.babel.md`
es un derivado que consumen los modelos (menos tokens por lectura). Dos gates de
EJECUCION deciden si el babel se guarda — el LLM nunca gatea:
  1. ratio medido: si no comprime lo suficiente, babel no paga (medido 2026-07:
     notas ya densas quedan en 0.9 — solo prosa natural comprime).
  2. fidelidad QA cross-family: preguntas fácticas del original, respondidas por
     un LECTOR de otra familia que solo ve el babel; grading determinista por
     containment de tokens. El par compresor↔lector cross-family es el riesgo
     que el propio paper declara ("depends on the compressor-reader pair").

Medido 2026-07: deepseek necesita presupuesto DURO de caracteres en el prompt o
solo reformatea (95.6% sin límite vs 47.7% con límite, 10/10 fidelidad).
Medido 2026-08-02: deepseek-chat (v4-flash) IGNORA el presupuesto en docs >10k
(0.945-0.967, devuelve output idéntico ante reasks); gemini sí cumple
(gemini-2.5-flash-lite 0.531, gemini-2.5-flash 0.611 sobre el mismo doc) ->
encoder default = gemini, lector = deepseek (cross-family, dirección invertida).
"""
from __future__ import annotations

import json
import re
import shutil
from dataclasses import dataclass
from pathlib import Path

from .config import DEFAULT_GENERATOR, family_of
from .vault import VAULT

LEXICON = VAULT / "lexicon.md"
LEXICON_VERSION = "v1"
RATIO_MAX = 0.7        # comprime <30% -> no paga (dense notes medidas en 0.906)
FIDELITY_MIN = 0.8
DEFAULT_ENCODER = "gemini-2.5-flash-lite"   # medido: cumple el char budget
DEFAULT_READER = DEFAULT_GENERATOR          # deepseek: cross-family vs encoder


def _call_default(model: str, messages: list[dict]) -> str:
    from .providers import call as _call
    return _call(model, messages, pattern="babel", node="babel",
                 temperature=0.0).text


def _lexicon_text() -> str:
    return LEXICON.read_text(encoding="utf-8") if LEXICON.exists() else ""


def encode(text: str, *, model: str = DEFAULT_ENCODER,
           target_ratio: float = 0.5, call_fn=None) -> str:
    """Comprime `text` a babel con presupuesto DURO de caracteres.

    Un reask si el modelo se pasa del presupuesto (+15% tolerancia); si aun asi
    se pasa, devuelve lo que hay — el gate de ratio en ingest() decide.
    """
    call_fn = call_fn or _call_default
    budget = max(200, int(len(text) * target_ratio))
    # A/B medido 2026-08-02 (gemini-2.5-flash-lite, doc 15k): prompt minimal
    # 0.518; agregar el lexicon completo 0.934; agregar UNA linea de simbolos
    # 1.017. El vocabulario simbolico en el prompt del ENCODER rompe la
    # compresion -> encoder minimal; el lexicon es decoder key del LECTOR.
    sys_p = (
        "Comprimí a formato telegráfico ultra-denso (babel): mínimo de "
        "caracteres, máximo significado. Números exactos, nombres, autores, "
        "URLs y paths intactos. Salida: SOLO el texto comprimido.\n"
        f"HARD LIMIT: output <= {budget} caracteres.")

    def _ask(payload: str) -> str:
        # limite repetido al FINAL del user msg: medido 2026-07-30, deepseek
        # ignora el limite si solo va en system (README 19k -> 0.945 ratio)
        return call_fn(model, [
            {"role": "system", "content": sys_p},
            {"role": "user", "content":
             payload + f"\n\n[HARD LIMIT: tu output <= {budget} caracteres. "
             "Si no entra, comprimí más agresivo con el lexicon.]"}]).strip()

    out = _ask(text)
    for _ in range(2):                     # recomprime su propio output si se pasa
        if len(out) <= budget * 1.15:
            break
        out = _ask(out)
    return out


def _toks(t: str) -> set[str]:
    return {w for w in re.sub(r"[^\w]", " ", t.lower()).split() if len(w) > 1}


@dataclass
class Fidelity:
    score: float               # fraccion de respuestas correctas [0,1]
    n_questions: int
    misses: list[str]          # preguntas falladas (para debug/curado)


def fidelity(original: str, babel_text: str, *, n: int = 6,
             questioner: str = DEFAULT_ENCODER,
             reader: str = DEFAULT_READER, call_fn=None) -> Fidelity:
    """QA round-trip: preguntas fácticas del ORIGINAL, respondidas leyendo SOLO
    el babel por un modelo de OTRA familia que el compresor/questioner.
    Grading determinista: correcta si >=60% de los tokens del gold aparecen en
    la respuesta del lector (containment, no LLM-judge)."""
    if family_of(questioner) == family_of(reader):
        raise ValueError(
            f"babel: reader {reader} comparte familia con questioner {questioner} "
            "— el riesgo del paper es justamente el par cross-family.")
    call_fn = call_fn or _call_default
    qa_raw = call_fn(questioner, [{"role": "user", "content":
        f"Del texto siguiente extraé {n} preguntas fácticas con respuesta corta "
        "y verificable (números, nombres, hechos concretos). Respondé SOLO JSON: "
        '[{"q": "...", "a": "..."}]\n\nTEXTO:\n' + original}])
    m = re.search(r"\[.*\]", qa_raw, re.DOTALL)
    try:
        pairs = [p for p in json.loads(m.group(0) if m else qa_raw)
                 if isinstance(p, dict) and "q" in p and "a" in p][:n]
    except (json.JSONDecodeError, TypeError):
        pairs = []
    if not pairs:
        # questioner roto -> fidelidad 0: el gate skipea el babel (conservador),
        # el original queda intacto — nunca crashear el ingest por JSON del LLM.
        return Fidelity(score=0.0, n_questions=0, misses=["questioner sin JSON válido"])
    misses = []
    for p in pairs:
        ans = call_fn(reader, [
            {"role": "system", "content":
             "Respondé SOLO con la información del documento. Decoder key:\n"
             + _lexicon_text()},
            {"role": "user", "content":
             f"DOCUMENTO:\n{babel_text}\n\nPREGUNTA: {p['q']}\nRespuesta corta:"}])
        gold, got = _toks(str(p["a"])), _toks(ans)
        if gold and len(gold & got) / len(gold) < 0.6:
            misses.append(p["q"])
    n_q = len(pairs) or 1
    return Fidelity(score=round(1 - len(misses) / n_q, 3),
                    n_questions=len(pairs), misses=misses)


def ingest(path: str | Path, *, folder: str = "research",
           model: str = DEFAULT_ENCODER, reader: str = DEFAULT_READER,
           target_ratio: float = 0.5, call_fn=None) -> dict:
    """Mueve un archivo de research al vault global + genera su `.babel.md` si
    los gates pasan. Devuelve dict con ratio/fidelity/paths y `skipped` si el
    babel no pagó (el original SIEMPRE se ingesta igual)."""
    src = Path(path)
    text = src.read_text(encoding="utf-8")
    dst_dir = VAULT / folder
    dst_dir.mkdir(parents=True, exist_ok=True)
    dst = dst_dir / src.name
    if src.resolve() != dst.resolve():
        shutil.copy2(src, dst)
    out: dict = {"path": str(dst), "babel_path": None, "ratio": None,
                 "fidelity": None, "skipped": None}

    # 2 intentos: si la fidelidad falla, reintenta comprimiendo MENOS agresivo
    # (ratio sube -> fidelidad sube; varianza run-a-run medida: mismo doc dio
    # 1.0 y 0.5 en corridas consecutivas). Falla el 2do -> skip, original queda.
    b, ratio, fid = "", 0.0, None
    for tr in (target_ratio, min(target_ratio + 0.15, RATIO_MAX)):
        b = encode(text, model=model, target_ratio=tr, call_fn=call_fn)
        ratio = round(len(b) / max(len(text), 1), 3)
        out["ratio"] = ratio
        if ratio > RATIO_MAX:
            out["skipped"] = f"ratio {ratio} > {RATIO_MAX} (no paga)"
            continue
        fid = fidelity(text, b, questioner=model, reader=reader, call_fn=call_fn)
        out["fidelity"] = fid.score
        if fid.score >= FIDELITY_MIN:
            out["skipped"] = None
            break
        out["skipped"] = (f"fidelity {fid.score} < {FIDELITY_MIN}; "
                          f"misses: {fid.misses}")
    if out["skipped"] is not None or fid is None:
        return out
    bp = dst.with_suffix(".babel.md")
    bp.write_text(
        "---\n"
        f"source: {dst.name}\n"
        f"lexicon: {LEXICON_VERSION}\n"
        f"ratio: {ratio}\n"
        f"fidelity: {fid.score}\n"
        "derived: true\n"
        "---\n" + b + "\n", encoding="utf-8")
    out["babel_path"] = str(bp)
    return out


if __name__ == "__main__":
    # self-check cero-costo: call_fn inyectado, vault real no se toca (tmp).
    import tempfile

    # 1. encode: presupuesto duro en system Y user + recompresión si se pasa
    seen: list[list[dict]] = []

    def _fake_encode(m, msgs):
        seen.append(msgs)
        return "x" * 5000 if len(seen) == 1 else "compacto → ✓"
    long_text = "palabra " * 1000                     # 8000 chars, budget 4000
    b = encode(long_text, call_fn=_fake_encode)
    assert "HARD LIMIT" in seen[0][0]["content"], "limite ausente en system"
    assert "HARD LIMIT" in seen[0][1]["content"], "limite ausente en user"
    assert len(seen) == 2 and b == "compacto → ✓", "recompresión no disparó"
    assert "x" * 100 in seen[1][1]["content"], "2do intento no recomprime el output propio"

    # 2. fidelity: grading determinista por containment
    def _fake_qa(m, msgs):
        if "preguntas fácticas" in msgs[-1]["content"]:
            return '[{"q": "timeout?", "a": "1800 segundos"}, {"q": "modelo?", "a": "deepseek"}]'
        return ("el timeout es 1800 segundos" if "timeout" in msgs[-1]["content"]
                else "ni idea")
    f = fidelity("orig", "babel", call_fn=_fake_qa)
    assert f.score == 0.5 and f.misses == ["modelo?"], f

    # 2b. questioner devuelve basura -> fidelidad 0 conservadora, sin crash
    fb = fidelity("o", "b", call_fn=lambda m, msgs: "no soy json {")
    assert fb.score == 0.0 and fb.n_questions == 0, fb

    # 3. cross-family enforced
    try:
        fidelity("o", "b", questioner="deepseek-chat", reader="deepseek-chat",
                 call_fn=_fake_qa)
        raise AssertionError("misma familia debió fallar")
    except ValueError:
        pass

    # 4. ingest gates: ratio malo -> skip babel pero original ingresa igual
    # (patch de globals() del modulo EN EJECUCION: bajo `python -m`, __main__ e
    # `import mmorch.babel` son objetos modulo DISTINTOS — patchear el import
    # dejaria ingest() escribiendo al vault real)
    with tempfile.TemporaryDirectory() as td:
        _orig_vault = VAULT
        globals()["VAULT"] = Path(td)
        try:
            src = Path(td) / "nota.md"
            src.write_text("hola " * 200, encoding="utf-8")
            r = ingest(src, folder="research",
                       call_fn=lambda m, msgs: "hola " * 195)  # ratio ~0.97
            assert r["skipped"] and "ratio" in r["skipped"] and r["babel_path"] is None
            assert (Path(td) / "research" / "nota.md").exists(), "original no ingresó"

            # 5. gates verdes -> .babel.md con frontmatter
            def _good(m, msgs):
                c = msgs[-1]["content"]
                if "preguntas fácticas" in c:
                    return '[{"q": "q1", "a": "hola"}]'
                if "PREGUNTA" in c:
                    return "hola"
                return "hola " * 40                    # ratio ~0.2
            r2 = ingest(src, folder="research", call_fn=_good)
            assert r2["babel_path"] and r2["skipped"] is None, r2
            bt = Path(r2["babel_path"]).read_text(encoding="utf-8")
            assert "lexicon: v1" in bt and "derived: true" in bt
        finally:
            globals()["VAULT"] = _orig_vault
    print("babel self-check OK (6 propiedades)")
