"""Repo de juguete para los tests de SDD fase 2 (deltas por area y propiedades). Cero API: Claude y el writer simulados.

Areas: `calc` = pkg/*.py, `libs` = lib/*.py, `builder` = sdlc/*.py. El worktree de la corrida es el repo mismo.
"""
import pathlib
import subprocess
import types

import sdlc.pipeline as S

TOML = '''accept_cmd = "python -m pytest -q"
approve_accept = false
ext = ["py"]
files = ["pkg/**/*.py", "lib/**/*.py", "sdlc/**/*.py"]

[areas]
calc = ["pkg/*.py"]
libs = ["lib/*.py"]
builder = ["sdlc/*.py"]
'''

A = "def suma(a, b):\n    return a + b\n\n\ndef es_par(n):\n    return n % 2 == 0\n"
C = "def doble(x):\n    return 2 * x\n"


def git(repo, *args) -> str:
    return subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True, text=True).stdout


def commit(repo, msg="base"):
    git(repo, "add", "-A")
    git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--no-verify", "-m", msg)


def escribir(repo, archivos: dict[str, str]):
    for rel, txt in archivos.items():
        p = pathlib.Path(repo) / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(txt, encoding="utf-8")


def repo(tmp_path, toml=TOML, archivos=None):
    r = tmp_path / "repo"
    r.mkdir(parents=True)
    escribir(r, {"sdlc.toml": toml, "pkg/a.py": A, "pkg/c.py": C, "lib/b.py": "B = 1\n", "sdlc/pipeline.py": "P = 1\n",
                 "tests/test_viejo.py": "def test_viejo():\n    assert True\n", **(archivos or {})})
    git(r, "init", "-q")
    commit(r)
    return r


def conf(repo, monkeypatch, *, files=None, task="tarea", name="feat x", accept=None, approve=None):
    if approve is not None:
        t = (repo / "sdlc.toml").read_text(encoding="utf-8")
        (repo / "sdlc.toml").write_text(t.replace("approve_accept = false", f"approve_accept = {str(approve).lower()}"),
                                        encoding="utf-8")
    monkeypatch.setattr(S, "RUNS", repo.parent / "runs")
    t = types.SimpleNamespace(name=name, task=task, accept_files=dict(accept or {}))
    feat = {"repo": str(repo), "task": task, "files": list(files or []), "techo": S._toml(repo)["files"],
            "suite": ["tests", "-q", "-rfE", "-p", "no:cacheprovider"]}
    S.configure(t, contract=[], feat=feat, wt=repo, accept_cmd="python -m pytest -q")
    return t


class Claude:
    """`_revisor` simulado: cada llamada escribe el siguiente dict de archivos de la cola (None = no escribe nada)."""

    def __init__(self, repo, *cola):
        self.repo, self.cola, self.prompts = repo, list(cola), []

    def __call__(self, prompt, **kw):
        self.prompts.append(prompt)
        archivos = self.cola.pop(0) if self.cola else None
        if archivos:
            escribir(self.repo, archivos)
        return {"ok": True, "result": "ok", "returncode": 0, "steps": 1}


class Bd:
    """`bd` simulado: issues en memoria con labels, estado y notas."""

    def __init__(self, falla=False):
        self.issues: list[dict] = []
        self.llamadas: list[tuple[list[str], str]] = []
        self.falla = falla

    def __call__(self, args, cwd):
        import json
        self.llamadas.append((list(args), str(cwd)))
        if self.falla:
            return False, "bd: no anda"
        if args[0] == "list":
            lab, st = args[args.index("--label") + 1], args[args.index("--status") + 1]
            return True, json.dumps([i for i in self.issues if lab in i["labels"] and i["status"] == st])
        if args[0] == "create":
            i = {"id": f"x-{len(self.issues) + 1}", "labels": [args[args.index("--labels") + 1]], "status": "open",
                 "notes": args[args.index("--description") + 1]}
            self.issues.append(i)
            return True, json.dumps(i)
        if args[0] == "update":
            i = next(i for i in self.issues if i["id"] == args[1])
            i["notes"] += "\n" + args[args.index("--append-notes") + 1]
            return True, "ok"
        return False, "?"


def spec_area(reqs: dict[str, str], rutas=("pkg/a.py",), area="calc") -> str:
    """Spec viva de un area con el formato que regenera el codigo (Trazabilidad vacia: la deriva `regenerar`)."""
    return (f"# Spec del area {area}\n\n## Contrato\n\n" + "".join(f"- `{r}`\n" for r in rutas)
            + "\n## Requisitos\n\n" + "".join(f"- {i}: {t}\n" for i, t in reqs.items())
            + "\n## Trazabilidad\n\n| ID | tests |\n|---|---|\n")


def test_de(rid: str, cuerpo="assert True", area="calc", slug="x") -> tuple[str, str]:
    return f"tests/sdlc/{area}/test_{rid}_{slug}.py", f"def test_{rid}_{slug}():\n    {cuerpo}\n"
