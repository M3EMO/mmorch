"""Kill-switch global de los loops autonomos: un solo archivo, un solo dueño.

`logs/loop_paused` presente = todo loop autonomo se salta. Lo crea un halt
(auto-apply) o el humano a mano; solo el humano lo borra.
"""

from __future__ import annotations

from pathlib import Path

FLAG = "loop_paused"


def paused(logs: str | Path) -> bool:
    return (Path(logs) / FLAG).exists()


def pause(logs: str | Path) -> None:
    Path(logs).mkdir(parents=True, exist_ok=True)
    (Path(logs) / FLAG).touch()
