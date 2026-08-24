"""Detector de estancamiento — tendencias sobre la historia nocturna, cero LLM.

Motivo (2026-08-21): la reflexión LLM detectó estancamientos reales (evolve 12+
noches sin PRs, autoresearch 15+ corridas plano, suite ajena roja 3+ noches)
pero emite PROSA que nadie consume — auto_repair solo ve ERRORES de la última
corrida, y un estancamiento no es un error: cada noche individual "salió bien",
el problema solo aparece mirando N noches juntas. Ese día el bucle se cerró a
MANO (diagnóstico + fix + gates). Este módulo es la versión mecánica del paso 1
de ese workflow: reglas deterministas sobre nightly.jsonl.

Enchufe deliberadamente invisible: nightly.py mete el resultado como
rec["stuck"] = {"errors": [...]} y findings_from_record (auto_repair) lo
levanta SIN cambios — mismo circuito probado: task de REPAIR en worktree,
gate de suite, branch amarilla, merge humano. Un sensor nuevo, cero mecanismo
nuevo.

Qué NO detecta a propósito:
- proyectos ajenos con suite roja crónica: eso ya lo cubre project_repair
  cada noche (con su retry window) — duplicarlo acá haría dos reparadores
  pisándose el mismo objetivo.
- nada que requiera juicio de dominio (qué evento político agregar, qué
  dirección): eso queda humano SIEMPRE — el finding solo describe la señal.

Firma estable: _sig() de auto_repair usa source + detail[:80] — el texto de
cada finding arranca con un prefijo FIJO por regla+objetivo y deja los
números cambiantes al final, así el retry window (5d) agrupa bien.
"""

from __future__ import annotations

_MIN_NIGHTS = 5
_HISTORY = 10   # cuántas noches mirar hacia atrás (incluida la actual)


def _consecutive_recent(history: list[dict], pred) -> int:
    """Cuántas noches CONSECUTIVAS desde la más reciente cumplen pred."""
    n = 0
    for rec in reversed(history):
        if not pred(rec):
            break
        n += 1
    return n


