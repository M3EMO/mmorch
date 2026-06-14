"""autoregister_project — hook SessionStart: registra el cwd actual como proyecto mmorch.
Lee el JSON del hook por stdin ({cwd, ...}); fallback a os.getcwd(). Append a projects.json
SIN importar mmorch (stdlib pura -> corre con cualquier python). Idempotente.

Solo da VISIBILIDAD (el proyecto aparece en el dashboard). Editar sigue siendo accion
explicita per-call (mode='edit') — auto-registrar no auto-habilita escritura remota.
"""
import json, os, sys

ORCH = os.path.join(os.path.expanduser("~"), ".claude", "orchestration")
PROJECTS = os.path.join(ORCH, "projects.json")
# no registrar la propia orchestration ni dirs de sistema
_SKIP = {ORCH.lower()}


def _cwd_from_stdin():
    try:
        data = json.loads(sys.stdin.read() or "{}")
        return data.get("cwd") or data.get("workspace") or ""
    except Exception:
        return ""


def main():
    cwd = _cwd_from_stdin() or os.getcwd()
    cwd = os.path.abspath(cwd)
    if not os.path.isdir(cwd) or cwd.lower() in _SKIP:
        return
    name = os.path.basename(cwd) or cwd
    try:
        data = json.load(open(PROJECTS, encoding="utf-8")) if os.path.exists(PROJECTS) else {}
    except Exception:
        data = {}
    if data.get(name) == cwd:
        return   # idempotente
    data[name] = cwd
    os.makedirs(ORCH, exist_ok=True)
    json.dump(data, open(PROJECTS, "w", encoding="utf-8"), ensure_ascii=False, indent=2)


if __name__ == "__main__":
    main()
