"""Rutas de ESTADO del sistema (logs, DBs, bandits, memoria, cache).

Unico modulo autorizado a anclar `Path(__file__).parents[1]`: todo el estado
resuelve via env MMORCH_HOME para poder instalar el paquete como wheel sin
escribir dentro de site-packages y para correr instancias aisladas (tests,
segunda instancia). Default = el checkout actual, para no romper el layout
existente. Rutas de CODIGO del paquete (prompts/, roles/, workflows/) NO
pasan por aca: viajan con el codigo, no con el estado.

Los modulos consumidores anclan sus paths a import-time, asi que MMORCH_HOME
debe estar seteado ANTES de importar mmorch (igual que las API keys de .env).
"""

from __future__ import annotations

import os
from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[1]


def home() -> Path:
    """Raiz del estado: env MMORCH_HOME o el checkout actual."""
    env = os.getenv("MMORCH_HOME")
    return Path(env).resolve() if env else _REPO_ROOT


def data_dir() -> Path:
    """Estado suelto en la raiz del home (prices.json, hosts.json, DBs)."""
    d = home()
    d.mkdir(parents=True, exist_ok=True)
    return d


def logs_dir() -> Path:
    """Logs, JSONL append-only y estado aprendido (bandits, memoria)."""
    d = home() / "logs"
    d.mkdir(parents=True, exist_ok=True)
    return d


def db_path(name: str) -> Path:
    """Path de una DB por nombre (chat.db, workflow.db) bajo data_dir()."""
    return data_dir() / name
