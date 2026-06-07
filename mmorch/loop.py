"""loop_until_done — scope DESCONOCIDO, 'segui hasta que este limpio'. Control-flow
puro (sin API propia): corre un step() repetido, deduplica contra TODO lo visto, y
para cuando `patience` rondas seguidas no traen nada nuevo (loop-until-DRY), o el step
senaliza done (devuelve None), o se llega a max_rounds.

Por que loop-until-dry y no 'while count < N': un contador fijo corta la cola (los
ultimos hallazgos raros aparecen tarde). Dry-streak espera a que la veta se agote.
Dedup contra `seen`, NUNCA contra una lista filtrada aguas abajo (si no, lo rechazado
reaparece cada ronda y no converge).
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable


@dataclass
class LoopResult:
    items: list                       # todos los unicos acumulados
    rounds: int
    stopped: str                      # dry | explicit_done | max_rounds
    new_per_round: list[int] = field(default_factory=list)


def loop_until_done(
    step: Callable[[int], list | None],
    *,
    key: Callable = lambda x: x,
    max_rounds: int = 10,
    patience: int = 2,
) -> LoopResult:
    """step(round:int) -> lista de items de esa ronda, o None para senalar 'done'.

    key(item) define identidad para dedup. Para cuando:
      - `patience` rondas consecutivas con 0 items nuevos (dry), o
      - step devuelve None (done explicito), o
      - se alcanza max_rounds.
    """
    seen: set = set()
    items: list = []
    new_per_round: list[int] = []
    dry = 0
    for r in range(1, max_rounds + 1):
        batch = step(r)
        if batch is None:
            return LoopResult(items, r, "explicit_done", new_per_round)
        fresh = []
        for x in batch:
            k = key(x)
            if k not in seen:
                seen.add(k)
                items.append(x)
                fresh.append(x)
        new_per_round.append(len(fresh))
        if not fresh:
            dry += 1
            if dry >= patience:
                return LoopResult(items, r, "dry", new_per_round)
        else:
            dry = 0
    return LoopResult(items, max_rounds, "max_rounds", new_per_round)
