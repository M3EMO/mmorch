"""impacto_externo — lectores fuera del repo en prompts y configuracion de agentes (ticket 13).

Toma los nombres de archivo de datos que escribe un modulo Python (`nightly.jsonl`) y los busca
en tareas programadas, skills, hooks, comandos, agentes y settings de Claude Code y Cursor.
Medido (ticket 13, deepseek-v4-pro): con lector en un prompt o hook, el acierto pasa de 0/12 a
12/12; sobre 148 modulos reales de orchestration, 18 de 20 lineas apuntan a lectores reales.

El indice de la configuracion (archivo -> nombre de archivo de datos -> primera linea) tiene
cache por (mtime_ns, size). Mismo texto que el detector medido (orch-spike, commit 4494eb0).
"""
from __future__ import annotations

import ast
import json
import re
from pathlib import Path

MAX_LINES = 10
_DATA_FILE = re.compile(r"[A-Za-z0-9_.-]+\.(?:jsonl|json|db|sqlite|sqlite3|csv|tsv|log|parquet|ya?ml|txt|properties)\b")
_TEXT = (".md", ".json", ".js", ".ts", ".ps1", ".sh", ".toml", ".yaml", ".yml", ".txt")
_SKIP = {".git", "node_modules", ".venv", "venv", "__pycache__", "projects", "file-history", "cache",
         "plugins", "shell-snapshots", "session-env", "sessions", "paste-cache", "telemetry"}


def config_roots(home: Path | None = None) -> list[tuple[str, Path]]:
    """(como mostrarlo, carpeta o archivo) de la configuracion de agentes del usuario."""
    h = home or Path.home()
    return [("~/.claude/scheduled-tasks", h / ".claude/scheduled-tasks"), ("~/.claude/skills", h / ".claude/skills"),
            ("~/.claude/hooks", h / ".claude/hooks"), ("~/.claude/commands", h / ".claude/commands"),
            ("~/.claude/agents", h / ".claude/agents"), ("~/.claude/settings.json", h / ".claude/settings.json"),
            ("~/.cursor/rules", h / ".cursor/rules"), ("~/.cursor/hooks.json", h / ".cursor/hooks.json")]


def data_files(src: str) -> dict[str, int]:
    """Nombres de archivo de datos en literales de un modulo Python (sin docstrings) -> primera linea."""
    tree = ast.parse(src)
    docs = {id(n.body[0].value) for n in ast.walk(tree)
            if isinstance(n, (ast.Module, ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and n.body
            and isinstance(n.body[0], ast.Expr) and isinstance(n.body[0].value, ast.Constant)}
    out: dict[str, int] = {}
    for n in ast.walk(tree):
        if isinstance(n, ast.Constant) and isinstance(n.value, str) and id(n) not in docs:
            for m in _DATA_FILE.findall(n.value):
                out.setdefault(Path(m).name, n.lineno)
    return out


def _mentions(text: str) -> dict[str, int]:
    """Nombre de archivo de datos -> primera linea donde lo nombra un archivo de configuracion."""
    out: dict[str, int] = {}
    for i, ln in enumerate(text.splitlines(), 1):
        for m in _DATA_FILE.findall(ln):
            out.setdefault(m, i)
    return out


def _text_files(base: Path) -> list[Path]:
    if base.is_file():
        return [base]
    if not base.is_dir():
        return []
    return sorted(p for p in base.rglob("*") if p.is_file() and p.suffix in _TEXT
                  and not any(x in _SKIP for x in p.relative_to(base).parts[:-1]))


def index(roots: list[tuple[str, Path]], cache_path: Path) -> list[tuple[str, dict[str, int]]]:
    """[(como mostrar el archivo, menciones)] en el orden de `roots`, con cache por archivo."""
    try:
        cache = json.loads(cache_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        cache = {}
    out, fresh = [], {}
    for label, base in roots:
        for f in _text_files(base):
            try:
                st = f.stat()
                key = [st.st_mtime_ns, st.st_size]
                hit = cache.get(str(f))
                m = hit[1] if hit and hit[0] == key else _mentions(f.read_text(encoding="utf-8", errors="ignore"))
            except OSError:
                continue
            fresh[str(f)] = [key, m]
            out.append((label if base.is_file() else f"{label}/{f.relative_to(base).as_posix()}", m))
    try:
        cache_path.write_text(json.dumps(fresh), encoding="utf-8")
    except OSError:
        pass
    return out


def report(src: str, writer: str, idx: list[tuple[str, dict[str, int]]]) -> str:
    """Informe de lectores en configuracion de agentes para el modulo `writer` (codigo `src`)."""
    body = []
    for n, line in sorted(data_files(src).items(), key=lambda kv: kv[1]):
        hits = [f"{shown}:{m[n]}" for shown, m in idx if n in m]
        if hits:
            body.append(f"- `{n}` (linea {line}) lo lee tambien: " + ", ".join(hits[:5])
                        + (f" y {len(hits) - 5} mas" if len(hits) > 5 else ""))
    if not body:
        return ""
    return (f"LECTORES FUERA DEL REPO (automatico): archivos de datos que escribe {writer} y que "
            "nombran prompts o configuracion de agentes (tareas programadas, skills, hooks). Si "
            "cambias su formato, nombre o campos, actualiza esos archivos:\n" + "\n".join(body[:MAX_LINES]))
