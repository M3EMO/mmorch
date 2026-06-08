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


def _det_bareiss(M: list[list[int]]) -> int:
    """Determinante ENTERO EXACTO (algoritmo de Bareiss, fraction-free). Sin numpy,
    sin floats -> sin error de redondeo. Para los casos que `arithmetic` no expresa
    (matrices) — justo donde el LLM-verify erro el signo."""
    n = len(M)
    if any(len(r) != n for r in M):
        raise ValueError("matriz no cuadrada")
    M = [[int(x) for x in row] for row in M]
    sign, prev = 1, 1
    for i in range(n - 1):
        if M[i][i] == 0:
            swap = next((r for r in range(i + 1, n) if M[r][i] != 0), None)
            if swap is None:
                return 0
            M[i], M[swap] = M[swap], M[i]
            sign = -sign
        for j in range(i + 1, n):
            for k in range(i + 1, n):
                M[j][k] = (M[j][k] * M[i][i] - M[j][i] * M[i][k]) // prev
        prev = M[i][i]
    return sign * M[n - 1][n - 1]


def _check_determinant(*, matrix, expected, **_) -> CheckResult:
    """Verifica el determinante de una matriz entera contra `expected` (exacto)."""
    try:
        got = _det_bareiss([list(r) for r in matrix])
    except (ValueError, TypeError) as e:
        return CheckResult(False, f"matriz invalida: {e}", "determinant", expected, None)
    ok = (got == expected)
    return CheckResult(ok, f"det = {got} (esperado {expected})", "determinant", expected, got)


def _check_predicate(*, value, predicate: Callable[[Any], bool], **_) -> CheckResult:
    """Corre un predicado del caller (cualquier check determinista en codigo)."""
    ok = bool(predicate(value))
    return CheckResult(ok, f"predicate({value!r})={ok}", "predicate", True, ok)


# --- checksum: digito verificador (luhn|isbn10|isbn13|ean13) ---------------- #
def _luhn(d: list[int]) -> bool:
    s = 0
    for i, x in enumerate(reversed(d)):
        s += x if i % 2 == 0 else (x * 2 - 9 if x * 2 > 9 else x * 2)
    return s % 10 == 0


def _check_checksum(*, value: str, kind: str = "luhn", **_) -> CheckResult:
    raw = str(value).strip().replace("-", "").replace(" ", "")
    try:
        if kind == "luhn":
            ok = raw.isdigit() and _luhn([int(c) for c in raw])
        elif kind == "isbn10":
            ds = [10 if c in "Xx" else int(c) for c in raw]
            ok = len(ds) == 10 and sum((10 - i) * d for i, d in enumerate(ds)) % 11 == 0
        elif kind in ("isbn13", "ean13"):
            ds = [int(c) for c in raw]
            n = 13 if kind == "isbn13" else 13
            ok = len(ds) == n and sum((1 if i % 2 == 0 else 3) * d
                                      for i, d in enumerate(ds)) % 10 == 0
        else:
            return CheckResult(False, f"kind desconocido: {kind}", "checksum")
    except ValueError:
        return CheckResult(False, f"valor no valido para {kind}: {value!r}", "checksum")
    return CheckResult(ok, f"{kind}({value}) {'valido' if ok else 'INVALIDO'}", "checksum")


# --- python_ast_valid: sintaxis valida (NO ejecuta) ------------------------- #
def _check_python_ast(*, code: str, **_) -> CheckResult:
    try:
        ast.parse(code)
        return CheckResult(True, "sintaxis Python valida", "python_ast_valid")
    except SyntaxError as e:
        return CheckResult(False, f"SyntaxError: {e}", "python_ast_valid")


# --- regex_format: matchea un patron (nombrado o custom) -------------------- #
_NAMED_RE = {
    "email": r"[^@\s]+@[^@\s]+\.[^@\s]+",
    "uuid": r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    "ipv4": r"(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)",
    "iso_date": r"\d{4}-\d{2}-\d{2}",
    "hex_color": r"#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})",
}


def _check_regex(*, value: str, fmt: str | None = None, pattern: str | None = None, **_) -> CheckResult:
    import re as _re
    pat = pattern if pattern is not None else _NAMED_RE.get(fmt or "")
    if pat is None:
        return CheckResult(False, f"formato desconocido: {fmt!r}", "regex_format")
    ok = _re.fullmatch(pat, str(value)) is not None
    return CheckResult(ok, f"{value!r} {'matchea' if ok else 'NO matchea'} {fmt or pat}",
                       "regex_format")


# --- set_equal / numeric_close ---------------------------------------------- #
def _check_set_equal(*, a, b, **_) -> CheckResult:
    ok = set(a) == set(b)
    return CheckResult(ok, f"sets {'iguales' if ok else 'distintos'} "
                       f"(|a|={len(set(a))}, |b|={len(set(b))})", "set_equal")


def _check_numeric_close(*, a, b, tol: float = 1e-9, **_) -> CheckResult:
    ok = abs(float(a) - float(b)) <= tol
    return CheckResult(ok, f"|{a}-{b}|<= {tol}? {ok}", "numeric_close", b, a)


# --- sorted_monotonic ------------------------------------------------------- #
def _check_monotonic(*, seq, direction: str = "asc", strict: bool = False, **_) -> CheckResult:
    s = list(seq)
    if direction == "desc":
        s = s[::-1]
    op = (lambda x, y: x < y) if strict else (lambda x, y: x <= y)
    ok = all(op(s[i], s[i + 1]) for i in range(len(s) - 1))
    return CheckResult(ok, f"{'estrictamente ' if strict else ''}monotona {direction}? {ok}",
                       "sorted_monotonic")


# --- number_theory: primalidad (Miller-Rabin determinista, sin dep) --------- #
def _is_prime(n: int) -> bool:
    if n < 2:
        return False
    for p in (2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37):
        if n % p == 0:
            return n == p
    d, r = n - 1, 0
    while d % 2 == 0:
        d //= 2; r += 1
    for a in (2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37):  # determinista para n < 3.3e24
        x = pow(a, d, n)
        if x in (1, n - 1):
            continue
        for _ in range(r - 1):
            x = x * x % n
            if x == n - 1:
                break
        else:
            return False
    return True


def _check_number_theory(*, n: int, claim: str = "prime", expected: bool = True, **_) -> CheckResult:
    if claim == "prime":
        got = _is_prime(int(n))
    elif claim == "composite":
        got = not _is_prime(int(n)) and int(n) > 1
    else:
        return CheckResult(False, f"claim desconocido: {claim}", "number_theory")
    ok = (got == expected)
    return CheckResult(ok, f"{n} {claim}? got={got} expected={expected}", "number_theory", expected, got)


_REGISTRY: dict[str, Callable[..., CheckResult]] = {
    "arithmetic": _check_arithmetic,
    "determinant": _check_determinant,
    "json_schema": _check_json_schema,
    "predicate": _check_predicate,
    "checksum": _check_checksum,
    "python_ast_valid": _check_python_ast,
    "regex_format": _check_regex,
    "set_equal": _check_set_equal,
    "numeric_close": _check_numeric_close,
    "sorted_monotonic": _check_monotonic,
    "number_theory": _check_number_theory,
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
