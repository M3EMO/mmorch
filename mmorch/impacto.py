"""impacto — informe de impacto por literales compartidos (mapa `.scratch/mapa-de-impacto`).

Antes de editar un archivo Python, TypeScript o Java, lista los otros modulos que comparten con el literales
de datos (tablas SQL, rutas, claves JSON, variables de entorno).
Medido (ticket 09): con este informe, tareas de cambio con un lector acoplado pasan de 42% a
96% de acierto (deepseek-v4-pro); sin el, el agente cambia el escritor y no toca al lector.
En Python agrega las aristas indirectas de `mmorch.impacto_indirecto` (ticket 04: 38% -> 92%)
y los lectores en prompts y configuracion de agentes de `mmorch.impacto_externo` (ticket 13: 0/12 -> 12/12).

Formato (grilling del ticket 03): orden por especificidad (menos modulos primero), un literal
en mas de COMMON modulos se resume con su conteo, tope de MAX_LINES lineas. Sin linea de
tests: en la aceptacion del ticket 05 no mejoro el acierto (20/24 con ella, 23/24 sin ella) y
promete algo que no cumple, porque los tests visibles no cubren a los lectores acoplados.

Uso como hook de Claude Code (PreToolUse sobre Edit|Write|MultiEdit):
    python -m mmorch.impacto hook   < json del hook   -> json con additionalContext o nada
Uso como hook de Cursor (postToolUse, despues de la primera edicion; ticket 10):
    python -m mmorch.impacto cursor < json del hook   -> json con additional_context o nada
Chequeo de costo despues de escribir en Claude Code (PostToolUse; ticket 15, `mmorch.impacto_costo`):
    python -m mmorch.impacto post   < json del hook   -> json con additionalContext o nada
El modo cursor ya corre despues de editar y suma el mismo chequeo.
Siempre sale con codigo 0: si algo falla, la edicion sigue sin informe (fail-open).
"""
from __future__ import annotations

import ast
import json
import os
import re
import sys
import tempfile
import time
from pathlib import Path

MAX_LINES = 10
COMMON = 5
MAX_FILES = 5000   # ponytail: tope duro para repos enormes; subir si un repo real lo necesita
_TOK = re.compile(r"[A-Za-z_][A-Za-z0-9_]{2,}")
_STOP = {"utf", "encoding", "SELECT", "FROM", "WHERE", "INSERT", "INTO", "VALUES", "UPDATE", "SET",
         "CREATE", "TABLE", "EXISTS", "INTEGER", "PRIMARY", "KEY", "REAL", "TEXT", "COALESCE", "SUM",
         "COUNT", "GROUP", "ORDER", "LIMIT", "DESC", "NOT", "AND", "NULL"}
_SKIP_DIRS = {".git", ".venv", "venv", "node_modules", "__pycache__", "build", "dist", "site-packages",
              ".codegraph", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".tox"}


def _source_files(root: Path, exts: tuple[str, ...]) -> list[Path]:
    out: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in _SKIP_DIRS and not d.startswith(".")]
        out += [Path(dirpath) / f for f in filenames if f.endswith(exts) and not f.endswith(".d.ts")]
        if len(out) >= MAX_FILES:
            break
    return out


def _is_test(rel: str) -> bool:
    parts = rel.split("/")
    name = parts[-1]
    return bool({"tests", "test", "__tests__"} & set(parts[:-1])) or name.startswith("test_") \
        or ".test." in name or ".spec." in name or name.endswith(("Test.java", "Tests.java"))


_SQL = re.compile(r"\s*(select|insert|update|delete|create|with|alter|drop)\b", re.I)


def _looks_like_data(s: str) -> bool:
    """Literal con forma de dato: sin espacios (clave, ruta, variable de entorno) o SQL.
    Los textos en prosa (mensajes, prompts) comparten palabras por azar y solo agregan ruido."""
    s = s.strip()
    return bool(s) and (" " not in s or bool(_SQL.match(s)))