def stuck_findings(history: list[dict], *, min_nights: int = _MIN_NIGHTS) -> list[str]:
    """Reglas de tendencia sobre los últimos records nocturnos (viejo→nuevo,
    el actual último). Devuelve strings listos para rec['stuck']['errors'].
    Cada regla se resetea sola cuando la señal desaparece (una noche con PR
    abierto corta la racha de evolve, una mejora corta la de autoresearch)."""
    history = history[-_HISTORY:]
    out: list[str] = []
    if not history:
        return out

    # 1. evolve en bucle muerto: genera findings pero jamás abre un PR
    n = _consecutive_recent(
        history,
        lambda r: (r.get("evolve") or {}).get("findings", 0) > 0
        and not (r.get("evolve") or {}).get("opened"))
    if n >= min_nights:
        rojos: dict[str, int] = {}
        for r in history:
            for m in (r.get("evolve") or {}).get("red") or []:
                rojos[m] = rojos.get(m, 0) + 1
        top = sorted(rojos, key=lambda m: -rojos[m])[:3]
        out.append(
            "stuck evolve bucle muerto: genera findings pero jamas abre PR — "
            "atacar el mecanismo, no los modulos marcados. Racha actual: "
            f"{n} noches. Modulos mas repetidos en rojo: {', '.join(top)}. "
            "Diagnostica POR QUE los sandboxes nunca verdean (leer "
            "logs/evolve_red.jsonl — cada sandbox rojo persiste ahi su "
            "fitness.detail con el output real de pytest — y "
            "logs/evolve_findings.jsonl) antes de proponer un fix — el fix va "
            "al mecanismo, no a los modulos marcados.")

    # 2. autoresearch plano: mismo target, sin mejora, N corridas seguidas
    def _flat(r: dict) -> bool:
        a = r.get("autoresearch") or {}
        return bool(a.get("target")) and not a.get("improved")

    n = _consecutive_recent(history, _flat)
    if n >= min_nights:
        targets = {(r.get("autoresearch") or {}).get("target") for r in history[-n:]}
        out.append(
            "stuck autoresearch plano: sin mejora hace "
            f"{n} corridas (targets: {', '.join(sorted(t for t in targets if t))}). "
            "O el evaluador no discrimina (baseline==best fijo) o el propose "
            "no ve QUE falla. Correr el scorer a mano y mirar su detalle por "
            "tarea (lineas FAIL) antes de tocar nada.")

    # 3. módulo crónicamente rojo: aparece en evolve.red la mayoría de las
    # noches Y nadie le abrió un PR en la ventana (PR abierto = ya está
    # siendo atendido, marcarlo igual sería un reparador pisando a otro —
    # caso cazado por el self-check, no teórico)
    atendidos = {m for r in history
                 for m in (r.get("evolve") or {}).get("opened") or []}
    conteo: dict[str, int] = {}
    for r in history:
        for m in set((r.get("evolve") or {}).get("red") or []):
            if m not in atendidos:
                conteo[m] = conteo.get(m, 0) + 1
    for m, c in sorted(conteo.items(), key=lambda kv: -kv[1]):
        if c >= min_nights:
            out.append(
                f"stuck modulo cronico {m}: en rojo {c} de las ultimas "
                f"{len(history)} noches — el finding se repite y ningun intento "
                "prospera. Leer los findings acumulados de ese modulo en "
                "logs/evolve_findings.jsonl y atacar la CAUSA COMUN, no el "
                "sintoma de una noche.")
            break   # 1 módulo por noche: auto_repair igual toma 1 finding/noche

    # 4. chequeo del smoke rojo N noches seguidas. auto_repair solo levanta
    # claves error/errors del record, y el smoke reporta {"fails": [...]} —
    # forma de dato, no de excepción. Medido 2026-08-24: el check "server"
    # llevaba 6 noches rojo (el proceso murió el 18 y su tarea solo arranca
    # AtLogOn) sin que ningún sensor lo nombrara.
    fallando = set((history[-1].get("smoke") or {}).get("fails") or [])
    for chk in sorted(fallando):
        n = _consecutive_recent(
            history, lambda r, c=chk: c in ((r.get("smoke") or {}).get("fails") or []))
        if n >= min_nights:
            why = ((history[-1].get("smoke") or {}).get("why") or {}).get(chk, "")
            out.append(
                f"stuck smoke {chk}: rojo {n} noches seguidas. "
                f"Motivo de anoche: {why[:200] or '(el smoke no registro el porque)'}. "
                "Verificar primero que el subsistema este VIVO (proceso, tarea "
                "programada, puerto) antes de tocar codigo — el chequeo es "
                "read-only, su rojo suele ser de entorno, no de logica.")
            break   # 1 por noche

    # 5. tren con gate rojo N noches seguidas: dos amarillas verdes por
    # separado que se rompen ENTRE SI y nadie lo mira. gate_reason trae el
    # tail real de pytest desde 2026-08-24.
    n = _consecutive_recent(
        history, lambda r: (r.get("merge_train") or {}).get("gate") == "rojo")
    if n >= min_nights:
        mt = history[-1].get("merge_train") or {}
        out.append(
            f"stuck merge_train gate rojo: {n} noches seguidas sin tren verde "
            f"(ramas: {', '.join(mt.get('merged') or []) or 'sin datos'}). "
            f"Salida del gate: {(mt.get('gate_reason') or '')[-400:] or '(sin registro)'}. "
            "El fix va a la INTERACCION entre las ramas, no a una sola.")

    return out


def _demo() -> None:
    """Self-check: racha detectada, racha cortada, historia corta callada."""
    muerto = {"evolve": {"findings": 3, "opened": [], "red": ["mmorch/a.py"]}}
    vivo = {"evolve": {"findings": 3, "opened": ["mmorch/a.py"], "red": []}}
    assert stuck_findings([muerto] * 6)  # 6 noches muertas -> finding
    assert not stuck_findings([muerto] * 3)  # racha corta -> silencio
    assert not stuck_findings([muerto] * 6 + [vivo])  # PR abierto corta la racha
    plano = {"autoresearch": {"target": "p.txt", "improved": False}}
    assert any("autoresearch" in f for f in stuck_findings([plano] * 5))
    assert stuck_findings([]) == []
    print("stuck_detector ok")


if __name__ == "__main__":
    _demo()
