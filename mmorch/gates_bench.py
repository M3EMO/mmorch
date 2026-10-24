"""Banco de los gates del SDLC (bd orchestration-bpn, 2026-10-23): precision de cada chequeo de mmorch/gates.py.

La etiqueta de un caso sale de la INTENCION con que se construye, nunca de la regla del gate ni de un LLM. Si el
generador etiquetara con la regla del gate, la accuracy saldria 1.0 por definicion. Casos = incidentes reales (lista
literal; `reconstruido` marca los que solo sobreviven como un test rehecho) + un generador con semilla por gate. Misma
semilla = mismos casos. Positivo = rojo (el gate debe rechazar). Metricas: las de la seccion "Measured" del README.
Un caso `aparte` no entra a la metrica principal: su acierto se publica solo por grupo (D12).

Uso: python -m mmorch.gates_bench --seed 42 [--n 400] [--gate sin-clones] [--dry]
Cada corrida agrega una fila por gate a logs/ablation_results.jsonl. Decisiones: brainstorms/2026-10-23-gates-a-mmorch.md.
"""
from __future__ import annotations

import argparse
import json
import random
import time
from typing import Any, Callable

from mmorch import gates
from mmorch.paths import logs_dir

Caso = dict[str, Any]  # {"args": tuple, "rojo": bool, "origen": str, "grupo": str | None, "aparte"?: bool}

# --------------------------------------------------------------------------- sin-clones

_IMPORTS = ["import os", "import re", "import json", "import sys", "import time", "import subprocess",
            "from pathlib import Path", "from typing import Any", "from dataclasses import dataclass"]
_VARS = ["a", "b", "x", "y", "n", "res", "total", "datos"]
_SILABAS = ["ca", "lo", "mi", "ter", "pu", "ran", "so", "ve", "dor", "ki", "ble", "fe"]
_MAX_EDICION = 0.30  # decision del usuario 2026-10-23: una copia con hasta 30% de lineas editadas sigue siendo un clon


def _nombre(rng: random.Random) -> str:
    return "_".join("".join(rng.choices(_SILABAS, k=rng.randint(2, 3))) for _ in range(rng.randint(1, 2)))


def _sentencia(rng: random.Random) -> str:
    v, u = rng.choice(_VARS), rng.choice(_VARS)
    return rng.choice([f"{v} = {u} {rng.choice('+-*')} {rng.randint(0, 99)}",
                       f"if {u} > {rng.randint(0, 99)}:\n        {v} = {_nombre(rng)}({u})",
                       f"{v} = {_nombre(rng)}({u}, {rng.randint(0, 9)})"])


def _modulo(rng: random.Random) -> list[str]:
    """Un modulo Python plausible: cabecera e imports comunes (el boilerplate que dos modulos sanos comparten)."""
    lines = [f'"""Modulo {_nombre(rng)}."""', "from __future__ import annotations",
             *rng.sample(_IMPORTS, rng.randint(2, 6))]
    for _ in range(rng.randint(2, 6)):
        lines.append(f"def {_nombre(rng)}({', '.join(rng.sample(_VARS, 2))}):")
        lines += ["    " + _sentencia(rng) for _ in range(rng.randint(2, 6))]
        lines.append(f"    return {rng.choice(_VARS)}")
    return "\n".join(lines).split("\n")


def _copia_editada(rng: random.Random, orig: list[str]) -> tuple[list[str], float]:
    """Copia de `orig` con una fraccion de lineas reescritas en [0, _MAX_EDICION]. La intencion es copiar."""
    frac = rng.uniform(0, _MAX_EDICION)
    idx = rng.sample(range(len(orig)), int(frac * len(orig)))  # int: nunca supera _MAX_EDICION
    out = list(orig)
    for i in idx:
        sangria = out[i][:len(out[i]) - len(out[i].lstrip())]
        out[i] = sangria + _sentencia(rng).split("\n")[0]
    return out, len(idx) / len(orig)


