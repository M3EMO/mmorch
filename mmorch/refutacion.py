"""refutacion — banco congelado que mide la skill "refutar tests" (tickets 14 y 16 del mapa sdlc-6-gates).

Un caso = un test de aceptacion aprobado al que le faltaba un requisito, mas dos versiones del codigo en git: la version
con el defecto que ese hueco dejo pasar y la version corregida (un caso sin defecto conocido solo trae la corregida y
mide falsas alarmas). `Banco.evaluar(caso, test)` escribe el test propuesto en worktrees de esas versiones, lo corre con
el comando del caso y lo clasifica sin juez LLM:

    invalido      no compila o no se recolecta en alguna version
    falsa_alarma  falla con la version corregida
    acierto       falla con el defecto y pasa con la corregida
    neutro        pasa en todas (no atrapa nada, no molesta)

La definicion del banco NO va al repo (mmorch es publico y los casos apuntan a repos privados): vive en
`logs/refutacion/banco.json`. Los worktrees se abren una vez por version y se cierran al salir del `with`.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import time
import tempfile
import uuid
from pathlib import Path

from .paths import logs_dir
from .projects import resolve
from .worktree_driver import Worktree, _git


SKILL = Path(__file__).with_name("sdlc_templates") / "refutar-tests.md"
_BLOQUE = re.compile(r"```[\w.]*\n(.*?)```", re.S)


def banco_path() -> Path:
    return logs_dir() / "refutacion" / "banco.json"


def _git_show(repo: str, ref: str, rel: str) -> str:
    r = subprocess.run(["git", "-C", repo, "show", f"{ref}:{rel}"], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return r.stdout if r.returncode == 0 else ""


def prompt_de(caso: dict) -> str:
    """El pedido que reciben los dos runtimes: la skill comun + tarea, ficha, formato y el test aprobado del caso."""
    repo = resolve(caso["proyecto"])
    return "\n\n".join([
        SKILL.read_text(encoding="utf-8"),
        f"## Formato del caso\n\n{caso['formato']}\nEl comando que corre los tests es: `{caso['cmd']}`",
        f"## Tarea\n\n{caso['tarea']}",
        f"## Ficha\n\n{_git_show(repo, caso['test_ref'], caso['ficha_rel'])}",
        f"## Test de aceptacion aprobado (el que tenes que refutar)\n\n"
        f"{_git_show(repo, caso['test_ref'], caso['test_rel'])}",
    ])


def tests_propuestos(respuesta: str, max_n: int = 3) -> list[str]:
    """Los bloques de codigo de la respuesta, en orden. Sin bloques no hay propuesta (la skill lo permite)."""
    return [b.strip() + "\n" for b in _BLOQUE.findall(respuesta)][:max_n]


def cargar_banco(path: Path | None = None) -> dict[str, dict]:
    return {c["id"]: c for c in json.loads(Path(path or banco_path()).read_text(encoding="utf-8"))}


HERMES = Path.home() / "Documents" / "Hermes" / "hermes-agent" / ".venv" / "Scripts" / "hermes-agent.exe"


def refutar(caso: dict, *, runtime: str = "mmorch", modelo: str = "deepseek-reasoner", temperature: float = 0.7,
            timeout: float = 900.0, max_n: int = 3) -> dict:
    """Corre la skill comun en un runtime y devuelve {tests, texto, segundos, usd}. `mmorch` llama al modelo directo;
    `hermes` usa el one-shot de Hermes Agent (mismo modelo: lo que se compara es el agente, no el modelo)."""
    prompt = prompt_de(caso)
    t0 = time.time()
    if runtime == "mmorch":
        from .providers import call
        r = call(modelo, prompt, pattern="sdlc-refutacion", node=modelo, phase="refutacion",
                 temperature=temperature, max_tokens=64000, timeout=timeout)
        texto, usd = r.text, r.cost_usd
    elif runtime == "hermes":
        if not HERMES.exists():
            raise RuntimeError(f"Hermes Agent no esta instalado en {HERMES}")
        p = subprocess.run([str(HERMES), "-z", "-", "--model", modelo, "--provider", "deepseek",
                            "-p", "sdlc-refutador", "-Q"], input=prompt, capture_output=True, text=True,
                           encoding="utf-8", errors="replace", timeout=timeout)
        texto, usd = p.stdout + p.stderr, 0.0  # el gasto de Hermes queda en su propio log
    else:
        raise ValueError(f"runtime desconocido: {runtime}")
    return {"tests": tests_propuestos(texto, max_n), "texto": texto, "segundos": round(time.time() - t0, 1), "usd": usd}


def _python_del_repo(repo: str) -> str:
    py = Path(repo) / ".venv" / "Scripts" / "python.exe"
    return str(py) if py.exists() else sys.executable


class Banco:
    """Contexto que materializa las versiones de los casos a medida que se evaluan y las borra al salir."""

    def __init__(self, casos: dict[str, dict]):
        self.casos = casos
        self._wts: dict[tuple[str, str], Worktree] = {}

    def __enter__(self) -> Banco:
        return self

    def __exit__(self, *_) -> None:
        for wt in self._wts.values():
            wt.close(keep_branch=True)  # desengancha los sembrados antes de borrar; sin rama: worktree detached
        self._wts.clear()

    def _worktree(self, caso: dict, ref: str) -> Worktree:
        clave = (caso["id"], ref)
        if clave not in self._wts:
            repo = resolve(caso["proyecto"])
            # ruta corta: el scratchpad supera MAX_PATH con repos de PDFs; sparse para no copiar datos del repo
            path = str(Path(tempfile.gettempdir()) / f"rb-{uuid.uuid4().hex[:8]}")
            rc, out = _git(repo, "-c", "core.longpaths=true", "worktree", "add", "--detach", "--no-checkout", path, ref)
            if rc != 0:
                raise RuntimeError(f"worktree de {caso['id']}@{ref}: {out}")
            wt = Worktree(repo, path, "")
            self._wts[clave] = wt
            if caso.get("sparse"):
                _git(path, "sparse-checkout", "set", *caso["sparse"])
            rc, out = _git(path, "-c", "core.longpaths=true", "read-tree", "-mu", "HEAD")  # puebla respetando sparse
            if rc != 0:
                raise RuntimeError(f"checkout de {caso['id']}@{ref}: {out}")
            wt.seed(caso.get("seed") or [])
        return self._wts[clave]

    def _correr(self, caso: dict, ref: str, test: str) -> tuple[bool, bool, str]:
        """(valido, verde, salida) del test propuesto en la version `ref`."""
        wt = self._worktree(caso, ref)
        prueba = Path(wt.path) / caso["prueba_rel"]
        prueba.parent.mkdir(parents=True, exist_ok=True)
        prueba.write_text(test, encoding="utf-8")
        cmd = caso["cmd"].format(python=_python_del_repo(wt.repo), prueba=caso["prueba_rel"])
        try:
            r = subprocess.run(cmd, shell=True, cwd=wt.path, capture_output=True, text=True, encoding="utf-8",
                               errors="replace", timeout=caso.get("timeout_s", 600))
            salida, rc = r.stdout + r.stderr, r.returncode
        except subprocess.TimeoutExpired:
            salida, rc = "TIMEOUT", 1
        finally:
            prueba.unlink(missing_ok=True)
        return not re.search(caso["invalido"], salida), rc == 0, salida[-3000:]

    def evaluar(self, caso_id: str, test: str) -> dict:
        caso = self.casos[caso_id]
        vc, verde_c, sal_c = self._correr(caso, caso["corregida"], test)
        vd, verde_d, sal_d = self._correr(caso, caso["defecto"], test) if caso.get("defecto") else (True, True, "")
        if not (vc and vd):
            clase = "invalido"
        elif not verde_c:
            clase = "falsa_alarma"
        elif not verde_d:
            clase = "acierto"
        else:
            clase = "neutro"
        return {"caso": caso_id, "clase": clase, "salida_corregida": sal_c, "salida_defecto": sal_d}
