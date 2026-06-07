"""evolve — subset DGM-inspirado, GATED (research: vault/research/
darwin-godel-machine-self-improving-agents). La critica cross-family marco que el
DGM completo (evolucion poblacional open-ended + auto-modificacion) es overreach
para mmorch. Aca solo el subset seguro:

- fitness(): corre el test suite (gate empirico) y devuelve pass-rate. Es la
  "performance empirica" del DGM, pero usando los tests propios como benchmark.
- archive: registro append-only de intentos de evolucion + su fitness (la
  "poblacion/archivo" del DGM, sin la evolucion automatica).
- propose_patch(): un modelo barato PROPONE un cambio (read-only, NO lo aplica).

NUNCA auto-modifica vivo. Aplicar un patch = sandbox + fitness verde + gate humano.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_ARCHIVE = ROOT / "logs" / "evolution_archive.jsonl"


def fitness(test_path: str = "tests", timeout: int = 300) -> dict:
    """Corre pytest y devuelve {passed, failed, total, pass_rate, ok}. Gate empirico."""
    proc = subprocess.run(
        [sys.executable, "-m", "pytest", test_path, "-q", "--no-header"],
        cwd=str(ROOT), capture_output=True, text=True, timeout=timeout)
    out = (proc.stdout or "") + (proc.stderr or "")
    passed = _count(out, r"(\d+) passed")
    failed = _count(out, r"(\d+) failed")
    total = passed + failed
    return {
        "passed": passed, "failed": failed, "total": total,
        "pass_rate": round(passed / total, 4) if total else 0.0,
        "ok": proc.returncode == 0 and failed == 0 and passed > 0,
    }


def _count(text: str, pat: str) -> int:
    m = re.search(pat, text)
    return int(m.group(1)) if m else 0


def archive_variant(name: str, fit: dict, notes: str = "", applied: bool = False) -> None:
    """Registra un intento de evolucion + su fitness (append-only)."""
    rec = {"ts": time.time(),
           "iso": time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime()),
           "name": name, "fitness": fit, "applied": applied, "notes": notes}
    _ARCHIVE.parent.mkdir(parents=True, exist_ok=True)
    with open(_ARCHIVE, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(rec, ensure_ascii=False) + "\n")


def read_archive() -> list[dict]:
    if not _ARCHIVE.exists():
        return []
    return [json.loads(ln) for ln in _ARCHIVE.read_text(encoding="utf-8").splitlines() if ln.strip()]


def propose_patch(target_file: str, finding: str, *, gen_model: str | None = None) -> str:
    """Un modelo barato PROPONE el contenido nuevo de target_file para resolver
    `finding`. READ-ONLY: devuelve el texto, NO escribe nada. Aplicar = gate aparte.
    """
    from .patterns import fan_out
    from .config import DEFAULT_GENERATOR
    src = (ROOT / target_file).read_text(encoding="utf-8") if (ROOT / target_file).exists() else ""
    prompt = (
        f"Sos un mejorador de codigo Python. Resolve este hallazgo SIN romper la API publica "
        f"ni los invariantes (cross-family, OneFlow, anti-sicofancia, observabilidad).\n\n"
        f"HALLAZGO: {finding}\n\nARCHIVO {target_file}:\n{src}\n\n"
        f"Devolve el CONTENIDO COMPLETO nuevo del archivo, sin explicacion, en un bloque de codigo.")
    return fan_out([prompt], gen_model=gen_model or DEFAULT_GENERATOR, phase="evolve")[0].text
