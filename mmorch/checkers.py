"""checkers — libreria propia de VERIFICADORES DETERMINISTAS (tool-verify).

Hallazgo (ablation_prompt, n=120): en checkeable-computable el LLM-verify es
irreducible-mente malo (~74% false-refute) porque el modelo no puede recomputar el
problema. La fix no es prompt: es VERIFICAR CON CODIGO. Esta es la libreria que mmorch
acumula — checkers nombrados, reutilizables, cero API, 100% confiables donde aplican.

Dos capas (la intuicion del usuario, unificada en un registry):
  - ESTRUCTURAL: el output esta bien formado (shape/type). -> wrap de schema.py.
  - COMPUTACIONAL: la AFIRMACION es verdadera contra ground-truth computable. -> aca:
    arithmetic (re-evalua una expr en sandbox ast), python_predicate (corre un check).

Uso:
    from mmorch.checkers import check, register_checker
    r = check("arithmetic", expr="comb(20,10)", expected=184756)   # r.passed == True
    register_checker("mi_check", fn)                                # extender la libreria

adversarial_verify(task_kind="checkable") deberia rutear aca cuando hay checker que
aplique, en vez de gastar un LLM que igual no sabe resolverlo.
"""
from __future__ import annotations

import ast
import math
import operator
from dataclasses import dataclass
from typing import Any, Callable


@dataclass
class CheckResult:
    passed: bool
    detail: str
    checker: str
    expected: Any = None
    got: Any = None


# --------------------------------------------------------------------------- #
# sandbox aritmetico (ast walk; NUNCA eval/exec sobre texto del modelo)        #
# --------------------------------------------------------------------------- #
_BIN = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul,
        ast.Div: operator.truediv, ast.FloorDiv: operator.floordiv,
        ast.Mod: operator.mod, ast.Pow: operator.pow}
_UNARY = {ast.UAdd: operator.pos, ast.USub: operator.neg}
_FUNCS: dict[str, Callable] = {
    "sqrt": math.isqrt, "isqrt": math.isqrt, "factorial": math.factorial,
    "comb": math.comb, "perm": math.perm, "gcd": math.gcd, "lcm": math.lcm,
    "abs": abs, "round": round, "sum": sum, "pow": pow, "floor": math.floor,
    "ceil": math.ceil, "fabs": math.fabs,
}
_CONSTS = {"pi": math.pi, "e": math.e}


class UnsafeExpr(ValueError):
    """La expresion usa algo fuera del whitelist (nombre/op/funcion no permitida)."""


def _eval(node: ast.AST) -> Any:
    if isinstance(node, ast.Expression):
        return _eval(node.body)
    if isinstance(node, ast.Constant):
        if isinstance(node.value, (int, float)):
            return node.value
        raise UnsafeExpr(f"constante no numerica: {node.value!r}")
    if isinstance(node, ast.BinOp) and type(node.op) in _BIN:
        return _BIN[type(node.op)](_eval(node.left), _eval(node.right))
    if isinstance(node, ast.UnaryOp) and type(node.op) in _UNARY:
        return _UNARY[type(node.op)](_eval(node.operand))
    if isinstance(node, ast.Name) and node.id in _CONSTS:
        return _CONSTS[node.id]
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) \
            and node.func.id in _FUNCS and not node.keywords:
        return _FUNCS[node.func.id](*[_eval(a) for a in node.args])
    if isinstance(node, (ast.List, ast.Tuple)):
        return [_eval(e) for e in node.elts]
    raise UnsafeExpr(f"nodo no permitido: {type(node).__name__}")


def safe_arith(expr: str) -> float:
    """Evalua una expr aritmetica en sandbox (solo numeros, ops y funcs whitelisted).
    NO usa eval/exec. Tira UnsafeExpr si la expr sale del whitelist."""
    tree = ast.parse(expr, mode="eval")
    return _eval(tree)


def _isclose(a, b) -> bool:
    try:
        return abs(float(a) - float(b)) <= max(1e-9, abs(float(b)) * 1e-9)
    except (TypeError, ValueError):
        return a == b


# --------------------------------------------------------------------------- #
# checkers built-in                                                            #
# --------------------------------------------------------------------------- #
def _check_arithmetic(*, expr: str, expected, **_) -> CheckResult:
    """Re-evalua `expr` en sandbox y compara con `expected`. 100% confiable donde el
    LLM-verify daba 74% false-refute."""
    try:
        got = safe_arith(expr)
    except UnsafeExpr as e:
        return CheckResult(False, f"expr rechazada: {e}", "arithmetic", expected, None)
    ok = _isclose(got, expected)
    return CheckResult(ok, f"{expr} = {got} (esperado {expected})", "arithmetic",
                       expected, got)


def _check_json_schema(*, data, schema: dict, **_) -> CheckResult:
    """Capa estructural: valida shape/type contra un JSON-schema (wrap de schema.py)."""
    from .schema import validate, extract_json
    if isinstance(data, str):
        data = extract_json(data)
    errs = validate(data, schema)
    return CheckResult(not errs, "ok" if not errs else "; ".join(errs), "json_schema")


def _check_predicate(*, value, predicate: Callable[[Any], bool], **_) -> CheckResult:
    """Corre un predicado del caller (cualquier check determinista en codigo)."""
    ok = bool(predicate(value))
    return CheckResult(ok, f"predicate({value!r})={ok}", "predicate", True, ok)


_REGISTRY: dict[str, Callable[..., CheckResult]] = {
    "arithmetic": _check_arithmetic,
    "json_schema": _check_json_schema,
    "predicate": _check_predicate,
}


def register_checker(name: str, fn: Callable[..., CheckResult]) -> None:
    """Suma un checker determinista a la libreria. fn(**ctx) -> CheckResult."""
    _REGISTRY[name] = fn


def available() -> list[str]:
    return sorted(_REGISTRY)


def check(name: str, **ctx) -> CheckResult:
    """Despacha al checker `name` con el contexto dado. KeyError si no existe."""
    if name not in _REGISTRY:
        raise KeyError(f"checker '{name}' no registrado. Disponibles: {available()}")
    return _REGISTRY[name](**ctx)
