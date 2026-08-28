"""Refutacion cross-family de una branch propuesta — el escalon entre el triage
mecanico y el juicio humano.

El triage saca la basura obvia mirando la forma del diff. Lo que queda necesita
entender la INTENCION, y ahi el filtro mecanico no llega: el caso medido
(2026-08-25) fue una branch que agregaba un parametro `isolate=True` a una
funcion que no aisla nada — el nombre promete una garantia que el codigo no da.
Ningun conteo de comentarios encuentra eso; un lector esceptico si.

Reusa patterns.adversarial_verify, que ya exige verificador de OTRA familia que
el generador (invariante OneFlow) y ya trae el prompt que refuta por default.
Aca solo vive la rubrica: las formas de fallar que se midieron de verdad.

Costo: 1 llamada por branch (~USD 0.0009 con DeepSeek/Gemini).

NO ES UN GATE — medido 2026-08-25, n=4: refuta 4 de 4, incluidas las 2 branches
que un humano mergeo y un commit propio que arreglo una caida real de 6 noches.
Cero discriminacion. Las objeciones no son tontas ("no verificas si schtasks
tuvo exito", "no explicas por que 20 y no otro numero"): son CIERTAS e
INMATERIALES, que es peor, porque parecen fundadas. Es el mismo false-refute
que §18.4 midio en ~74% para tareas dificiles, y el prompt esceptico + una
rubrica con 4 formas de fallar lo empuja al 100%.

Se usa como ADVISORY adjunto a la revision humana: el escepticismo escrito al
lado del diff ayuda a leerlo. Para que sea gate le falta una vara, y la vara
tiene que ser EJECUTABLE: exigirle al refutador un caso concreto (entrada ->
salida esperada vs real) y CORRERLO. Una refutacion que no se puede ejecutar es
una opinion, y el repo ya tiene la doctrina escrita (patterns.adversarial_verify:
"prefer a TOOL/code check over any LLM verifier when you can compute the truth").
"""

from __future__ import annotations

_MAX_DIFF = 12000

RUBRICA = """Este es el diff de un cambio propuesto por un modelo sobre un repo
de orquestacion. Paso la suite de tests completa, asi que "los tests pasan" NO
es evidencia de nada. Tu trabajo es TUMBARLO con evidencia concreta del diff.

Buscá especificamente estas formas de fallar, medidas en cambios reales:

1. NOMBRE QUE MIENTE: un parametro, flag o funcion nueva cuyo nombre promete una
   garantia que el codigo no da (ej: un `isolate=True` en una funcion que no
   aisla nada, un "atomic" que no sobrevive a que maten el proceso).
2. BORDE QUE SE MUEVE: el cambio altera el comportamiento en un limite —
   fechas, vacios, off-by-one, comparaciones de tipos distintos — de una forma
   que el autor no menciona. Calculá el borde con datos concretos.
3. GARANTIA QUE NO MEJORA: agrega maquinaria (context manager, wrapper, capa)
   sin cambiar lo que puede fallar. Preguntate: ¿que caso que ANTES rompia,
   ahora no rompe? Si no hay ninguno, es complejidad pura.
4. PORQUE BORRADO: pisa un comentario o log que explicaba una decision o un bug
   ya arreglado, sin reponer esa informacion.

Reglas: refutá SOLO con evidencia puntual del diff (citá la linea). No refutes
por estilo, por gusto, ni por "podria ser mejor". Si mirando el diff no
encontras ninguna de las cuatro, el cambio pasa."""


def refutar_branch(repo: str, branch: str, *, base: str,
                   verify_fn=None, git_fn=None) -> dict:
    """Un modelo de OTRA familia intenta tumbar la branch. Devuelve
    {refutado, razones, modelo, confianza, costo_usd}.

    Fail-open: si el verificador explota o no hay API, NO se refuta — un
    problema de infra jamas descarta trabajo (se manda al humano igual)."""
    if git_fn is None:
        from mmorch.automerge import _git as git_fn
    if verify_fn is None:
        from mmorch.patterns import adversarial_verify as verify_fn
    d = git_fn(repo, "diff", f"{base}..{branch}")
    if d.returncode != 0 or not d.stdout.strip():
        return {"refutado": False, "razones": [], "modelo": "",
                "confianza": 0.0, "costo_usd": 0.0,
                "error": "diff vacio o git fallo"}
    try:
        v = verify_fn(d.stdout[:_MAX_DIFF], rubric=RUBRICA, phase="triage_branch")
    except Exception as e:
        return {"refutado": False, "razones": [], "modelo": "",
                "confianza": 0.0, "costo_usd": 0.0,
                "error": f"{type(e).__name__}: {str(e)[:120]}"}
    return {"refutado": not v.passed, "razones": v.refutations,
            "modelo": v.verifier_model, "confianza": v.confidence,
            "costo_usd": v.cost_usd}


def _demo() -> None:
    """Self-check sin API: el seam inyectado decide, y el fail-open se respeta."""
    import types

    def _git_ok(repo, *args):
        return types.SimpleNamespace(returncode=0, stdout="- viejo\n+ nuevo\n", stderr="")

    def _refuta(artifact, **kw):
        return types.SimpleNamespace(passed=False, confidence=0.9,
                                     refutations=["el flag no hace lo que dice"],
                                     verifier_model="m", cost_usd=0.0)

    r = refutar_branch("r", "b", base="main", verify_fn=_refuta, git_fn=_git_ok)
    assert r["refutado"] and "flag" in r["razones"][0]

    def _explota(artifact, **kw):
        raise RuntimeError("sin API")

    r = refutar_branch("r", "b", base="main", verify_fn=_explota, git_fn=_git_ok)
    assert r["refutado"] is False and "sin API" in r["error"]   # fail-open

    def _git_vacio(repo, *args):
        return types.SimpleNamespace(returncode=0, stdout="", stderr="")

    assert refutar_branch("r", "b", base="main", verify_fn=_refuta,
                          git_fn=_git_vacio)["refutado"] is False
    print("refutar ok (3 casos)")


if __name__ == "__main__":
    _demo()
