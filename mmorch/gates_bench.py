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
import re
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
        elif k % 4 == 0:
            casos.append({"args": ({"pkg/a.py": "\n".join(a), "pkg/b.py": "\n".join(_modulo(rng))},), "rojo": False,
                          "origen": "generado", "grupo": "independientes"})
        else:  # orchestration-cuz: modulos hermanos, mismo esqueleto (cabecera, imports, firmas) y cuerpos propios
            b = [l if not l.startswith("    ") else "    " + _sentencia(rng).split("\n")[0] for l in a]
            casos.append({"args": ({"pkg/a.py": "\n".join(a), "pkg/b.py": "\n".join(b)},), "rojo": False,
                          "origen": "generado", "grupo": "hermanos"})
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


# --------------------------------------------------------------------------- trazabilidad-plan

_DIRS = ["src/escena", "src/servicio", "pkg", "mmorch", "app/core"]
_EXT = ["py", "ts", "java"]


def _render_traza(rng: random.Random, asignado: dict[str, list[int]], estilo: str) -> str:
    """Seccion '## Archivos' con el formato que pide plan() o una variante que el gate acepta a proposito."""
    def ids(ns: list[int]) -> str:
        if estilo == "rango" and len(ns) > 1:
            return f"R{ns[0]}{rng.choice(['–', '-'])}R{ns[-1]}"
        return ", ".join(f"R{n}" for n in ns)
    if estilo == "tabla":
        filas = [f"| `{f}` | {rng.choice(['nuevo', 'ampliar'])} | {_nombre(rng)} ({ids(ns)}). |" for f, ns in asignado.items()]
        return "\n| Archivo | Accion | Que aporta |\n|---|---|---|\n" + "\n".join(filas) + "\n"
    nombre = (lambda f: f.rsplit("/", 1)[-1]) if estilo == "nombre_suelto" else (lambda f: f)
    return "\n" + "\n".join(f"- `{nombre(f)}` [{ids(ns)}]" + rng.choice(["", " [P]"]) for f, ns in asignado.items()) + "\n"


