"""nudge — robo de Hermes 'periodic memory nudging': cada N loops cerrados, dispara
mantenimiento de memoria automatico (consolidar duplicados + reportar destilado pendiente)
en vez de esperar a que el humano corra consolidate a mano.

mmorch es una LIB, no un agente con event-loop propio: el 'nudge' es una funcion que los
loops llaman al cerrar (rubric_loop/code_loop ya lo hacen). Contador persistido; cuando
toca multiplo de `every`, corre la consolidacion (deterministica, cero API) y devuelve un
reporte. Graceful: si algo falla, no rompe el loop que lo llamo.
"""
from __future__ import annotations

import logging
import pathlib

from .iohelpers import atomic_write_json, load_json_tolerant

_log = logging.getLogger(__name__)

from .paths import logs_dir

_STATE = logs_dir() / "nudge.json"
_EVERY = 10
_DEFAULT_STATE = {"closes": 0, "last_nudge_at": 0}


def _load(path: pathlib.Path) -> dict:
    return dict(load_json_tolerant(path, _DEFAULT_STATE, what="nudge state"))


def tick(*, every: int = _EVERY, path: pathlib.Path | None = None,
         do_consolidate: bool = True) -> dict:
    """Suma 1 al contador de loops cerrados. Cada `every`, dispara mantenimiento.
    Devuelve {closes, nudged: bool, report}. report=None si no toco nudge."""
    path = path or _STATE
    st = _load(path)
    st["closes"] += 1
    nudged, report = False, None
    if st["closes"] % every == 0:
        nudged = True
        st["last_nudge_at"] = st["closes"]
        if do_consolidate:
            try:
                from .memory import consolidate as _consolidate
                report = _consolidate(None, dry_run=False)
            except Exception as e:
                report = {"error": str(e)[:200]}
            # destilado episodico->semantico (bounded, con watermark persistido): sin este paso
            # consolidate mergea un set VACIO — medido 2026-07: 176 episodios, 0 notas, 16 nudges no-op.
            try:
                from .memory import distill_backlog as _distill
                d = _distill(after_id=int(st.get("distill_upto", 0)), limit=5)
                st["distill_upto"] = d["last_id"]
                if isinstance(report, dict):
                    report["distill"] = d
            except Exception as e:
                if isinstance(report, dict):
                    report["distill"] = {"error": str(e)[:200]}
    # guard monotonico: nudge.tick (diurno, MCP server) y nightly.py (02:10) hacen
    # load->modify->write sin lock inter-proceso. Sin esto, last-writer-wins puede
    # retroceder distill_upto y hacer que distill_backlog re-procese episodios ya
    # destilados. Re-leer justo antes de escribir y nunca bajar el watermark en disco.
    on_disk = _load(path)
    if on_disk.get("distill_upto", 0) > st.get("distill_upto", 0):
        st["distill_upto"] = on_disk["distill_upto"]
    atomic_write_json(path, st)
    return {"closes": st["closes"], "nudged": nudged, "report": report}


def status(path: pathlib.Path | None = None) -> dict:
    path = path or _STATE
    st = _load(path)
    return {"closes": st["closes"], "last_nudge_at": st.get("last_nudge_at", 0),
            "every": _EVERY, "next_in": (_EVERY - st["closes"] % _EVERY) % _EVERY or _EVERY}
