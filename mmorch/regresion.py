"""Refutacion EJECUTABLE de una branch: la objecion se prueba o no existe.

Por que (medido 2026-08-25/26, n=4 en cada intento):
- un refutador esceptico solo refuta 4 de 4 — objeciones ciertas e inmateriales.
- un debate de 3 roles (refutador/habilitador/juez) aprueba 11 de 11 puntos:
  empata con no llamar a nadie.
Los dos fallan por lo mismo: la prosa no tiene vara. "Esto podria romper un
borde" no se puede confirmar ni desmentir, asi que el veredicto termina
dependiendo del rol que le tocó al modelo, no del codigo.

Aca el refutador no escribe prosa: escribe un TEST que debe PASAR en `base` y
FALLAR en la branch. Se corre en los dos arboles y el resultado decide:

    pasa en base  +  falla en branch  ->  REGRESION REAL (material)
    cualquier otra combinacion        ->  la objecion no se sostiene, se cae

Nadie opina. Si el modelo alucina una regresion, su propio test lo desmiente.

LIMITES CONOCIDOS, ninguno es un bug:

1. Atrapa cambios de COMPORTAMIENTO. Un olor de diseño —el caso real fue un
   parametro `isolate=True` en una funcion que no aisla nada— no cambia ningun
   comportamiento observable y por lo tanto NO es expresable como test. Eso
   sigue yendo al humano. El reparto es a proposito: lo que rompe se bloquea
   con prueba, lo que huele se conversa.

2. Prueba DIFERENCIA, no ALCANZABILIDAD. El test puede demostrar la regresion
   por un camino que ningun caller usa hoy: medido 2026-08-26, la regresion
   real de health.py se probo con `hours<24` y el unico caller pasa el default
   de 48. La diferencia era cierta igual (tambien rompe en 48), pero "probado"
   no quiere decir "te va a pasar mañana".

3. `declara_intencion` es un proxy grueso (¿el diff agrega asserts?). No
   verifica que el assert cubra JUSTO el caso demostrado, asi que una branch
   que agregue un assert de cualquier otra cosa se lleva el perdon. Por eso no
   absuelve: degrada de 'regresion probada' a 'cambio declarado'.

Medicion (n=5, casos reales del 2026-08-25, etiquetados a mano):
5/5 con gemini-2.5-flash, contra 4/5 de la linea base "aprobar siempre" — y la
diferencia es justo la unica regresion real. Los dos intentos anteriores
(refutador solo, debate de 3 roles) NO le ganaban a esa linea base.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

_MAX_DIFF = 12000
_TIMEOUT = 300
_LLM_TIMEOUT = 180      # 60s no alcanza con un diff grande: medido, 3/5 timeouts
# el DEFAULT_VERIFIER (flash-lite) no da la talla para esta tarea: escribio un
# test que no pasaba ni en base. Medido sobre los mismos 5 casos —
# flash-lite 3/5, deepseek-reasoner sin corridas validas (timeouts),
# gemini-2.5-flash 5/5.
MODELO = "gemini-2.5-flash"


def declara_intencion(diff: str) -> bool:
    """La branch AGREGA asserts (tests o self-check `_demo`)?

    Sin esto, "cambio de comportamiento" e "arreglo intencional" son
    indistinguibles: un test escrito contra la conducta VIEJA pasa en base y
    falla en la branch, que es exactamente la firma de una regresion. Medido
    2026-08-26 con 3 modelos: 2 de 3 marcaron como regresion un cambio
    deliberado (stuck_detector) que ademas venia con su propio caso de
    self-check. La intencion estaba en el parche; a la tabla de verdad le
    faltaba mirarla.

    Proxy a proposito, no prueba: un assert nuevo no garantiza que cubra JUSTO
    el caso demostrado. Por eso no absuelve — degrada de 'regresion probada' a
    'cambio declarado', que es una fila distinta para el humano."""
    return any(ln.startswith("+") and not ln.startswith("+++")
               and "assert" in ln for ln in diff.splitlines())

_TEST_SCHEMA = {
    "type": "object",
    "properties": {
        "hay_regresion": {"type": "boolean"},
        "explicacion": {"type": "string"},
        "test": {"type": "string"},
    },
    "required": ["hay_regresion"],
}

_PROMPT = """Este es el diff de un cambio propuesto sobre un repo Python. La
suite completa pasa con el cambio aplicado, asi que "los tests pasan" NO es
evidencia de nada: si hay una regresion, es de un caso que la suite no cubre.

