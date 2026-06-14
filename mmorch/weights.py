"""weights — gestion de pesos de nodos neuronales (model-cards + verificacion). Source of
truth = weights/manifest.json. Cada peso tiene sha256: load_weights() lo VERIFICA al resolver
(detecta corrupcion/tamper antes de inferir — ethos red-zone). card() expone la metadata.

Diseno: float32 .npz pa inferencia (sin torch en el core); el peso es un CACHE regenerable
(regen_cmd en el manifest). Promover una version nueva pasa por el gate (debe batir metric).
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "weights" / "manifest.json"


def _manifest(path: Path | None = None) -> dict:
    p = Path(path or MANIFEST)
    if not p.exists():
        return {}
    try:
        d = json.loads(p.read_text(encoding="utf-8"))
        return {k: v for k, v in d.items() if not k.startswith("_")}
    except Exception:
        return {}


def card(name: str, *, path: Path | None = None) -> dict | None:
    """Model-card de un peso (arch, metric, sha256, regen_cmd...). None si no existe."""
    return _manifest(path).get(name)


def list_weights(*, path: Path | None = None) -> list[str]:
    return sorted(_manifest(path))


def sha256_of(file_path: str | Path) -> str:
    h = hashlib.sha256()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def verify(name: str, *, path: Path | None = None) -> tuple[bool, str]:
    """Chequea que el archivo del peso exista y su sha256 matchee el manifest.
    (ok, detalle). ok=False si falta, no matchea, o el card no tiene sha."""
    c = card(name, path=path)
    if not c:
        return False, f"peso '{name}' no esta en el manifest"
    fp = ROOT / c.get("path", "")
    if not fp.exists():
        return False, f"archivo ausente: {fp} (regen: {c.get('regen_cmd', 'n/a')})"
    want = c.get("sha256", "")
    if not want:
        return True, "sin sha256 en el card (no verificable)"
    got = sha256_of(fp)
    if got != want:
        return False, f"sha256 NO matchea (got {got[:12]} != {want[:12]}) — corrupto/tamper"
    return True, "ok"


def resolve(name: str, *, verify_hash: bool = True, path: Path | None = None) -> str:
    """Path absoluto del archivo del peso. Si verify_hash y el sha no matchea -> ValueError
    (no se infiere con un peso corrupto/manipulado)."""
    c = card(name, path=path)
    if not c:
        raise KeyError(f"peso '{name}' no registrado en el manifest")
    if verify_hash:
        ok, detail = verify(name, path=path)
        if not ok:
            raise ValueError(f"verificacion de '{name}' fallo: {detail}")
    return str(ROOT / c["path"])
