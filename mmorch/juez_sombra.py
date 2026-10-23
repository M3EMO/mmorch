"""juez_sombra — opina sobre el residuo NO computable de un entregable, de a pares y sin bloquear (builder-e2e ticket 09).

Generaliza el juez visual de sprites.py: compara dos entregables (A y B) con una rubrica con anclas y criterios
binarios, sin puntajes numericos (medido 2026-09-17: un juez VLM ordena pero puntua mal). Cada par se juzga dos
veces con el orden invertido; si los dos ordenes no coinciden, la opinion es "inconsistente" (sesgo de posicion).
Todo va a logs/sdlc/juez_sombra.jsonl. Sale de sombra solo con kappa >= KAPPA_MIN contra pares etiquetados por el
usuario (`confiable`). Texto: DEFAULT_VERIFIER (razona). Imagen: Sonnet 5.5 por la CLI de Claude, con cupo
(decision del usuario 2026-10-17: GLM sin saldo).
"""
from __future__ import annotations

import base64
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path

from .config import DEFAULT_VERIFIER
from .paths import logs_dir
from sdlc.sprites import kappa

VLM = "claude:claude-sonnet-5-5"   # un modelo `claude:` corre por la CLI y gasta cupo, nunca por API

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


def _claude(modelo: str, mensaje: list[dict], **_) -> dict:
    """La CLI headless y aislada (sin herramientas, hooks ni MCP). Las imagenes van como bloques por stream-json."""
    from .claude_exec import claude_bin
    bloques = []
    for parte in mensaje[0]["content"] if isinstance(mensaje[0]["content"], list) else [{"type": "text", "text": mensaje[0]["content"]}]:
        if parte["type"] == "text":
            bloques.append(parte)
        else:   # data:image/png;base64,<datos> (formato OpenAI de _imagen)
            tipo, datos = parte["image_url"]["url"][5:].split(";base64,")
            bloques.append({"type": "image", "source": {"type": "base64", "media_type": tipo, "data": datos}})
    argv = claude_bin() + ["-p", "--model", modelo.split(":", 1)[1], "--restricted", "--strict-mcp-config", "--tools", "",
                           "--no-session-persistence", "--input-format", "stream-json", "--output-format", "stream-json",
                           "--verbose"]
    entrada = json.dumps({"type": "user", "message": {"role": "user", "content": bloques}})
    with tempfile.TemporaryDirectory() as cwd:
        r = subprocess.run(argv, input=entrada, capture_output=True, text=True, timeout=300, cwd=cwd, env=os.environ.copy())
    fin = next((d for d in map(json.loads, filter(str.strip, reversed(r.stdout.splitlines()))) if d.get("type") == "result"), {})
    if fin.get("is_error") or not fin.get("result"):
        raise RuntimeError(f"juez claude: {str(fin.get('result') or r.stderr)[:200]}")
    return {"text": fin["result"], "cupo_usd": float(fin.get("total_cost_usd") or 0.0)}


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
    if llamar is None and modelo.startswith("claude:"):
        llamar = _claude
    if llamar is None:
        from .providers import call as llamar
    res = llamar(modelo, mensaje, pattern="juez-sombra", node=modelo, phase=f"juez-{rubrica}", temperature=0.0,
                 max_tokens=16384, timeout=300)   # 4096: el reasoner agotaba el presupuesto razonando
    crudo = res["text"] if isinstance(res, dict) else res.text if hasattr(res, "text") else str(res)
    cupo = res.get("cupo_usd", 0.0) if isinstance(res, dict) else 0.0
    try:
        return json.loads(crudo[crudo.index("{"):crudo.rindex("}") + 1]) | {"_cupo": cupo}
    except ValueError:
        return {"mejor": "", "motivo": f"respuesta no parseable: {crudo[:200]}", "_cupo": cupo}


def comparar(a: Path, b: Path, rubrica: str, *, modelo: str | None = None, llamar=None) -> dict:
    """Opinion de a pares sobre las capturas del driver (carpetas con texto.txt y PNGs). NUNCA bloquea."""
    modelo = modelo or (VLM if rubrica == "visual" else DEFAULT_VERIFIER)
    try:
        r_ab, r_ba = _una_vez(a, b, rubrica, modelo, llamar), _una_vez(b, a, rubrica, modelo, llamar)
    except Exception as e:  # noqa: BLE001 — la sombra nunca puede tumbar una corrida
        r_ab, r_ba, error = {}, {}, str(e)[:200]
    else:
        error = ""
    ab, ba = r_ab.get("mejor", ""), r_ba.get("mejor", "")
    directo = {"primero": "A", "segundo": "B"}.get(ab, ab)
    invertido = {"primero": "B", "segundo": "A"}.get(ba, ba)
    mejor = directo if directo == invertido else "inconsistente"
    d = {"ts": time.time(), "rubrica": rubrica, "a": str(a), "b": str(b), "modelo": modelo, "mejor": mejor,
         "orden_ab": directo, "orden_ba": invertido, "motivo_ab": r_ab.get("motivo", ""), "motivo_ba": r_ba.get("motivo", ""),
         "cupo_usd": round(r_ab.get("_cupo", 0.0) + r_ba.get("_cupo", 0.0), 4), "error": error}
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
        respuestas = iter(['{"mejor": "primero", "motivo": "A explica los aportes"}', '{"mejor": "segundo"}', '{"mejor": "primero"}', '{"mejor": "primero"}'])
        falso = lambda *a, **k: next(respuestas)  # noqa: E731
        import os
        os.environ.setdefault("MMORCH_HOME", td)
        op = comparar(a, b, "seo_texto", llamar=falso)
        assert op["mejor"] == "A" and op["motivo_ab"] == "A explica los aportes"   # A gana en los dos ordenes
        assert comparar(a, b, "seo_texto", llamar=falso)["mejor"] == "inconsistente"   # cada orden elige al primero
    print("juez_sombra ok")
