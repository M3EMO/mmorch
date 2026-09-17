"""Migra la config de Claude Code (~/.claude) entre PCs con rutas relativas al home.

    python claude_config.py export <carpeta>    # en la PC de origen
    python claude_config.py install <carpeta>   # en la PC nueva, con Claude cerrado

export copia config, extensiones, memoria y servidores MCP a <carpeta>, con el home y la carpeta de Node cambiados por
marcadores ({{HOME.win}}, {{HOME.json}}, ...). install cambia los marcadores por las rutas de ESTA PC, respalda lo que
pisa en ~/.claude/backups/install-<fecha>/ y agrega los MCP a ~/.claude.json. Nunca copia credenciales, historial,
sesiones ni transcripciones. Los valores de variables con KEY/TOKEN/SECRET/PASSWORD salen como {{SECRETO}}.
La carpeta lleva memoria y config personal: guardala en un lugar privado, nunca en un repo publico. Solo stdlib.
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import time
from pathlib import Path

ARCHIVOS = ["CLAUDE.md", "settings.json", "never-edit.txt", "statusline-merged.ps1", "package.json", "keybindings.json"]
CARPETAS = ["hooks", "skills", "commands", "agents", "scripts"]
IGNORAR = {"__pycache__", "node_modules", ".git"}
SECRETO = re.compile(r"key|token|secret|password", re.I)
NODE_DEFAULT = r"C:\Program Files\nodejs"


def _formas(base: str) -> dict[str, str]:
    """Las grafias de una ruta de Windows que aparecen en la config: JSON escapado, Windows, barras, bash y la clave
    con la que Claude Code nombra la carpeta de un proyecto (C:\\Users\\ana\\.claude -> C--Users-ana--claude)."""
    win = str(Path(base))
    fwd = win.replace("\\", "/")
    posix = "/" + fwd[0].lower() + fwd[2:] if fwd[1:2] == ":" else fwd
    return {"json": win.replace("\\", "\\\\"), "win": win, "fwd": fwd, "posix": posix, "key": re.sub(r"[:\\/.]", "-", win)}


def _marcas(home: str, node_dir: str | None) -> dict[str, str]:
    bases = {"HOME": home, "NODE": node_dir or NODE_DEFAULT}
    return {f"{{{{{n}.{k}}}}}": v for n, base in bases.items() for k, v in _formas(base).items()}


def _a_marcas(texto: str, marcas: dict[str, str]) -> str:
    for marca, literal in sorted(marcas.items(), key=lambda kv: -len(kv[1])):  # la grafia mas larga primero
        texto = re.sub(re.escape(literal), marca, texto, flags=re.I)
    return texto


def _de_marcas(texto: str, marcas: dict[str, str]) -> str:
    for marca, literal in marcas.items():
        texto = texto.replace(marca, literal)
    return texto


def _items(raiz: Path):
    for nombre in ARCHIVOS:
        if (raiz / nombre).is_file():
            yield raiz / nombre
    for carpeta in CARPETAS:
        if (raiz / carpeta).is_dir():
            yield from (p for p in sorted((raiz / carpeta).rglob("*"))
                        if p.is_file() and not IGNORAR & set(p.relative_to(raiz).parts))
    yield from (p for p in sorted(raiz.glob("projects/*/memory/*")) if p.is_file())


def _convertir(datos: bytes, fn) -> bytes:
    try:
        return fn(datos.decode("utf-8")).encode("utf-8")
    except UnicodeDecodeError:
        return datos  # binario: se copia tal cual


def _sin_secretos(servidores: dict) -> dict:
    return {n: {**s, "env": {k: ("{{SECRETO}}" if SECRETO.search(k) else v) for k, v in (s.get("env") or {}).items()}}
            if s.get("env") else s for n, s in servidores.items()}


def exportar(raiz: Path, carpeta: Path, home: str, node_dir: str | None, claude_json: Path) -> int:
    marcas = _marcas(home, node_dir) if node_dir else {k: v for k, v in _marcas(home, None).items() if "HOME" in k}
    n = 0
    for p in _items(raiz):
        destino = carpeta / "claude" / _a_marcas(p.relative_to(raiz).as_posix(), marcas)
        destino.parent.mkdir(parents=True, exist_ok=True)
        destino.write_bytes(_convertir(p.read_bytes(), lambda t: _a_marcas(t, marcas)))
        n += 1
    cfg = json.loads(claude_json.read_text(encoding="utf-8")) if claude_json.exists() else {}
    mcp = {"user": _sin_secretos(cfg.get("mcpServers") or {}),
           "local": {k: _sin_secretos(v["mcpServers"]) for k, v in (cfg.get("projects") or {}).items() if v.get("mcpServers")}}
    (carpeta / "mcp.json").write_text(_a_marcas(json.dumps(mcp, indent=2, ensure_ascii=False), marcas), encoding="utf-8")
    return n


def instalar(carpeta: Path, raiz: Path, home: str, node_dir: str | None, claude_json: Path) -> list[str]:
    marcas = _marcas(home, node_dir)
    respaldo = raiz / "backups" / f"install-{time.strftime('%Y%m%d-%H%M%S')}"
    origen = carpeta / "claude"
    for p in sorted(origen.rglob("*")):
        if not p.is_file():
            continue
        rel = _de_marcas(p.relative_to(origen).as_posix(), marcas)
        final, nuevo = raiz / rel, _convertir(p.read_bytes(), lambda t: _de_marcas(t, marcas))
        if final.exists():
            if final.read_bytes() == nuevo:
                continue
            (respaldo / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(final, respaldo / rel)
        final.parent.mkdir(parents=True, exist_ok=True)
        final.write_bytes(nuevo)
    pendientes = []
    if (carpeta / "mcp.json").exists():
        texto = _de_marcas((carpeta / "mcp.json").read_text(encoding="utf-8"), marcas)
        mcp = json.loads(texto)
        cfg = json.loads(claude_json.read_text(encoding="utf-8")) if claude_json.exists() else {}
        if claude_json.exists():
            respaldo.mkdir(parents=True, exist_ok=True)
            shutil.copy2(claude_json, respaldo / ".claude.json")
        cfg.setdefault("mcpServers", {}).update(mcp["user"])
        for proyecto, servidores in mcp["local"].items():
            cfg.setdefault("projects", {}).setdefault(proyecto, {}).setdefault("mcpServers", {}).update(servidores)
        claude_json.write_text(json.dumps(cfg, indent=2, ensure_ascii=False), encoding="utf-8")
        if "{{SECRETO}}" in texto:
            pendientes.append(f"{claude_json}: completar los valores {{{{SECRETO}}}} de los MCP")
    if not node_dir:
        pendientes.append(f"Node no esta en el PATH: los hooks apuntan a {NODE_DEFAULT}")
    orch = raiz / "orchestration"
    if not (orch / ".venv").is_dir():
        pendientes.append(f"clonar mmorch en {orch} y crear su .venv (los hooks de Python y el MCP mmorch la usan)")
    if not (orch / ".env").is_file():
        pendientes.append(f"copiar a mano {orch / '.env'} (API keys) por un canal seguro")
    return pendientes


def main(argv: list[str] | None = None) -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("accion", choices=["export", "install"])
    ap.add_argument("carpeta", type=Path)
    a = ap.parse_args(argv)
    home, node = Path.home(), shutil.which("node")
    node_dir = str(Path(node).parent) if node else None
    if a.accion == "export":
        n = exportar(home / ".claude", a.carpeta, str(home), node_dir, home / ".claude.json")
        print(f"{n} archivos + mcp.json en {a.carpeta}. Contiene memoria personal: guardala privada.")
    else:
        for p in instalar(a.carpeta, home / ".claude", str(home), node_dir, home / ".claude.json"):
            print("PENDIENTE:", p)
        print("Listo. Abri Claude Code e inicia sesion.")


if __name__ == "__main__":
    main()