def _casos_sin_clones(rng: random.Random, n: int) -> list[Caso]:
    casos: list[Caso] = []
    for k in range(n):
        a = _modulo(rng)
        if k % 2:
            b, frac = _copia_editada(rng, a)
            tramo = f"{int(frac * 10) * 10}-{int(frac * 10) * 10 + 10}%"
            casos.append({"args": ({"pkg/a.py": "\n".join(a), "pkg/b.py": "\n".join(b)},), "rojo": True,
                          "origen": "generado", "grupo": f"copia {tramo}"})
        else:
            casos.append({"args": ({"pkg/a.py": "\n".join(a), "pkg/b.py": "\n".join(_modulo(rng))},), "rojo": False,
                          "origen": "generado", "grupo": "independientes"})
    return casos


# --------------------------------------------------------------------------- spec-review

_PREG = ["cap de la tasa?", "que pasa con un monto negativo?", "el redondeo es a 2 decimales?",
         "la fecha usa la zona del servidor?", "el archivo vacio es un error?", "se aceptan duplicados?"]
_RESP = ["si, lo dice la TAREA", "SUPUESTO: float sin cap", "no, el test R2 lo rechaza", "SUPUESTO: UTC"]
_AFECTA = ["R1", "R2", "R1, R3", "Contrato", "R4"]
_CERO = ["- sin preguntas: la spec cubre el barrido.", "Hice 0 preguntas: la spec cubre todo.",
         "No hay preguntas abiertas.", "Ninguna pregunta: los tests responden todo."]
_SPEC = "# Spec\n\n## Requisitos\n\n- R1: suma montos.\n- R2: rechaza negativos.\n\n"
_COLA = ["", "\n## Trazabilidad\n\n| R1 | test_R1_suma |\n"]
# decision del usuario 2026-10-24 (Q10): el template pide formato exacto; estas variantes se publican aparte, como
# costo de reintento, y no entran a la metrica principal
_VARIANTES = ("espacios", "negrita", "numerada")


def _pregunta(rng: random.Random, i: int, estilo: str, resp: str | None = None) -> str:
    q, r, a = rng.choice(_PREG), rng.choice(_RESP) if resp is None else resp, rng.choice(_AFECTA)
    return {"canonica": f"- P{i}: {q} | R: {r} | afecta: {a}",
            "espacios": f"- P{i}:  {q}  |  R: {r}  |  afecta: {a}",
            "negrita": f"- **P{i}:** {q} | **R:** {r} | afecta: {a}",
            "numerada": f"{i}. P{i}: {q} | R: {r} | afecta: {a}",
            "sin_afecta": f"- P{i}: {q} | R: {r}"}[estilo]