def _casos_traza_plan(rng: random.Random, n: int) -> list[Caso]:
    """Verde = cada R<n> de la spec esta asignado a un archivo y cada archivo cita un R<n> de la spec. El contrato es el
    pedido de plan(): "Cada item cita los IDs R<n> de la spec que cubre. Todo R<n> de la spec aparece en algun item."
    Variantes verdes = las que el gate acepta a proposito (tabla de SSB F7a, rangos, nombre sin carpeta de F4b).
    Rojo = un R<n> sin archivo, un archivo sin R<n>, un R<n> que solo aparece en prosa fuera de los items, o un
    archivo que solo cita un ID que la spec no tiene."""
    casos: list[Caso] = []
    verdes, rojos = ["lista", "tabla", "rango", "nombre_suelto"], ["id_huerfano", "archivo_sin_id", "id_en_prosa", "id_inexistente"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        n_r, n_f = rng.randint(2, 8), rng.randint(1, 4)
        files = list(dict.fromkeys(f"{rng.choice(_DIRS)}/{_nombre(rng)}.{rng.choice(_EXT)}" for _ in range(n_f)))
        cortes = sorted(rng.sample(range(2, n_r + 1), min(len(files), n_r) - 1))  # cada bloque con >= 1 R
        bloques = [list(range(a, b)) for a, b in zip([1, *cortes], [*cortes, n_r + 1], strict=True)]  # R contiguos por archivo
        asignado = dict(zip(files, bloques, strict=False))  # con mas archivos que R, sobran archivos
        files = list(asignado)
        spec_ids = list(range(1, n_r + 1))
        extra = ""
        if tipo in ("id_huerfano", "id_en_prosa"):
            spec_ids.append(n_r + 1)
            if tipo == "id_en_prosa":
                extra = f"\nNota: R{n_r + 1} queda para una feature futura.\n"
        block = _render_traza(rng, asignado, "lista" if rojo else tipo) + extra
        if tipo in ("archivo_sin_id", "id_inexistente"):
            f = f"{rng.choice(_DIRS)}/{_nombre(rng)}_extra.{rng.choice(_EXT)}"
            files.append(f)
            block += f"- `{f}`" + (" [R99]" if tipo == "id_inexistente" else ": helper comun") + "\n"
        spec = "## Requisitos\n" + "".join(f"- R{i}: {_nombre(rng)}.\n" for i in spec_ids)
        casos.append({"args": (spec, block, files), "rojo": rojo, "origen": "generado", "grupo": tipo})
    return casos


_F7A = """
| Archivo | Acción | Qué aporta |
|---|---|---|
| `src/escena/tipos.ts` | ampliar | `TimelineApp.cameras?` y `ClaveCamaraApp` (R1). |
| `src/escena/construir.ts` | ampliar | `timeline.cameras[id]` desde `camara.claves` (R1). Sin cambios para F1 (`src/escena/png.ts`). |
| `src/servicio/tipos.ts` | nuevo | `EscenaSpec`, `PlanoSpec`, `Look`, `Reporte` (R2). |
| `src/servicio/escena.ts` | nuevo | `construirEscena`, `editarEscena`, `leerEscena`, `verificarPlano` (R2–R10). |
| `tests/` | nuevo | Los tests de la tabla de Trazabilidad. |

No se escribe Python: TypeScript ejecutado por Node 24 sin build.
"""

# --------------------------------------------------------------------------- trazabilidad-spec


def _casos_traza_spec(rng: random.Random, n: int) -> list[Caso]:
    """Contrato = spec-template.md: "Cada R<n> cita al menos un test de aceptacion por su nombre exacto. Un test puede
    cubrir varios IDs. Un ID sin test es un fallo del gate." Sin tests nombrados ni archivos de aceptacion, accept_cmd
    es el oraculo y alcanzan los IDs (verde). Rojo = sin IDs, un ID sin fila, una fila que cita un test que no existe,
    una fila con la celda vacia, o la tabla ausente con tests nombrados."""
    casos: list[Caso] = []
    verdes = ["canonica", "varios_tests", "test_comun", "backticks", "sin_tests_nombrados"]
    rojos = ["sin_ids", "id_sin_fila", "test_inexistente", "celda_vacia", "sin_tabla"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        n_r = rng.randint(1, 8)
        ids = list(range(1, n_r + 1))
        tests = {i: [f"test_R{i}_{_nombre(rng)}"] for i in ids}
        if tipo == "varios_tests":
            for i in ids:
                tests[i].append(f"test_R{i}_{_nombre(rng)}_borde")
        if tipo == "test_comun" and n_r > 1:
            tests[2] = tests[1]
        nombres = {x for ts in tests.values() for x in ts}
        celda = (lambda ts: ", ".join(f"`{x}`" for x in ts)) if tipo == "backticks" else (lambda ts: ", ".join(ts))
        filas = {i: f"| R{i} | {celda(tests[i])} |" for i in ids}
        malo = rng.choice(ids)
        if tipo == "id_sin_fila":
            del filas[malo]
        elif tipo == "test_inexistente":
            filas[malo] = f"| R{malo} | test_R{malo}_{_nombre(rng)}_viejo |"
        elif tipo == "celda_vacia":
            filas[malo] = f"| R{malo} |  |"
        if tipo == "sin_ids":
            req = "".join(f"- {_nombre(rng)}: entrada -> salida.\n" for _ in ids)
        else:
            req = "".join(f"- R{i}: {_nombre(rng)}: entrada -> salida.\n" for i in ids)
        tabla = "" if tipo in ("sin_tabla", "sin_ids") else (
            "## Trazabilidad\n\n| ID | tests |\n|---|---|\n" + "\n".join(filas.values()) + "\n\n")
        spec = f"# Spec\n\n## Requisitos\n\n{req}\n{tabla}" + rng.choice(["", "## Casos borde\n\n- vacio.\n"])
        hay = tipo != "sin_tests_nombrados"
        casos.append({"args": (spec, nombres if hay else set(), hay), "rojo": rojo, "origen": "generado", "grupo": tipo})
    return casos


# --------------------------------------------------------------------------- plan-allowlist


def _casos_plan_allowlist(rng: random.Random, n: int) -> list[Caso]:
    """Contrato = el pedido de plan() y el techo de sdlc.toml: el plan lista los archivos OBLIGATORIOS, no escribe tests
    salvo los obligatorios (un cableo como tests/test_capas.py) y no sale del techo. Rojo = lista un test ajeno, omite
    un obligatorio, lista un archivo fuera del techo, o una ruta que sale del directorio del techo con `..`."""
    casos: list[Caso] = []
    verdes = ["canonica", "cableo_test", "anidado", "sin_techo"]
    rojos = ["test_ajeno", "omite_obligatorio", "fuera_del_techo", "ruta_con_puntos"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        d, ext = rng.choice(["pkg", "src", "app/core"]), rng.choice(_EXT)
        techo = [] if tipo == "sin_techo" else [f"{d}/**/*.{ext}"]
        need = [f"{d}/{_nombre(rng)}.{ext}" for _ in range(rng.randint(1, 2))]
        files = need + [f"{d}/{_nombre(rng)}_extra.{ext}" for _ in range(rng.randint(0, 2))]
        if tipo == "cableo_test":
            need.append("tests/test_capas.py")
            techo.append("tests/test_capas.py")
            files.append("tests/test_capas.py")
        elif tipo == "anidado":
            files.append(f"{d}/{_nombre(rng)}/{_nombre(rng)}.{ext}")
        elif tipo == "sin_techo":
            files.append(f"{_nombre(rng)}/{_nombre(rng)}.{ext}")
        elif tipo == "fuera_del_techo":
            files.append(f"otro/{_nombre(rng)}.{ext}")
        elif tipo == "ruta_con_puntos":
            files.append(f"{d}/../../{_nombre(rng)}.{ext}")
        items = [f"- `{f}` [R1]" for f in files]
        if tipo == "test_ajeno":
            items.append(f"- `tests/test_{_nombre(rng)}.py` [R1]")  # _plan_files lo descarta de files: el gate lo ve en el plan
        if tipo == "omite_obligatorio":
            falta = rng.choice(need)
            items = [i for i in items if f"`{falta}`" not in i]
            files = [f for f in files if f != falta]
        rng.shuffle(items)
        plan = "# Plan\n\n## Archivos\n\n" + "\n".join(items) + "\n\n## Prueba\n\n`python -m pytest -q`\n"
        casos.append({"args": (plan, files, need, techo, "tests/"), "rojo": rojo, "origen": "generado", "grupo": tipo})
    return casos


# --------------------------------------------------------------------------- cambio-minimo


def _archivo(rng: random.Random, n_mod: int) -> list[str]:
    return [l for _ in range(n_mod) for l in _modulo(rng)]


def _casos_cambio_minimo(rng: random.Random, n: int) -> list[Caso]:
    """Intencion = D11: el coder reescribe o recorta un archivo existente cuando la tarea pedia un cambio chico. Verde =
    un parche (hasta 10% de lineas editadas mas unas lineas nuevas), un archivo nuevo, o una tarea que pide reescribir.
    Rojo = una reescritura no pedida (por tamaño del original), un recorte de 40-70% de las lineas, o una reescritura
    cuando la tarea dice explicitamente que no se reescriba."""
    casos: list[Caso] = []
    verdes = ["parche", "parche_archivo_chico", "archivo_nuevo", "tarea_pide_reescribir"]
    rojos = ["reescritura", "recorte", "tarea_prohibe_reescribir"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        orig = _archivo(rng, 1) if tipo == "parche_archivo_chico" else _archivo(rng, rng.randint(1, 8))
        tarea = "Agrega una guarda contra montos negativos en pkg/a.py."
        grupo = tipo
        if tipo in ("parche", "parche_archivo_chico"):
            nuevo = _copia_editada(rng, orig)[0] if tipo == "parche_archivo_chico" else list(orig)  # chico: hasta 30%
            idx = [] if tipo == "parche_archivo_chico" else rng.sample(range(len(nuevo)), int(rng.uniform(0, 0.10) * len(nuevo)))
            for i in idx:
                nuevo[i] = nuevo[i][:len(nuevo[i]) - len(nuevo[i].lstrip())] + _sentencia(rng).split("\n")[0]
            nuevo += ["    if n < 0:", "        raise ValueError(n)"]
        elif tipo == "recorte":
            keep = sorted(rng.sample(range(len(orig)), int(len(orig) * rng.uniform(0.30, 0.60))))
            nuevo = [orig[i] for i in keep] + ["    if n < 0:", "        raise ValueError(n)"]
        else:
            nuevo = _archivo(rng, max(1, len(orig) // 25))
            if tipo == "tarea_pide_reescribir":
                tarea = rng.choice(["Reescribi pkg/a.py con una sola funcion publica.",
                                    "Hay que reescribir pkg/a.py: el modulo viejo no se entiende.",
                                    "Refactor: reescribí el parser de pkg/a.py desde cero."])
            elif tipo == "tarea_prohibe_reescribir":
                tarea = rng.choice(["Agrega la guarda sin reescribir pkg/a.py.", "Agrega la guarda; no reescribas pkg/a.py.",
                                    "Agrega la guarda y nunca reescribas el archivo entero.",
                                    "Agrega la guarda. Conserva pkg/a.py tal cual, solo suma lineas."])
            else:
                grupo = f"reescritura {'<40' if len(orig) < 40 else '>=40'} lineas"
        o = {} if tipo == "archivo_nuevo" else {"pkg/a.py": "\n".join(orig)}
        casos.append({"args": (o, {"pkg/a.py": "\n".join(nuevo)}, tarea), "rojo": rojo, "origen": "generado", "grupo": grupo})
    return casos


# --------------------------------------------------------------------------- docstring-intacto

_DOC = ["Banco de mmorch: mide verificadores sobre tareas con verdad computada.", "Registro de plugins del engine.",
        "Ruteo multi-modelo.\n\nDecide que modelo corre cada nodo y registra su costo."]


def _casos_docstring(rng: random.Random, n: int) -> list[Caso]:
    """Intencion = D4: un docstring de modulo que existia desaparece sin que la tarea lo pida. Verde = intacto, editado,
    sin docstring antes ni despues, o una tarea que pide cambiar los docstrings. Rojo = borrado, reemplazado por un
    marcador vacio de contenido (`TODO`, `.`), o borrado cuando la tarea dice que no se toquen."""
    casos: list[Caso] = []
    verdes, rojos = ["intacto", "editado", "sin_docstring", "tarea_docstrings"], ["borrado", "marcador", "tarea_prohibe"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        fs = [f"pkg/{_nombre(rng)}.py" for _ in range(rng.randint(1, 3))]
        antes: dict[str, str | None] = {f: (None if tipo == "sin_docstring" else rng.choice(_DOC)) for f in fs}
        despues = dict(antes)
        malo = rng.choice(fs)
        tarea = "Agrega el parametro timeout a la funcion run."
        if tipo == "editado":
            despues[malo] = f"{antes[malo]} Agrega timeout."
        elif tipo in ("borrado", "tarea_docstrings", "tarea_prohibe"):
            despues[malo] = None
            tarea = {"tarea_docstrings": rng.choice(["Actualiza los docstrings de pkg/.", "Borra el docstring viejo de run.",
                                                     "Los docstrings estan desactualizados: reescribilos."]),
                     "tarea_prohibe": rng.choice(["Agrega timeout; no toques ningun docstring.",
                                                  "Agrega timeout sin cambiar los docstrings.",
                                                  "Agrega timeout. Deja los docstrings como estan.",
                                                  "Agrega timeout; los docstrings quedan igual."])}.get(tipo, tarea)
        elif tipo == "marcador":
            despues[malo] = rng.choice(["TODO", ".", "..."])
        casos.append({"args": (antes, despues, tarea), "rojo": rojo, "origen": "generado", "grupo": tipo})
    return casos


# --------------------------------------------------------------------------- un-archivo

_FILE_RE = r"((?:[\w.-]+/)*[\w.-]+\.(?:py|ts|java))"  # el de sdlc/pipeline._file_re con ext = [py, ts, java]


def _un_archivo_en_pipeline(code: str, rel: str, file_re: str, esperado: str | None) -> tuple[bool, str]:
    """El gate como lo usa `one_file`: veredicto mas recorte. Pasa solo si lo que se escribiria es el archivo pedido."""
    ok, detalle = gates.un_archivo(code, rel, file_re)
    if not ok:
        return False, detalle
    escrito = gates.secciones(code, file_re).get(rel, code)
    return escrito.strip() == (esperado or "").strip(), detalle


def _casos_un_archivo(rng: random.Random, n: int) -> list[Caso]:
    """Intencion = r1 2026-09-11: el coder pega varios archivos en una respuesta. Positivo = lo correcto es rechazar:
    ninguna seccion es la del archivo pedido. Verde = el pipeline debe escribir el archivo pedido: un solo archivo, o
    varias secciones que lo incluyen con cualquier cabecera plausible (`# ruta`, `// ruta`, `# File: ruta`, la ruta
    entre backticks). Se mide el resultado de `one_file` (veredicto mas recorte), no solo el veredicto: antes de
    orchestration-6qb el gate pasaba `# File: ruta` y el pipeline escribia todo el texto en el archivo pedido."""
    casos: list[Caso] = []
    verdes = ["un_solo_archivo", "con_el_pedido", "cabecera_con_prefijo", "cabecera_en_backticks"]
    rojos = ["sin_el_pedido", "sin_el_pedido_con_prefijo"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        rel = f"pkg/{_nombre(rng)}.py"
        otros = [f"pkg/{_nombre(rng)}_otro.py" for _ in range(rng.randint(1, 2))]
        cuerpos = {r: "\n".join(_modulo(rng)) for r in [rel, *otros]}
        if tipo == "un_solo_archivo":
            code = cuerpos[rel]
        else:
            rutas = otros if rojo else rng.sample([rel, *otros], len(otros) + 1)
            cab = {"cabecera_con_prefijo": "# File: {}", "sin_el_pedido_con_prefijo": "# File: {}",
                   "cabecera_en_backticks": "# `{}`"}.get(tipo, rng.choice(["# {}", "// {}"]))
            code = "\n\n".join(cab.format(r) + "\n" + cuerpos[r] for r in rutas)
        casos.append({"args": (code, rel, _FILE_RE, None if rojo else cuerpos[rel]), "rojo": rojo, "origen": "generado",
                      "grupo": tipo})
    return casos


# --------------------------------------------------------------------------- tipo-del-archivo


def _casos_tipo_propio(rng: random.Random, n: int) -> list[Caso]:
    """Intencion = orchestration-7b6: en Java el tipo publico de primer nivel lleva el nombre del archivo; el coder
    devolvio otra clase. Verde = el tipo propio (con modificadores o con tipos anidados publicos) o un archivo que no es
    Java. Rojo = un tipo publico ajeno (clase, interfaz, con modificadores) o con la anotacion en la misma linea."""
    casos: list[Caso] = []
    verdes, rojos = ["tipo_propio", "con_anidado", "no_java"], ["tipo_ajeno", "interfaz_ajena", "anotacion_en_linea"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        propio = _nombre(rng).replace("_", "").capitalize()
        ajeno = propio + rng.choice(["Service", "Negocio", "Item"])
        mod = rng.choice(["", "final ", "abstract "])
        rel = f"src/main/{propio}.java"
        cuerpo = "{\n    private int n;\n}\n"
        code = {"tipo_propio": f"public {mod}class {propio} {cuerpo}",
                "con_anidado": f"public class {propio} {{\n    public static class {ajeno} {{}}\n}}\n",
                "no_java": f"export class {ajeno} {cuerpo}",
                "tipo_ajeno": f"public {mod}class {ajeno} {cuerpo}",
                "interfaz_ajena": f"public interface {ajeno} {cuerpo}",
                "anotacion_en_linea": f"@Deprecated public class {ajeno} {cuerpo}"}[tipo]
        if tipo == "no_java":
            rel = f"src/{propio}.ts"
        code = "package app;\n\nimport java.util.List;\n\n" + code if rel.endswith(".java") else code
        casos.append({"args": (code, rel), "rojo": rojo, "origen": "generado", "grupo": tipo})
    return casos


# --------------------------------------------------------------------------- baseline-intacto


def _casos_baseline(rng: random.Random, n: int) -> list[Caso]:
    """Intencion: los tests de aceptacion quedan congelados durante el build. Rojo = el coder debilita un assert,
    borra el archivo, le agrega un test o solo le cambia espacios (el archivo ya no es el aprobado). Verde = intacto,
    reescrito con los mismos bytes, o un cambio en un archivo que no es de aceptacion. El hash se calcula aca, igual
    que `sdlc/pipeline.sha_file`."""
    import hashlib
    h = lambda s: hashlib.sha256(s.encode("utf-8")).hexdigest()  # noqa: E731
    casos: list[Caso] = []
    verdes, rojos = ["intacto", "reescrito_igual", "otro_archivo"], ["assert_debilitado", "borrado", "test_agregado", "espacios"]
    for k in range(n):
        rojo = bool(k % 2)
        tipo = rng.choice(rojos if rojo else verdes)
        tests = {f"tests/test_sdlc_{_nombre(rng)}.py":
                 "".join(f"def test_R{i}_{_nombre(rng)}():\n    assert {_nombre(rng)}({i}) == {rng.randint(0, 99)}\n\n"
                         for i in range(1, rng.randint(2, 5))) for _ in range(rng.randint(1, 2))}
        ahora = dict(tests)
        rel = rng.choice(list(tests))
        if tipo == "assert_debilitado":
            ahora[rel] = re.sub(r"== \d+", "is not None", ahora[rel], count=1)
        elif tipo == "test_agregado":
            ahora[rel] += "def test_extra():\n    assert True\n"
        elif tipo == "espacios":
            ahora[rel] = ahora[rel].replace("    assert", "        assert", 1)
        hashes: dict[str, str | None] = {r: h(c) for r, c in ahora.items()}
        if tipo == "borrado":
            hashes[rel] = None
        if tipo == "otro_archivo":
            hashes["pkg/a.py"] = h("x = 2\n")
        casos.append({"args": ({r: h(c) for r, c in tests.items()}, hashes), "rojo": rojo, "origen": "generado",
                      "grupo": tipo})
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
    "trazabilidad-plan": [
        # SSB F7a (2026-09-28): plan en tabla con un rango R2–R10; el gate de entonces decia "plan omite" los 4 archivos.
        {"args": ("## Requisitos\n" + " ".join(f"R{i}" for i in range(1, 11)) + "\n", _F7A,
                  ["src/escena/tipos.ts", "src/escena/construir.ts", "src/servicio/tipos.ts", "src/servicio/escena.ts"]),
         "rojo": False, "origen": "SSB F7a", "grupo": None, "reconstruido": True},
        # SSB F4b (2026-09-30): la nota de un archivo lo nombra sin carpeta.
        {"args": ("## Requisitos\nR1 R2 R3\n", "\n- `src/encuadre/resolver.ts` [R1, R2, R3]\n\nNotas por archivo:\n\n"
                  "- `resolver.ts`: único módulo del feature. Importa `Vec3` de `../escena/tipos.ts`.\n",
                  ["src/encuadre/resolver.ts"]),
         "rojo": False, "origen": "SSB F4b", "grupo": None, "reconstruido": True},
    ],
    "cambio-minimo": [
        # D11: el coder reescribio project_loop.py (-216 lineas) para agregar una guarda. El texto original no se guardo.
        {"args": ({"mmorch/project_loop.py": "\n".join(f"linea_{i} = {i}" for i in range(300))},
                  {"mmorch/project_loop.py": "\n".join(f"linea_{i} = {i}" for i in range(84)) + "\nif guarda:\n    pass"},
                  "Agrega una guarda en project_loop.py."),
         "rojo": True, "origen": "D11", "grupo": None, "reconstruido": True},
    ],
    "docstring-intacto": [
        # D4: el coder borro el docstring de modulo (22 lineas). Solo sobrevive la descripcion.
        {"args": ({"mmorch/x.py": "\n".join(f"linea {i} del docstring" for i in range(22))}, {"mmorch/x.py": None},
                  "Agrega una opcion."),
         "rojo": True, "origen": "D4", "grupo": None, "reconstruido": True},
    ],
    "un-archivo": [
        # r1 2026-09-11: el coder imito las cabeceras "# path" del prompt y pego 3 archivos en uno.
        {"args": ("# limiter/__init__.py\nfrom x import y\n\n# limiter/core.py\nclass TokenBucket: pass\n\n"
                  "# limiter/util.py\nZ = 1\n", "limiter/core.py", _FILE_RE, "class TokenBucket: pass"),
         "rojo": False, "origen": "r1 2026-09-11", "grupo": None, "reconstruido": True},
    ],
    "tipo-del-archivo": [
        # orchestration-7b6: pedido Opcion.java, el coder devolvio la clase Negocio; pedido Bot.java, devolvio Opcion.
        {"args": ("public class Negocio {\n}\n", "src/Opcion.java"), "rojo": True, "origen": "orchestration-7b6",
         "grupo": None, "reconstruido": True},
        {"args": ("public class Opcion {\n}\n", "src/Bot.java"), "rojo": True, "origen": "orchestration-7b6",
         "grupo": None, "reconstruido": True},
    ],
}

CHEQUEOS: dict[str, Callable[..., tuple[bool, str]]] = {"sin-clones": gates.sin_clones,
                                                         "spec-review": gates.clarificaciones,
                                                         "trazabilidad-plan": gates.traza_plan,
                                                         "trazabilidad-spec": gates.traza_spec,
                                                         "plan-allowlist": gates.plan_allowlist,
                                                         "cambio-minimo": gates.cambio_minimo,
                                                         "docstring-intacto": gates.docstring_intacto,
                                                         "un-archivo": _un_archivo_en_pipeline,
                                                         "tipo-del-archivo": gates.tipo_propio,
                                                         "baseline-intacto": gates.baseline_intacto}
GENERADORES: dict[str, Callable[[random.Random, int], list[Caso]]] = {"sin-clones": _casos_sin_clones,
                                                                    "spec-review": _casos_spec_review,
                                                                    "trazabilidad-plan": _casos_traza_plan,
                                                                    "trazabilidad-spec": _casos_traza_spec,
                                                                    "plan-allowlist": _casos_plan_allowlist,
                                                                    "cambio-minimo": _casos_cambio_minimo,
                                                                    "docstring-intacto": _casos_docstring,
                                                                    "un-archivo": _casos_un_archivo,
                                                                    "tipo-del-archivo": _casos_tipo_propio,
                                                                    "baseline-intacto": _casos_baseline}


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
