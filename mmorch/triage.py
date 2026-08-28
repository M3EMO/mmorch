"""Triage mecanico de branches propuestas — cero LLM, cero cupo, determinista.

Motivo (2026-08-25): el gate de ejecucion acepta todo lo que no rompe tests, y
eso es un filtro debil. De 5 branches que abrio evolve esa noche, las 5 pasaron
la suite y 3 eran para descartar. El tren las empaqueto a todas y un click
humano casi mete las 3 adentro.

Este modulo es el escalon que faltaba ENTRE "compila" y "el humano decide": las
preguntas que se contestan mirando el diff, sin opinion y sin gastar una
llamada. Lo que sobrevive va al refutador cross-family y recien despues al
humano.

Que NO hace a proposito: juzgar si el cambio es una buena idea. Eso necesita
entender la intencion (el flag `isolate` que no aisla nada solo se cae leyendo
que hace la funcion). Para eso esta el refutador; esto saca la basura obvia
primero para que el refutador no la gaste.
"""

from __future__ import annotations

import ast
import io
import tokenize
from collections import Counter

VEREDICTOS = ("descartar", "revisar", "ok")


def _sin_cambio_semantico(antes: str, despues: str) -> bool:
    """El AST es identico: el diff no puede cambiar el comportamiento.

    Los comentarios NO estan en el AST, asi que un patch que solo toca
    comentarios/espacios cae aca. Los docstrings SI son nodos, y cambiarlos
    cuenta como cambio."""
    try:
        return ast.dump(ast.parse(antes)) == ast.dump(ast.parse(despues))
    except SyntaxError:
        return False           # sin poder parsear no se afirma nada


def _comentarios(src: str) -> Counter:
    """Multiset del TEXTO de cada comentario (tokenize, no regex: un '#'
    adentro de un string no es un comentario). Por texto y no por conteo:
    mover un comentario de lugar no cuenta como borrarlo."""
    try:
        return Counter(t.string.strip()
                       for t in tokenize.generate_tokens(io.StringIO(src).readline)
                       if t.type == tokenize.COMMENT)
    except (tokenize.TokenError, SyntaxError, IndentationError):
        return Counter()


def triage_archivo(antes: str, despues: str) -> tuple[str, str]:
    """(veredicto, motivo) para UN archivo. Ver triage_branch para el agregado.

    Lo que se cuenta son los comentarios BORRADOS, no el neto: el caso medido
    (health.py, 2026-08-25) sacaba 4 y ponia 5, asi que el neto daba +1 y el
    reemplazo pasaba invisible. Lo que se perdia ahi era el porque de un bug
    ya arreglado, que cuesta mas que el codigo."""
    igual = _sin_cambio_semantico(antes, despues)
    ca, cd = _comentarios(antes), _comentarios(despues)
    borrados = sum((ca - cd).values())
    agregados = sum((cd - ca).values())
    if igual and borrados:
        return "descartar", f"borra {borrados} comentario(s) y no cambia comportamiento"
    if igual and not agregados:
        return "descartar", "no cambia comportamiento ni documentacion (ruido)"
    if igual:
        return "revisar", f"solo documentacion (+{agregados} comentarios)"
    if borrados:
        return "revisar", (f"cambia comportamiento y pisa {borrados} comentario(s) "
                           "preexistente(s): revisar que el porque no se haya perdido")
    return "ok", "cambio de comportamiento sin perdida de documentacion"


def triage_branch(repo: str, branch: str, *, base: str, git_fn=None) -> dict:
    """Veredicto agregado de una branch: el PEOR de sus archivos .py manda.

    `git_fn(repo, *args) -> CompletedProcess` inyectable (seam de test sin git).
    Devuelve {veredicto, motivo, archivos: {path: [veredicto, motivo]}}."""
    if git_fn is None:
        from mmorch.automerge import _git as git_fn
    diff = git_fn(repo, "diff", "--name-only", f"{base}..{branch}")
    if diff.returncode != 0:
        return {"veredicto": "revisar", "motivo": f"git diff fallo: {diff.stderr[:100]}",
                "archivos": {}}
    paths = [p.strip() for p in diff.stdout.splitlines()
             if p.strip().endswith(".py")]
    if not paths:
        return {"veredicto": "revisar", "motivo": "sin archivos .py en el diff",
                "archivos": {}}
    archivos: dict[str, list[str]] = {}
    for p in paths:
        antes = git_fn(repo, "show", f"{base}:{p}")
        despues = git_fn(repo, "show", f"{branch}:{p}")
        if antes.returncode != 0 or despues.returncode != 0:
            archivos[p] = ["ok", "archivo nuevo o borrado: sin par para comparar"]
            continue
        archivos[p] = list(triage_archivo(antes.stdout, despues.stdout))
    peor = min((v[0] for v in archivos.values()), key=VEREDICTOS.index)
    motivo = next(v[1] for v in archivos.values() if v[0] == peor)
    return {"veredicto": peor, "motivo": motivo, "archivos": archivos}


def _demo() -> None:
    """Self-check con los 3 casos REALES del 2026-08-25 (reducidos)."""
    base = "def f(x):\n    # el porque de este borde, medido\n    return x + 1\n"

    # 1. solo espacios/salto de linea -> ruido
    assert triage_archivo(base, base.rstrip("\n"))[0] == "descartar"

    # 2. borra el comentario sin tocar el comportamiento
    sin_comentario = "def f(x):\n    return x + 1\n"
    v, m = triage_archivo(base, sin_comentario)
    assert v == "descartar" and "borra 1" in m

    # 3. cambia comportamiento Y se lleva el comentario puesto
    otro = "def f(x):\n    return x + 2\n"
    assert triage_archivo(base, otro)[0] == "revisar"

    # 3b. el caso REAL (health.py): reemplaza el comentario — saca 1, pone 1.
    #     Por conteo neto daba 0 y pasaba invisible; por texto se ve.
    reemplaza = "def f(x):\n    # otra explicacion cualquiera\n    return x + 2\n"
    v, m = triage_archivo(base, reemplaza)
    assert v == "revisar" and "pisa 1" in m

    # 3c. mover el MISMO comentario de lugar no es borrarlo
    movido = "# el porque de este borde, medido\ndef f(x):\n    return x + 2\n"
    assert triage_archivo(base, movido)[0] == "ok"

    # 4. cambio real que conserva la documentacion -> pasa
    bueno = base.replace("return x + 1", "return x + 2")
    assert triage_archivo(base, bueno)[0] == "ok"

    # 5. solo agrega documentacion -> no es ruido, pero tampoco automerge
    mas_doc = base.replace("    return", "    # segundo comentario\n    return")
    assert triage_archivo(base, mas_doc)[0] == "revisar"

    print("triage ok (7 casos)")


if __name__ == "__main__":
    _demo()
