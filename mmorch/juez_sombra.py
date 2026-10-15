"""juez_sombra — opina sobre el residuo NO computable de un entregable, de a pares y sin bloquear (builder-e2e ticket 09).

Generaliza el juez visual de sprites.py: compara dos entregables (A y B) con una rubrica con anclas y criterios
binarios, sin puntajes numericos (medido 2026-09-17: un juez VLM ordena pero puntua mal). Cada par se juzga dos
veces con el orden invertido; si los dos ordenes no coinciden, la opinion es "inconsistente" (sesgo de posicion).
Todo va a logs/sdlc/juez_sombra.jsonl. Sale de sombra solo con kappa >= KAPPA_MIN contra pares etiquetados por el
usuario (`confiable`). Texto: DEFAULT_VERIFIER (razona). Imagen: el VLM de sprites.
"""
from __future__ import annotations

import base64
import json
import time
from pathlib import Path

from .config import DEFAULT_VERIFIER
from .paths import logs_dir
from .sprites import JUEZ as VLM
from .sprites import kappa

KAPPA_MIN, MIN_PARES = 0.6, 20
RUBRICAS = {   # anclas del ticket 03 (research de builder-e2e)
    "seo_texto": {
        "pregunta": "¿Cuál texto sirve mejor a quien busca calcular su sueldo neto en Argentina?",
        "anclas": {1: "genérico o de relleno, sin explicar aportes, con keyword stuffing o datos sin fecha",
                   3: "explica aportes y Ganancias con fecha de vigencia, pero sin ejemplo ni preguntas frecuentes",
                   5: "responde la intención en el primer párrafo, ejemplo con números, preguntas frecuentes y fuentes "
                      "oficiales enlazadas, en español rioplatense natural"},
        "criterios": ["explica los aportes", "da la fecha de vigencia de las tablas", "trae un ejemplo con números",
                      "enlaza fuentes oficiales", "no repite palabras clave de forma artificial"]},
    "visual": {
        "pregunta": "¿Cuál página se ve y se usa mejor?",
        "anclas": {1: "el resultado no se ve sin scroll en móvil, hay superposiciones o contraste pobre",
                   3: "limpia y usable, pero el desglose es plano y la jerarquía débil",
                   5: "el neto resalta, el desglose es legible por línea, consistente en móvil y escritorio y muestra "
                      "la fecha de las tablas"},
        "criterios": ["el neto resalta", "el desglose es legible", "se usa bien en móvil", "buen contraste"]},
}


def _sombra() -> Path:
    return logs_dir() / "sdlc" / "juez_sombra.jsonl"


def _imagen(p: Path) -> dict:
    return {"type": "image_url", "image_url": {"url": "data:image/png;base64," + base64.b64encode(p.read_bytes()).decode()}}


def _una_vez(primero: Path, segundo: Path, rubrica: str, modelo: str, llamar) -> dict:
    r = RUBRICAS[rubrica]
    texto = (f"Comparás dos entregables. {r['pregunta']}\nAnclas de referencia (no las uses como puntaje): "
             + "; ".join(f"{k} = {v}" for k, v in r["anclas"].items())
             + '.\nResponde SOLO un JSON: {"mejor": "primero"|"segundo"|"empate", "criterios": {"primero": {'
             + ", ".join(f'"{c}": true|false' for c in r["criterios"]) + '}, "segundo": {...}}, "motivo": "<una linea>"}')
    if rubrica == "visual":
        partes = [{"type": "text", "text": texto + "\nPrimero: escritorio y móvil. Segundo: escritorio y móvil."}]
        partes += [_imagen(d / f) for d in (primero, segundo) for f in ("escritorio.png", "movil.png")]
        mensaje = [{"role": "user", "content": partes}]
    else:
        mensaje = [{"role": "user", "content": texto + "\n\nPRIMERO:\n" + (primero / "texto.txt").read_text(encoding="utf-8")
                    + "\n\nSEGUNDO:\n" + (segundo / "texto.txt").read_text(encoding="utf-8")}]
    if llamar is None:
        from .providers import call as llamar
    res = llamar(modelo, mensaje, pattern="juez-sombra", node=modelo, phase=f"juez-{rubrica}", temperature=0.0,
                 max_tokens=4096, timeout=180)
    crudo = res.text if hasattr(res, "text") else str(res)
    try:
        return json.loads(crudo[crudo.index("{"):crudo.rindex("}") + 1])
    except ValueError:
        return {"mejor": "", "motivo": f"respuesta no parseable: {crudo[:200]}"}