def literal_tokens(src: str) -> dict[str, int]:
    """token -> primera linea donde aparece dentro de un literal de texto (sin docstrings)."""
    tree = ast.parse(src)
    docs = set()
    for n in ast.walk(tree):
        if isinstance(n, (ast.Module, ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and n.body \
                and isinstance(n.body[0], ast.Expr) and isinstance(n.body[0].value, ast.Constant):
            docs.add(id(n.body[0].value))
    out: dict[str, int] = {}
    for n in ast.walk(tree):
        if isinstance(n, ast.Constant) and isinstance(n.value, str) and id(n) not in docs \
                and _looks_like_data(n.value):
            for t in _TOK.findall(n.value):
                if t not in _STOP:
                    out.setdefault(t, n.lineno)
    return out


_TS_PARSERS: dict[str, object] = {}


def _parser(key: str):
    """Parser de tree-sitter por lenguaje ("ts", "tsx", "java"), creado una sola vez."""
    if key not in _TS_PARSERS:
        from tree_sitter import Language, Parser
        if key == "java":
            import tree_sitter_java as tsj
            lang = tsj.language()
        else:
            import tree_sitter_typescript as tst
            lang = tst.language_tsx() if key == "tsx" else tst.language_typescript()
        _TS_PARSERS[key] = Parser(Language(lang))
    return _TS_PARSERS[key]


def _fragment_tokens(key: str, src: str, skip=lambda node: False) -> dict[str, int]:
    """Mismo criterio que `literal_tokens` sobre los nodos `string_fragment` de tree-sitter."""
    data = src.encode("utf-8")
    stack = [_parser(key).parse(data).root_node]
    out: dict[str, int] = {}
    while stack:
        n = stack.pop()
        stack.extend(n.children)
        if n.type != "string_fragment" or skip(n):
            continue
        text = data[n.start_byte:n.end_byte].decode("utf-8", "ignore")
        if _looks_like_data(text):
            for t in _TOK.findall(text):
                if t not in _STOP:
                    out.setdefault(t, n.start_point[0] + 1)
    return out


def _in_import(n) -> bool:
    """La fuente de un import/export ("node:fs", "./store.ts") es codigo, no datos."""
    p = n.parent
    while p is not None and p.type in ("string", "string_fragment"):
        p = p.parent
    return p is not None and p.type in ("import_statement", "export_statement")


def ts_tokens(src: str, tsx: bool = False) -> dict[str, int]:
    """TypeScript (ticket 11, variante v1): literales de texto con forma de dato, sin las fuentes
    de import/export. Medido en el banco TS: acopladas 38% -> 100% (deepseek-v4-pro).
    Requiere tree-sitter (dependencia opcional `impacto-ts`)."""
    return _fragment_tokens("tsx" if tsx else "ts", src, _in_import)


def java_tokens(src: str) -> dict[str, int]:
    """Java (ticket 12): literales de texto con forma de dato. Java no pone rutas de import en
    strings. Medido en el banco Java: acopladas 38% -> 96% (deepseek-v4-pro).
    Requiere tree-sitter (dependencia opcional `impacto-java`)."""
    return _fragment_tokens("java", src)


# extension -> (familia, extensiones de la familia, tokenizador). Cada lenguaje entra solo
# despues de pasar el banco de tareas con lector acoplado (grilling del ticket 03, Q7).
_LANGS = {
    ".py": ("py", (".py",), literal_tokens),
    ".ts": ("ts", (".ts", ".tsx"), ts_tokens),
    ".tsx": ("ts", (".ts", ".tsx"), lambda s: ts_tokens(s, tsx=True)),
    ".java": ("java", (".java",), java_tokens),
}


def _cache_path(root: Path, family: str = "py") -> Path:
    import hashlib
    d = Path(tempfile.gettempdir()) / "mmorch_impacto"
    d.mkdir(exist_ok=True)
    return d / f"tokens_{family}_{hashlib.sha1(str(root).encode()).hexdigest()[:12]}.json"


def _all_tokens(root: Path, suffix: str = ".py") -> dict[str, dict[str, int]]:
    """rel -> tokens de cada modulo no-test del mismo lenguaje, con cache por (mtime_ns, size):
    en Portfolio (327 modulos) el parseo en frio cuesta 5-15 s; con cache, solo lo cambiado."""
    family, exts, _ = _LANGS[suffix]
    cp = _cache_path(root, family)
    try:
        cache = json.loads(cp.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        cache = {}
    toks: dict[str, dict[str, int]] = {}
    fresh: dict[str, list] = {}
    for p in _source_files(root, exts):
        rel = p.relative_to(root).as_posix()
        if _is_test(rel):
            continue
        try:
            st = p.stat()
            key = [st.st_mtime_ns, st.st_size]
            hit = cache.get(rel)
            t = hit[1] if hit and hit[0] == key else _LANGS[p.suffix][2](p.read_text(encoding="utf-8"))
        except (SyntaxError, UnicodeDecodeError, ValueError, OSError):
            continue
        toks[rel] = t
        fresh[rel] = [key, t]
    try:
        cp.write_text(json.dumps(fresh), encoding="utf-8")
    except OSError:
        pass
    return toks


def report(root: Path, target: Path) -> str:
    """Informe de impacto de `target` dentro de `root`. Vacio si no hay nada que avisar.
    En Python suma las aristas indirectas (mmorch.impacto_indirecto, ticket 04)."""
    lit = _literal_report(root, target)
    if target.suffix != ".py":
        return lit
    from mmorch import impacto_indirecto   # import tardio: solo Python lo usa
    root, target = root.resolve(), target.resolve()
    files = [p for p in _source_files(root, (".py",)) if not _is_test(p.relative_to(root).as_posix())]
    F = impacto_indirecto.all_facts(root, files, _cache_path(root, "pyind"))
    rel = target.relative_to(root).as_posix()
    ind = impacto_indirecto.report(F, rel)
    ext = _external_report(target, rel)
    return "\n\n".join(x for x in (lit, ind, ext) if x)


def _external_report(target: Path, rel: str) -> str:
    """Lectores en prompts y configuracion de agentes (mmorch.impacto_externo, ticket 13)."""
    from mmorch import impacto_externo
    try:
        src = target.read_text(encoding="utf-8")
        idx = impacto_externo.index(impacto_externo.config_roots(), _cache_path(Path.home(), "cfg"))
        return impacto_externo.report(src, rel, idx)
    except (SyntaxError, UnicodeDecodeError, ValueError, OSError):
        return ""


def _literal_report(root: Path, target: Path) -> str:
    """Literales de datos que `target` comparte con otros modulos del mismo lenguaje."""
    root, target = root.resolve(), target.resolve()
    if target.suffix not in _LANGS:
        return ""
    rel_target = target.relative_to(root).as_posix()
    try:
        toks = _all_tokens(root, target.suffix)
    except ImportError:   # TS o Java sin tree-sitter instalado: sin informe (fail-open)
        return ""
    if rel_target not in toks and target.is_file():
        try:
            toks[rel_target] = _LANGS[target.suffix][2](target.read_text(encoding="utf-8"))
        except (SyntaxError, UnicodeDecodeError, ValueError, OSError):
            toks[rel_target] = {}
    mine = toks.get(rel_target, {})
    entries = []
    for t, line in mine.items():
        others = sorted(rel for rel, tk in toks.items() if rel != rel_target and t in tk)
        if others:
            entries.append((len(others), t, line, others))
    entries.sort()
    body = []
    for n, t, line, others in entries[:MAX_LINES]:
        if n > COMMON:
            body.append(f"- `{t}` (linea {line}) aparece en {n} modulos mas (literal comun, sin lista)")
        else:
            refs = ", ".join(f"{o}:{toks[o][t]}" for o in others)
            body.append(f"- `{t}` (linea {line}) tambien en: {refs}")
    if len(entries) > MAX_LINES:
        body[-1] = f"- ... y {len(entries) - MAX_LINES + 1} literales compartidos mas"
    if not body:
        return ""
    return (f"INFORME DE IMPACTO (automatico, mmorch.impacto): {rel_target} comparte estos literales "
            "de datos con otros modulos. Si cambias su formato, nombre o significado, revisa esos "
            "modulos:\n" + "\n".join(body))


def _repo_root(path: Path) -> Path | None:
    """Carpeta con `.git` mas cercana. Sin repo, None: una raiz como C:\\ haria recorrer el disco."""
    for d in [path.parent, *path.parent.parents]:
        if (d / ".git").exists():
            return d
    return None


def _seen(session: str, file: str) -> bool:
    """Una vez por archivo por sesion: marca y devuelve si ya se habia informado."""
    d = Path(tempfile.gettempdir()) / "mmorch_impacto"
    d.mkdir(exist_ok=True)
    marks = d / f"{re.sub(r'[^A-Za-z0-9_-]', '_', session)[:80]}.txt"
    done = set(marks.read_text(encoding="utf-8").splitlines()) if marks.exists() else set()
    if file in done:
        return True
    with marks.open("a", encoding="utf-8") as fh:
        fh.write(file + "\n")
    return False


def _log(row: dict) -> None:
    """Dueno unico de logs/impacto.jsonl: un registro por informe calculado."""
    from .paths import logs_dir
    with (logs_dir() / "impacto.jsonl").open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(row, ensure_ascii=False) + "\n")


def _target(data: dict) -> tuple[Path, str, Path] | None:
    """(archivo, sesion, raiz del repo) del evento de hook, o None si no aplica."""
    ti = data.get("tool_input") or {}
    fp = ti.get("file_path") or ti.get("path") or ""
    if Path(fp).suffix not in _LANGS:
        return None
    path = Path(fp)
    if not path.is_absolute():
        path = Path(data.get("cwd") or ".") / path
    root = _repo_root(path.resolve())
    if root is None:
        return None
    return path, str(data.get("session_id") or data.get("conversation_id") or "sin-sesion"), root


def _pre_text(data: dict) -> str:
    """Informe de impacto, una vez por archivo por sesion."""
    t = _target(data)
    if t is None or _seen(t[1], str(t[0].resolve())):
        return ""
    path, session, root = t
    t0 = time.perf_counter()
    text = report(root, path)
    _log({"ts": time.time(), "session": session, "file": str(path), "root": str(root),
          "lines": text.count("\n") + 1 if text else 0, "chars": len(text),
          "ms": round((time.perf_counter() - t0) * 1000)})
    return text


def cost_report(root: Path, target: Path) -> str:
    """Chequeo de costo despues de escribir (mmorch.impacto_costo, ticket 15). Solo Python."""
    if target.suffix != ".py":
        return ""
    from mmorch import impacto_costo, impacto_indirecto
    root, target = root.resolve(), target.resolve()
    files = [p for p in _source_files(root, (".py",)) if not _is_test(p.relative_to(root).as_posix())]
    F = impacto_indirecto.all_facts(root, files, _cache_path(root, "pycost"), impacto_costo.facts)
    return impacto_costo.report(F, target.relative_to(root).as_posix())


def _post_text(data: dict) -> str:
    """Chequeo de costo, cada vez que cambia el aviso para ese archivo en la sesion."""
    t = _target(data)
    if t is None:
        return ""
    path, session, root = t
    try:
        text = cost_report(root, path)
    except (SyntaxError, UnicodeDecodeError, ValueError, OSError):
        return ""
    d = Path(tempfile.gettempdir()) / "mmorch_impacto"
    d.mkdir(exist_ok=True)
    state = d / f"post_{re.sub(r'[^A-Za-z0-9_-]', '_', session)[:80]}.json"
    try:
        last = json.loads(state.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        last = {}
    key = str(path.resolve())
    # sin numeros de linea: una edicion arriba corre las lineas y el mismo aviso volvia en cada edicion
    same = re.sub(r"linea \d+|:\d+\)", "", text)
    if last.get(key) == same:
        return ""
    last[key] = same
    state.write_text(json.dumps(last), encoding="utf-8")
    return text


def _emit(text: str, event: str, cursor: bool) -> str:
    if not text:
        return ""
    if cursor:
        return json.dumps({"additional_context": text}, ensure_ascii=False)
    return json.dumps({"hookSpecificOutput": {"hookEventName": event, "additionalContext": text}}, ensure_ascii=False)


def hook(raw: str, cursor: bool = False) -> str:
    """Entrada: JSON del hook. Claude Code (PreToolUse): `tool_input.file_path` + `session_id`,
    salida `hookSpecificOutput.additionalContext`, sin permissionDecision (los permisos del
    usuario siguen su curso). Cursor (postToolUse, ticket 10): `tool_input.path` +
    `conversation_id`, salida `additional_context`; como corre despues de editar, suma el
    chequeo de costo (ticket 15). Devuelve "" si no aplica."""
    data = json.loads(raw)
    text = _pre_text(data)
    if cursor:
        text = "\n\n".join(x for x in (text, _post_text(data)) if x)
    return _emit(text, "PreToolUse", cursor)


def post(raw: str) -> str:
    """Claude Code PostToolUse (ticket 15): chequeo de costo del archivo recien escrito."""
    return _emit(_post_text(json.loads(raw)), "PostToolUse", False)


def main(argv: list[str]) -> int:
    if argv[:1] in (["hook"], ["cursor"], ["post"]):
        try:
            raw = sys.stdin.read()
            out = post(raw) if argv[0] == "post" else hook(raw, cursor=argv[0] == "cursor")
            if out:
                print(out)
        except Exception:  # fail-open: un informe roto nunca bloquea la edicion
            pass
        return 0
    if len(argv) == 2:
        print(report(Path(argv[0]), Path(argv[1])) or "(sin informe)")
        return 0
    print("uso: python -m mmorch.impacto hook|cursor|post  |  python -m mmorch.impacto <raiz> <archivo.py>")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
