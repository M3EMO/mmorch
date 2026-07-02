"""lang — capacidades deterministas POR LENGUAJE para el project-build engine.

El engine es agnóstico de lenguaje en todo (gates = ejecutar test_cmd; DAG/worktree/commits no
miran el código) EXCEPTO en dos checks deterministas que sí leen el fuente: ¿parsea? (floor) y
¿es un stub? (trigger de recursión). Esos dos viven acá, keyed por extensión, para que agregar
un lenguaje = agregar una clase, sin tocar F1/F2/F3.

Honestidad por nivel: Python tiene AST en stdlib -> stub-check fino (todas-las-funciones-triviales).
JS usa el parser REAL de Node (`node --check`, cero deps) para sintaxis, y para stub un check
textual (import/export-only, cuerpos vacíos) — más grueso, pero el juez final sigue siendo el
gate de EJECUCIÓN; acá solo se caza el vapor obvio (la clase de falla del stub de 130 chars).
Desconocido -> Generic (sustancia textual mínima). Nunca aceptar vacío; nunca frenar por no
conocer el lenguaje.
"""
from __future__ import annotations

import ast
import os
import re
import subprocess
import tempfile


class PyLang:
    """Python: AST de stdlib — el check más fino (el original de F1, movido acá)."""
    exts = (".py",)

    @staticmethod
    def _trivial(s: ast.stmt) -> bool:
        if isinstance(s, ast.Pass):
            return True
        if isinstance(s, ast.Expr) and isinstance(s.value, ast.Constant) and s.value.value is Ellipsis:
            return True
        if isinstance(s, ast.Raise):
            exc = s.exc
            name = (getattr(exc, "id", None) or getattr(getattr(exc, "func", None), "id", None)
                    or getattr(exc, "attr", None) or getattr(getattr(exc, "func", None), "attr", None))
            return name in ("NotImplementedError", "NotImplemented")
        return False

    def syntax_ok(self, code: str) -> tuple[bool, str]:
        try:
            ast.parse(code)
            return True, ""
        except SyntaxError as e:
            return False, f"syntax error: {str(e)[:80]}"

    def stub_check(self, code: str) -> tuple[bool, str]:
        ok, why = self.syntax_ok(code)
        if not ok:
            return True, why
        tree = ast.parse(code)
        funcs = [n for n in ast.walk(tree) if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))]
        classes = [n for n in ast.walk(tree) if isinstance(n, ast.ClassDef)]
        if not funcs and not classes:
            return True, "no function/class definitions (import-only / __all__ stub)"
        if not funcs:
            return False, ""   # clases config/atributos = no-stub legítimo
        trivial = 0
        for f in funcs:
            body = [s for s in f.body
                    if not (isinstance(s, ast.Expr) and isinstance(s.value, ast.Constant)
                            and isinstance(s.value.value, str))]
            if not body or all(self._trivial(s) for s in body):
                trivial += 1
        if trivial == len(funcs):
            return True, f"all {len(funcs)} function(s) are stubs (pass/.../NotImplementedError)"
        return False, ""


_JS_COMMENT_RX = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)
_JS_IMPORTISH_RX = re.compile(r"^(import\b|export\s+(\{|default\s+\w+\s*;|\*)|const\s+\w+\s*=\s*require\b)")


class JsLang:
    """JS/ESM: sintaxis via el parser REAL de Node (`node --check`, cero deps). Stub = textual:
    import/export-only o sin sustancia. Más grueso que el AST de Python — el gate de ejecución
    (node --test) es quien realmente juzga; acá solo se caza el vapor obvio."""
    exts = (".js", ".mjs", ".cjs")

    def syntax_ok(self, code: str) -> tuple[bool, str]:
        try:
            # --check parsea sin ejecutar. .mjs para aceptar sintaxis ESM (import/export).
            with tempfile.NamedTemporaryFile("w", suffix=".mjs", delete=False,
                                             encoding="utf-8") as f:
                f.write(code)
                tmp = f.name
            try:
                p = subprocess.run(["node", "--check", tmp], capture_output=True, text=True,
                                   timeout=15)
                return p.returncode == 0, (p.stderr.strip()[:120] if p.returncode else "")
            finally:
                os.unlink(tmp)
        except FileNotFoundError:
            return True, ""    # sin node en PATH: fail-open (el gate de ejecución fallará después)
        except Exception as e:
            return True, f"(node --check no disponible: {str(e)[:60]})"

    def stub_check(self, code: str) -> tuple[bool, str]:
        ok, why = self.syntax_ok(code)
        if not ok:
            return True, why
        body = _JS_COMMENT_RX.sub("", code)
        lines = [ln.strip() for ln in body.splitlines() if ln.strip()]
        if not lines:
            return True, "empty / comments-only"
        substantive = [ln for ln in lines if not _JS_IMPORTISH_RX.match(ln)]
        if not substantive:
            return True, "import/export-only stub"
        # cuerpos vacíos: todo lo declarado termina en '{}' (function f() {} / () => {})
        if all(re.search(r"\{\s*\}\s*;?$", ln) or _JS_IMPORTISH_RX.match(ln) for ln in lines):
            return True, "all declared bodies are empty"
        return False, ""


class GenericLang:
    """Fallback para extensiones no registradas: nunca frenar por desconocer el lenguaje,
    nunca aceptar vacío. Sustancia textual mínima; la ejecución juzga el resto."""
    exts = ()

    def syntax_ok(self, code: str) -> tuple[bool, str]:
        return True, ""

    def stub_check(self, code: str) -> tuple[bool, str]:
        lines = [ln.strip() for ln in code.splitlines()
                 if ln.strip() and not ln.strip().startswith(("#", "//", "/*", "*"))]
        if len(lines) < 2:
            return True, f"no substance ({len(lines)} non-comment line(s))"
        return False, ""


_LANGS: list[PyLang | JsLang] = [PyLang(), JsLang()]
_GENERIC = GenericLang()
_BY_EXT: dict[str, PyLang | JsLang] = {e: lang for lang in _LANGS for e in lang.exts}


def for_file(file: str | None):
    """Lenguaje para un path (por extensión). None -> Python (back-compat: el engine nació py)."""
    if not file:
        return _BY_EXT[".py"]
    return _BY_EXT.get(os.path.splitext(str(file))[1].lower(), _GENERIC)


if __name__ == "__main__":
    py = for_file("a.py")
    assert py.stub_check("from .x import y\n__all__=['y']")[0] is True          # el stub original
    assert py.stub_check("def f():\n    return 1")[0] is False
    assert py.stub_check("def f():\n    raise m.NotImplementedError")[0] is True
    assert py.syntax_ok("def f(:")[0] is False

    js = for_file("app/main.js")
    assert isinstance(js, JsLang)
    assert js.stub_check("import { x } from './x.js';\nexport { x };")[0] is True   # import-only
    assert js.stub_check("// solo comentarios\n/* nada */")[0] is True
    ok_js = "export function inc(x) {\n  return x + 1;\n}\n"
    assert js.stub_check(ok_js) == (False, ""), js.stub_check(ok_js)
    s_ok, s_why = js.syntax_ok("function f( {")            # roto
    assert s_ok is False or "no disponible" in s_why       # False con node; fail-open sin node

    g = for_file("styles.css")
    assert isinstance(g, GenericLang)
    assert g.stub_check("")[0] is True
    assert g.stub_check(".btn { color: red; }\n.tab { display: flex; }")[0] is False

    assert for_file(None).__class__ is PyLang              # back-compat
    print("lang OK — py(AST) / js(node --check + textual) / generic, registry por extensión")