def comparar(a: Path, b: Path, rubrica: str, *, modelo: str | None = None, llamar=None) -> dict:
    """Opinion de a pares sobre las capturas del driver (carpetas con texto.txt y PNGs). NUNCA bloquea."""
    modelo = modelo or (VLM if rubrica == "visual" else DEFAULT_VERIFIER)
    try:
        ab = _una_vez(a, b, rubrica, modelo, llamar).get("mejor", "")
        ba = _una_vez(b, a, rubrica, modelo, llamar).get("mejor", "")
    except Exception as e:  # noqa: BLE001 — la sombra nunca puede tumbar una corrida
        ab, ba, error = "", "", str(e)[:200]
    else:
        error = ""
    directo = {"primero": "A", "segundo": "B"}.get(ab, ab)
    invertido = {"primero": "B", "segundo": "A"}.get(ba, ba)
    mejor = directo if directo == invertido else "inconsistente"
    d = {"ts": time.time(), "rubrica": rubrica, "a": str(a), "b": str(b), "modelo": modelo, "mejor": mejor,
         "orden_ab": directo, "orden_ba": invertido, "error": error}
    _sombra().parent.mkdir(parents=True, exist_ok=True)
    with _sombra().open("a", encoding="utf-8") as f:
        f.write(json.dumps(d, ensure_ascii=False) + "\n")
    return d


def confiable(etiquetas: list[dict], sombra: list[dict] | None = None) -> tuple[bool, str]:
    """etiquetas = [{"a", "b", "rubrica", "label": "A"|"B"|"empate"}] del usuario. Sale de sombra con kappa >= KAPPA_MIN."""
    if sombra is None:
        sombra = [json.loads(x) for x in _sombra().read_text(encoding="utf-8").splitlines() if x.strip()] \
            if _sombra().exists() else []
    ultima = {(d["a"], d["b"], d["rubrica"]): d["mejor"] for d in sombra}
    pares = [(ultima[(e["a"], e["b"], e["rubrica"])], e["label"]) for e in etiquetas if (e["a"], e["b"], e["rubrica"]) in ultima]
    k = kappa(pares)
    if len(pares) < MIN_PARES:
        return False, f"sombra: {len(pares)} de {MIN_PARES} pares etiquetados (kappa parcial {k})"
    return k >= KAPPA_MIN, f"kappa {k} sobre {len(pares)} pares (minimo {KAPPA_MIN})"


if __name__ == "__main__":
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        a, b = Path(td, "a"), Path(td, "b")
        for d, t in ((a, "texto A"), (b, "texto B")):
            d.mkdir()
            (d / "texto.txt").write_text(t, encoding="utf-8")
        respuestas = iter(['{"mejor": "primero"}', '{"mejor": "segundo"}', '{"mejor": "primero"}', '{"mejor": "primero"}'])
        falso = lambda *a, **k: next(respuestas)  # noqa: E731
        import os
        os.environ.setdefault("MMORCH_HOME", td)
        assert comparar(a, b, "seo_texto", llamar=falso)["mejor"] == "A"   # A gana en los dos ordenes
        assert comparar(a, b, "seo_texto", llamar=falso)["mejor"] == "inconsistente"   # cada orden elige al primero
    print("juez_sombra ok")