Tu trabajo NO es opinar. Es escribir un TEST DE PYTEST que demuestre una
regresion real de comportamiento:

  - tiene que PASAR con el codigo VIEJO (antes del diff)
  - tiene que FALLAR con el codigo NUEVO (despues del diff)

Lo vamos a correr en los dos arboles. Si no se comporta asi, tu objecion se
descarta automaticamente — no la defiende nadie.

Reglas del test:
- un solo archivo, autocontenido, sin dependencias nuevas, sin red, sin sleep.
- importa del paquete `mmorch` normalmente (el repo esta en el path).
- una sola funcion `def test_regresion():` con asserts concretos.
- si necesitas datos, construilos en el test (tmp files con tempfile, no rutas
  del repo).

Si mirando el diff NO encontras una regresion de comportamiento demostrable
—porque el cambio es correcto, o porque el problema es de diseño/nombres y no
de comportamiento— devolve hay_regresion=false y test="". Eso es una respuesta
valida y frecuente: NO inventes un test para llenar.

DIFF:
{diff}

Devolve EXACTAMENTE este JSON:
{{"hay_regresion": bool, "explicacion": str, "test": str}}"""


def _correr_test(repo: str, ref: str, codigo: str, *, timeout: int = _TIMEOUT) -> dict:
    """Corre `codigo` como test contra el arbol de `ref`, en un worktree
    efimero. Devuelve {paso, detalle}. El worktree se borra siempre."""
    from mmorch.worktree_driver import open_worktree
    wt = open_worktree(repo, prefix="mmorch/wt-regresion", base=ref)
    try:
        wt.seed([".venv"])
        destino = Path(wt.path) / "tests" / "test_regresion_generada.py"
        destino.parent.mkdir(exist_ok=True)
        destino.write_text(codigo, encoding="utf-8")
        import tempfile
        bt = tempfile.mkdtemp(prefix="mmorch_reg_")
        p = subprocess.run(
            [sys.executable, "-m", "pytest", "-q", "--no-header",
             str(destino.relative_to(wt.path)).replace("\\", "/"),
             f"--basetemp={bt}"],
            cwd=wt.path, capture_output=True, text=True,
            encoding="utf-8", errors="replace", timeout=timeout)
        return {"paso": p.returncode == 0, "detalle": ((p.stdout or "") + (p.stderr or ""))[-800:]}
    except subprocess.TimeoutExpired:
        return {"paso": False, "detalle": "timeout"}
    finally:
        wt.close(keep_branch=False)


def refutar_ejecutable(repo: str, branch: str, *, base: str, modelo: str | None = None,
                       git_fn=None, llm_fn=None, correr_fn=None) -> dict:
    """Devuelve {material, motivo, test, en_base, en_branch, costo_usd}.

    `material=True` SOLO si el test generado pasa en `base` y falla en `branch`.
    Fail-open: sin API, sin test, o con cualquier error -> material=False (un
    problema de infra jamas descarta trabajo; se manda al humano igual)."""
    if git_fn is None:
        from mmorch.automerge import _git as git_fn
    if correr_fn is None:
        correr_fn = _correr_test
    if llm_fn is None:
        from mmorch.schema import gated_json

        def llm_fn(prompt, *, schema):
            return gated_json(modelo or MODELO,
                              [{"role": "user", "content": prompt}], schema=schema,
                              pattern="refutar_ejecutable", phase="triage_branch",
                              temperature=0.0, timeout=_LLM_TIMEOUT)

    vacio = {"material": False, "motivo": "", "test": "", "en_base": None,
             "en_branch": None, "costo_usd": 0.0, "intencional": False}
    d = git_fn(repo, "diff", f"{base}..{branch}")
    if d.returncode != 0 or not d.stdout.strip():
        return {**vacio, "motivo": "diff vacio o git fallo"}
    intencional = declara_intencion(d.stdout)
    try:
        out = llm_fn(_PROMPT.format(diff=d.stdout[:_MAX_DIFF]), schema=_TEST_SCHEMA)
    except Exception as e:
        return {**vacio, "motivo": f"verificador caido: {type(e).__name__}"}
    costo = float(out.get("_cost_usd") or 0.0)
    codigo = (out.get("test") or "").strip()
    if not out.get("hay_regresion") or not codigo:
        return {**vacio, "costo_usd": costo,
                "motivo": "el verificador no encontro una regresion demostrable"}

    try:
        en_base = correr_fn(repo, base, codigo)
        en_branch = correr_fn(repo, branch, codigo)
    except Exception as e:
        return {**vacio, "costo_usd": costo, "test": codigo,
                "motivo": f"no se pudo ejecutar: {type(e).__name__}: {str(e)[:120]}"}

    difiere = bool(en_base["paso"] and not en_branch["paso"])
    # una diferencia de comportamiento QUE LA BRANCH DECLARA con asserts propios
    # es un arreglo, no una regresion: no bloquea, se le cuenta al humano
    material = difiere and not intencional
    if difiere and intencional:
        motivo = ("cambio de comportamiento DECLARADO por la branch (agrega asserts): "
                  f"{out.get('explicacion', '')[:160]}")
    elif material:
        motivo = f"REGRESION probada: {out.get('explicacion', '')[:200]}"
    elif not en_base["paso"]:
        # el test no pasa ni con el codigo viejo: no describe una regresion,
        # describe algo que nunca funciono (o el test esta mal escrito)
        motivo = "el test tampoco pasa en base: la objecion no se sostiene"
    else:
        motivo = "el test pasa en ambos arboles: la regresion no ocurre"
    return {"material": material, "motivo": motivo, "test": codigo,
            "en_base": en_base["paso"], "en_branch": en_branch["paso"],
            "intencional": intencional, "difiere": difiere,
            "costo_usd": round(costo, 6),
            "detalle_branch": en_branch.get("detalle", "")[-400:] if material else ""}


def _demo() -> None:
    """Self-check sin API ni git: la tabla de verdad es lo unico que decide."""
    import types

    def _git(repo, *args):
        return types.SimpleNamespace(returncode=0, stdout="- a\n+ b\n", stderr="")

    def _llm_con_test(prompt, *, schema):
        return {"hay_regresion": True, "explicacion": "se cae el borde",
                "test": "def test_regresion(): assert True"}

    def _corredor(base_pasa, branch_pasa):
        def fn(repo, ref, codigo, **kw):
            return {"paso": base_pasa if ref == "main" else branch_pasa, "detalle": ""}
        return fn

    def _git_intencional(repo, *args):
        diff = "- a\n+    assert nuevo == 1\n"
        return types.SimpleNamespace(returncode=0, stdout=diff, stderr="")

    kw = dict(repo="r", branch="b", base="main", git_fn=_git, llm_fn=_llm_con_test)
    # pasa en base + falla en branch -> UNICO caso material
    assert refutar_ejecutable(**kw, correr_fn=_corredor(True, False))["material"] is True
    # pasa en los dos -> la regresion no ocurre
    assert refutar_ejecutable(**kw, correr_fn=_corredor(True, True))["material"] is False
    # falla en los dos -> no describe una regresion
    r = refutar_ejecutable(**kw, correr_fn=_corredor(False, False))
    assert r["material"] is False and "tampoco pasa en base" in r["motivo"]
    # falla en base y pasa en branch -> el cambio ARREGLA algo
    assert refutar_ejecutable(**kw, correr_fn=_corredor(False, True))["material"] is False

    # sin regresion declarada -> ni se ejecuta nada
    def _llm_limpio(prompt, *, schema):
        return {"hay_regresion": False, "test": ""}

    def _explota(*a, **k):
        raise AssertionError("no debia ejecutar nada")

    assert refutar_ejecutable(repo="r", branch="b", base="main", git_fn=_git,
                              llm_fn=_llm_limpio, correr_fn=_explota)["material"] is False

    # verificador caido -> fail-open
    def _llm_roto(prompt, *, schema):
        raise RuntimeError("sin API")

    r = refutar_ejecutable(repo="r", branch="b", base="main", git_fn=_git,
                           llm_fn=_llm_roto, correr_fn=_explota)
    assert r["material"] is False and "caido" in r["motivo"]

    # la MISMA diferencia de comportamiento, pero declarada por la branch con
    # asserts propios: es un arreglo, no una regresion -> no bloquea
    r = refutar_ejecutable(repo="r", branch="b", base="main", git_fn=_git_intencional,
                           llm_fn=_llm_con_test, correr_fn=_corredor(True, False))
    assert r["material"] is False and r["difiere"] is True and r["intencional"] is True
    assert "DECLARADO" in r["motivo"]

    assert declara_intencion("+    assert x == 1\n") is True
    assert declara_intencion("-    assert x == 1\n") is False   # BORRAR no declara
    assert declara_intencion("+++ b/tests/test_x.py\n") is False

    print("regresion ok (10 casos)")


if __name__ == "__main__":
    _demo()
