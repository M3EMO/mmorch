"""W5.2 — runner de self-checks: los bloques `__main__` de mmorch/ SI se corren.

El repo tiene ~50 modulos con self-check inline (`python -m mmorch.X` corre asserts);
eran documentacion ejecutable latente porque nadie los ejecutaba en bulk (03 §2.1).
Este runner los descubre por grep del guard y los corre en subprocess con MMORCH_HOME
aislado (tmp) y timeout — cualquier assert roto en un self-check ahora pone la suite
en rojo, en vez de pudrirse en silencio (asi se encontro el drift del fake de evolve).
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]

# Modulos con bloque __main__ que NO son self-check hermetico — cada exclusion con
# su razon (si la razon desaparece, sacar la linea y el runner lo adopta solo):
EXCLUIR = {
    "server": "levanta el HTTP server real y bloquea (main loop, no self-check)",
    "cli": "entrypoint CLI: parsea argv y ejecuta comandos reales",
    "nightly": "pipeline nocturno completo: LLM/API real y estado del home real",
    "plugin_worker": "worker CLI: main() exige argv de job (IndexError sin args)",
    "babel": "asserta lexicon_version() del vault REAL — vacio en el home aislado",
    "minds": "federation() lee projects.json real — cero nodos en el home aislado",
}


def _modulos_con_selfcheck() -> list[str]:
    mods = []
    for p in sorted((REPO / "mmorch").glob("*.py")):
        if 'if __name__ == "__main__"' in p.read_text(encoding="utf-8", errors="replace"):
            mods.append(p.stem)
    return mods


MODULOS = [m for m in _modulos_con_selfcheck() if m not in EXCLUIR]
assert len(MODULOS) >= 40, f"el descubrimiento colapso: {len(MODULOS)} modulos"


@pytest.mark.parametrize("mod", MODULOS)
def test_selfcheck(mod, tmp_path):
    env = dict(os.environ, MMORCH_HOME=str(tmp_path), PYTHONIOENCODING="utf-8")
    # sin API keys: un self-check que intente red debe fallar aca, no gastar plata
    for k in list(env):
        if any(s in k.upper() for s in ("API_KEY", "OPENROUTER", "DEEPSEEK", "GEMINI")):
            env.pop(k)
    r = subprocess.run([sys.executable, "-m", f"mmorch.{mod}"], cwd=REPO, env=env,
                       capture_output=True, text=True, timeout=120)
    assert r.returncode == 0, (
        f"self-check de mmorch/{mod}.py fallo (rc={r.returncode}):\n"
        + "\n".join((r.stdout + r.stderr).strip().splitlines()[-8:]))