def _casos_spec_review(rng: random.Random, n: int) -> list[Caso]:
    """Verde = la revision se hizo: 1..5 preguntas respondidas, o 0 preguntas declaradas. Variantes plausibles de
    escritura (`_VARIANTES`) son verdes pero van aparte. Rojo = la revision no se hizo:
    sin seccion, seccion vacia, una pregunta sin respuesta o con TBD, o mas de 5 preguntas (el template pide <= 5)."""
    casos: list[Caso] = []
    verdes = ["canonica", *_VARIANTES, "cero"]
    rojos = ["sin_seccion", "seccion_vacia", "respuesta_vacia", "respuesta_tbd", "seis_preguntas"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        m = rng.randint(1, 5)
        if tipo == "cero":
            cuerpo = rng.choice(_CERO)
        elif tipo in ("canonica", "espacios", "negrita", "numerada"):
            cuerpo = "\n".join(_pregunta(rng, i, tipo) for i in range(1, m + 1))
        elif tipo in ("respuesta_vacia", "respuesta_tbd"):
            mala = rng.randint(1, m)
            cuerpo = "\n".join(_pregunta(rng, i, "canonica", ("" if tipo == "respuesta_vacia" else "TBD") if i == mala else None)
                                for i in range(1, m + 1))
        elif tipo == "seis_preguntas":
            cuerpo = "\n".join(_pregunta(rng, i, "canonica") for i in range(1, 7))
        else:
            cuerpo = ""
        seccion = "" if tipo == "sin_seccion" else f"## Clarificaciones\n\n{cuerpo}\n"
        casos.append({"args": (_SPEC + seccion + rng.choice(_COLA),), "rojo": rojo, "origen": "generado", "grupo": tipo,
                      "aparte": tipo in _VARIANTES})
    return casos


_PLUGINS = '''"""Registro de plugins."""
from __future__ import annotations
import importlib
_REGISTRO: dict[str, object] = {}
def registrar(nombre, fn):
    _REGISTRO[nombre] = fn
def cargar(modulo):
    m = importlib.import_module(modulo)
    for nombre, fn in getattr(m, "PLUGINS", {}).items():
        registrar(nombre, fn)
def lista():
    return sorted(_REGISTRO)
'''

# --------------------------------------------------------------------------- registro

INCIDENTES: dict[str, list[Caso]] = {
    "sin-clones": [
        # D6: el coder escribio el contenido de plugins.py dentro de plugin_worker.py. El texto original no se guardo.
        {"args": ({"mmorch/plugins.py": _PLUGINS,
                   "mmorch/plugin_worker.py": '"""Worker de plugins."""\n' + _PLUGINS.split("\n", 1)[1]},),
         "rojo": True, "origen": "D6", "grupo": None, "reconstruido": True},
    ],
    "spec-review": [
        # D11-diag (2026-09-15): Claude escribio "0 preguntas" y "afecta: Contrato"; el gate de entonces los rechazaba.
        {"args": ("## Clarificaciones\n\nHice 0 preguntas: la spec cubre todo.\n",), "rojo": False, "origen": "D11-diag",
         "grupo": None, "reconstruido": True},
        {"args": ("## Clarificaciones\n- P1: ¿x? | R: si | afecta: Contrato\n",), "rojo": False, "origen": "D11-diag",
         "grupo": None, "reconstruido": True},
    ],
}

CHEQUEOS: dict[str, Callable[..., tuple[bool, str]]] = {"sin-clones": gates.sin_clones,
                                                         "spec-review": gates.clarificaciones}
GENERADORES: dict[str, Callable[[random.Random, int], list[Caso]]] = {"sin-clones": _casos_sin_clones,
                                                                    "spec-review": _casos_spec_review}


def medir(gate: str, n: int = 400, seed: int = 42) -> dict[str, Any]:
    casos = INCIDENTES.get(gate, []) + GENERADORES[gate](random.Random(seed), n)
    tp = fn = tn = fp = 0
    grupos: dict[str, list[int]] = {}
    for c in casos:
        rechaza = not CHEQUEOS[gate](*c["args"])[0]
        if c.get("aparte"):
            pass
        elif c["rojo"]:
            tp, fn = tp + rechaza, fn + (not rechaza)
        else:
            fp, tn = fp + rechaza, tn + (not rechaza)
        if c["grupo"]:
            grupos.setdefault(c["grupo"], []).append(int(rechaza == c["rojo"]))
    sens = tp / max(tp + fn, 1)
    spec = tn / max(tn + fp, 1)
    aparte = sorted({c["grupo"] for c in casos if c.get("aparte")})
    return {"experiment": "gates_bench", "gate": gate, "seed": seed, "n": tp + fn + tn + fp, "aparte": aparte,
            "incidentes": len(INCIDENTES.get(gate, [])), "tp": tp, "fn": fn, "tn": tn, "fp": fp,
            "sensibilidad": round(sens, 3), "especificidad": round(spec, 3),
            "acc_balanceada": round((sens + spec) / 2, 3), "falso_rechazo": round(1 - spec, 3),
            "acierto_por_grupo": {g: round(sum(v) / len(v), 3) for g, v in sorted(grupos.items())}}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--gate", choices=sorted(CHEQUEOS))
    ap.add_argument("--dry", action="store_true", help="no agrega filas a logs/ablation_results.jsonl")
    a = ap.parse_args(argv)
    out = logs_dir() / "ablation_results.jsonl"
    for g in [a.gate] if a.gate else sorted(CHEQUEOS):
        fila = medir(g, a.n, a.seed) | {"ts": time.time(), "fecha": time.strftime("%Y-%m-%d %H:%M")}
        print(json.dumps(fila, ensure_ascii=False))
        if not a.dry:
            out.parent.mkdir(parents=True, exist_ok=True)
            with open(out, "a", encoding="utf-8") as fh:
                fh.write(json.dumps(fila, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
