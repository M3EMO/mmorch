"""Tests de propiedades en el SDLC (SDD fase 2, tickets 07 a 10 de .scratch/sdd-fase-2/). Funciones puras; el pipeline
las cablea.

- Marca `R3 [prop]: <regla>` pegada al ID: la regla vale para un conjunto de entradas generable.
- Un test de propiedades es `@given` (Hypothesis) o un test con `_prop_` en el nombre; se llama `test_R<n>_prop_*`.
- Atribucion por test con un solo parser de JUnit XML (pytest --junitxml, node --test, vitest).
- Determinismo: semilla del sha base y un perfil sin base de ejemplos, por un archivo auxiliar ignorado por git.
"""
from __future__ import annotations

import ast
import pathlib
import re
import xml.etree.ElementTree as ET

MARCA = re.compile(r"(?m)^- (R\d+) \[prop\]:")
AUX_PY, AUX_PY_MUT = "_sdlc_prop.py", "_sdlc_prop_mut.py"
AUX_MJS, AUX_MJS_MUT = "_sdlc_prop.mjs", "_sdlc_prop_mut.mjs"
EJEMPLOS_MUTANTES = 20   # ticket 09: valor inicial; el numero se mide en la corrida del ticket 11


def marcas(spec_md: str) -> list[str]:
    """IDs con marca `[prop]` (exacta, pegada al ID). Una mencion dentro de otra regla no marca."""
    return sorted(set(MARCA.findall(spec_md)), key=lambda i: int(i[1:]))


def _given(f: ast.FunctionDef | ast.AsyncFunctionDef) -> bool:
    return any("given" in ast.unparse(d).split("(")[0] for d in f.decorator_list)


def _tests_py(texto: str) -> list[ast.FunctionDef | ast.AsyncFunctionDef]:
    try:
        arbol = ast.parse(texto)
    except SyntaxError:
        return []
    return [n for n in arbol.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef)) and n.name.startswith("test")]


def _es_ts(rel: str) -> bool:
    return not rel.endswith(".py")


def propiedades(rel: str, texto: str) -> list[str]:
    """Nombres de los tests de propiedades de un archivo de test."""
    if _es_ts(rel):
        return sorted(set(re.findall(r"\b(test_\w*_prop_\w*)", texto)))
    return [f.name for f in _tests_py(texto) if _given(f) or "_prop_" in f.name]


def gate_propiedades(rel: str, texto: str) -> tuple[list[str], list[str]]:
    """(errores, IDs con test de propiedades). Una propiedad lleva `@given` y el ID en el nombre."""
    errores, ids = [], set()
    if _es_ts(rel):
        nombres = propiedades(rel, texto)
    else:
        nombres = []
        for f in _tests_py(texto):
            if "_prop_" in f.name and not _given(f):
                errores.append(f"{rel}::{f.name}: test de propiedades sin @given")
            elif _given(f) or "_prop_" in f.name:
                nombres.append(f.name)
    for n in nombres:
        m = re.match(r"test_(R\d+)_", n)
        if m:
            ids.add(m.group(1))
        else:
            errores.append(f"{rel}::{n}: test de propiedades sin ID (test_R<n>_prop_...)")
    return errores, sorted(ids, key=lambda i: int(i[1:]))


def sin_propiedades(rel: str, texto: str) -> str:
    """El archivo sin el codigo de sus propiedades, para los prompts por API (ticket 08: el coder ve la salida del
    fallo, no el oraculo). Python por ast (decoradores incluidos); otro lenguaje por balanceo de parentesis."""
    lineas = texto.splitlines(keepends=True)
    cortes: list[tuple[int, int]] = []
    if not _es_ts(rel):
        for f in _tests_py(texto):
            if _given(f) or "_prop_" in f.name:
                ini = min([f.lineno] + [d.lineno for d in f.decorator_list]) - 1
                cortes.append((ini, f.end_lineno or f.lineno))
    else:
        for i, linea in enumerate(lineas):
            if not re.search(r"\btest_\w*_prop_", linea):
                continue
            prof, abierto, j = 0, False, i
            for j in range(i, len(lineas)):
                for ch in lineas[j]:
                    prof += ch in "({"
                    prof -= ch in ")}"
                    abierto = abierto or ch in "({"
                if abierto and prof <= 0:
                    break
            cortes.append((i, j + 1))
    for ini, fin in sorted(cortes, reverse=True):
        lineas[ini:fin] = [f"# (propiedad oculta: {rel}, lineas {ini + 1}-{fin}; el coder ve solo su salida)\n"]
    return "".join(lineas)


