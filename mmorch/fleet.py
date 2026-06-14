"""fleet — control unificado de varios hosts mmorch en el tailnet. Cada maquina corre su
server; el fleet los lista, agrega su /state, y FORWARDEA jobs al host elegido (server->server
por el tailnet, evita CORS del browser y centraliza el token).

Registro en hosts.json (gitignored: tiene IPs/tokens). Un host = {url, token}. 'local' es
implicito (este server). forward() postea a {url}{path} con el token de ese host.
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOSTS_PATH = ROOT / "hosts.json"


def _load(path: Path | None = None) -> dict:
    p = Path(path or HOSTS_PATH)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _save(data: dict, path: Path | None = None) -> None:
    Path(path or HOSTS_PATH).write_text(json.dumps(data, ensure_ascii=False, indent=2),
                                        encoding="utf-8")


def register_host(name: str, url: str, token: str = "", *, store: Path | None = None) -> dict:
    """url ej 'http://100.88.0.57:8787'. token = el MMORCH_SERVER_TOKEN de ESE host."""
    data = _load(store)
    data[name] = {"url": url.rstrip("/"), "token": token}
    _save(data, store)
    return {"name": name, "url": data[name]["url"]}


def list_hosts(*, store: Path | None = None) -> dict:
    return _load(store)


def _get(url: str, token: str, timeout: float = 6.0):
    import httpx
    r = httpx.get(url, params={"token": token} if token else None, timeout=timeout)
    return r.status_code, (r.json() if r.headers.get("content-type", "").startswith("application/json") else {})


def fleet_state(*, store: Path | None = None) -> dict:
    """Agrega /state de cada host (server->server, tailnet). Host caido -> {error}."""
    out = {}
    for name, h in _load(store).items():
        try:
            code, body = _get(h["url"] + "/state", h.get("token", ""))
            out[name] = {"url": h["url"], "ok": code == 200,
                         "summary": body.get("summary"), "jobs": body.get("jobs"),
                         "budget": body.get("budget")}
        except Exception as e:
            out[name] = {"url": h["url"], "ok": False, "error": str(e)[:120]}
    return {"hosts": out}


def forward(name: str, path: str, payload: dict, *, store: Path | None = None) -> dict:
    """POST {host.url}{path} con el token del host. Para disparar un job en otra maquina."""
    import httpx
    h = _load(store).get(name)
    if not h:
        return {"ok": False, "error": f"host '{name}' no registrado"}
    try:
        r = httpx.post(h["url"] + path, json=payload,
                       headers={"X-Token": h.get("token", "")}, timeout=15.0)
        return {"ok": r.status_code == 200, "status": r.status_code,
                "body": r.json() if r.content else {}}
    except Exception as e:
        return {"ok": False, "error": str(e)[:160]}
