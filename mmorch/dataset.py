"""dataset — construye un dataset de CALIDAD DE CÓDIGO desde git history, SIN labels
humanos (JIT-defect / just-in-time defect prediction):

  commit de FIX → la función ANTES del fix = tenía el bug (label 0 = malo)
                  la misma función DESPUÉS = corregida (label 1 = bueno)

Señal de label gratis: el propio acto de "arreglar" un commit etiqueta el código previo
como defectuoso. Esto alimenta factory.train_code_quality. Es la rebanada realizable del
"aprende a diferenciar buen código de mal código": minar GitHub = minar git history.

Library-only, determinista (sin API). Funciona sobre cualquier repo git local/clonado.
"""
from __future__ import annotations

import ast
import re
import subprocess
from pathlib import Path

_FIX_RE = re.compile(r"\b(fix|fixes|fixed|bug|bugfix|patch|regression|broke|broken|"
                     r"crash|error|incorrect|wrong|defect|hotfix)\b", re.I)


def _git(repo: Path, *args, timeout: int = 60) -> str:
    p = subprocess.run(["git", *args], cwd=str(repo), capture_output=True, text=True,
                       timeout=timeout, errors="replace")
    return p.stdout


def fix_commits(repo: Path, max_n: int = 80) -> list[str]:
    """SHAs de commits cuyo mensaje indica un fix (excluye merges)."""
    out = _git(repo, "log", "--no-merges", "--pretty=%H|%s", "-n", "4000")
    shas = []
    for ln in out.splitlines():
        if "|" not in ln:
            continue
        sha, msg = ln.split("|", 1)
        if _FIX_RE.search(msg):
            shas.append(sha)
        if len(shas) >= max_n:
            break
    return shas


def _changed_py_files(repo: Path, sha: str) -> list[str]:
    out = _git(repo, "show", "--name-only", "--pretty=format:", sha)
    return [f for f in out.splitlines() if f.strip().endswith(".py")]


def _changed_lines(repo: Path, sha: str, path: str) -> set[int]:
    """Líneas (en la versión NUEVA) tocadas por el commit, del diff unificado."""
    diff = _git(repo, "show", sha, "--unified=0", "--", path)
    lines, newln = set(), 0
    for ln in diff.splitlines():
        m = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@", ln)
        if m:
            newln = int(m.group(1))
            continue
        if ln.startswith("+") and not ln.startswith("+++"):
            lines.add(newln); newln += 1
        elif not ln.startswith("-"):
            newln += 1
    return lines


def _file_at(repo: Path, sha: str, path: str) -> str:
    return _git(repo, "show", f"{sha}:{path}")


def _functions_covering(source: str, lines: set[int]) -> list[str]:
    """Funciones cuyo span [lineno, end_lineno] cubre alguna línea cambiada. Devuelve el
    código fuente de cada función (parseable, no hunks sueltos)."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return []
    src_lines = source.splitlines()
    out = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            a, b = node.lineno, getattr(node, "end_lineno", node.lineno)
            if lines and not any(a <= l <= b for l in lines):
                continue
            seg = "\n".join(src_lines[a - 1:b])
            if 2 <= len(seg.splitlines()) <= 120:   # funciones razonables
                out.append(seg)
    return out


def build_dataset(repo: Path, *, max_commits: int = 80, max_samples: int = 600) -> list[tuple[str, int]]:
    """[(code, label)] — función buggy (0) y su versión fixed (1) por cada fix-commit.
    Dedup por contenido. Balanceado por construcción (0 y 1 por par)."""
    repo = Path(repo)
    seen, data = set(), []
    for sha in fix_commits(repo, max_commits):
        for path in _changed_py_files(repo, sha):
            changed = _changed_lines(repo, sha, path)
            if not changed:
                continue
            before = _functions_covering(_file_at(repo, f"{sha}~1", path), changed)
            after = _functions_covering(_file_at(repo, sha, path), changed)
            for code, label in [(c, 0) for c in before] + [(c, 1) for c in after]:
                h = hash(code)
                if h in seen:
                    continue
                seen.add(h)
                data.append((code, label))
                if len(data) >= max_samples:
                    return data
    return data