def junit(path) -> dict[str, bool]:
    """{test: paso} de un reporte JUnit XML (pytest, node --test, vitest). Un test parametrizado cuenta por su nombre base."""
    out: dict[str, bool] = {}
    for tc in ET.parse(path).getroot().iter("testcase"):
        nombre = re.sub(r"\[.*\]$", "", tc.get("name") or "")
        ok = tc.find("failure") is None and tc.find("error") is None
        out[nombre] = out.get(nombre, True) and ok
    return out


def con_junit(cmd: str, destino: str) -> str | None:
    """El comando del repo con reporte JUnit en `destino`; None = runner sin JUnit (sin atribucion por test)."""
    if re.search(r"\bpytest\b", cmd):
        return f"{cmd} --junitxml={destino}"
    if re.search(r"\bnode\b.*--test\b", cmd):
        return f"{cmd} --test-reporter=spec --test-reporter-destination=stdout --test-reporter=junit --test-reporter-destination={destino}"
    if re.search(r"\bvitest\b", cmd):
        return f"{cmd} --reporter=default --reporter=junit --outputFile.junit={destino}"
    return None


def con_aux_node(cmd: str, mut: bool) -> str:
    """`node ...` carga el perfil de fast-check con --import antes de los tests."""
    return re.sub(r"(^|\s)node(\s)", rf"\1node --import ./{AUX_MJS_MUT if mut else AUX_MJS}\2", cmd, count=1)


def semilla(sha: str) -> int:
    """Semilla de la corrida: sale del sha del commit base (mismo commit, misma semilla; commits distintos, distinta)."""
    return int(re.sub(r"[^0-9a-f]", "", sha.lower())[:8] or "0", 16)


def aux_py(ejemplos: int | None) -> str:
    extra = f", max_examples={ejemplos}" if ejemplos else ""
    return ('"""Perfil de propiedades del SDLC: auxiliar ignorado por git, lo carga pytest con -p. La semilla llega por\n'
            '--hypothesis-seed (sha base)."""\nimport tempfile\n\ntry:\n    from hypothesis import settings\n'
            "    from hypothesis.configuration import set_hypothesis_home_dir\nexcept ImportError:\n    pass\nelse:\n"
            "    set_hypothesis_home_dir(tempfile.mkdtemp(prefix='sdlc-hyp-'))  # sin .hypothesis/ en el worktree\n"
            "    # sin base de ejemplos ni deadline: una corrida no depende de la anterior ni de la carga de la maquina\n"
            f"    settings.register_profile('sdlc', database=None, deadline=None{extra})\n"
            "    settings.load_profile('sdlc')\n")


def aux_mjs(seed: int, ejemplos: int | None) -> str:
    extra = f", numRuns: {ejemplos}" if ejemplos else ""
    return ("// Perfil de propiedades del SDLC: auxiliar ignorado por git, lo carga node con --import.\n"
            "import fc from 'fast-check';\n\n"
            f"fc.configureGlobal({{ seed: {seed}{extra} }});\n")


def escribir_aux(wt, seed: int, libreria: str) -> list[str]:
    """Escribe los auxiliares de la libreria en el worktree; devuelve sus nombres (van a .git/info/exclude)."""
    wt = pathlib.Path(wt)
    if libreria == "hypothesis":
        (wt / AUX_PY).write_text(aux_py(None), encoding="utf-8")
        (wt / AUX_PY_MUT).write_text(aux_py(EJEMPLOS_MUTANTES), encoding="utf-8")
        return [AUX_PY, AUX_PY_MUT]
    (wt / AUX_MJS).write_text(aux_mjs(seed, None), encoding="utf-8")
    (wt / AUX_MJS_MUT).write_text(aux_mjs(seed, EJEMPLOS_MUTANTES), encoding="utf-8")
    return [AUX_MJS, AUX_MJS_MUT]
