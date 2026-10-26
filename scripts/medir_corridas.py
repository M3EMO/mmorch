"""Mide los umbrales de los gates del SDLC con corridas reales (orchestration-amp, orchestration-1yu).

Uso: python scripts/medir_corridas.py [dir_logs]   (default logs/sdlc)
- tamano-plan: exito sin Claude por cantidad de archivos del plan (run-log-*.json con plan_tamano).
- mutacion y codigo-muerto: precision por umbral contra veredictos humanos (veredictos.jsonl), unidos por
  branch `sdlc/<fase>` -> run-log-<fase>.json.
Sin datos suficientes lo dice y sale 0: la medicion espera corridas nuevas.
"""
import json
import pathlib
import sys

MIN_GRANDES = 20  # el docstring de gate_tamano pide 20 corridas de 4 o mas archivos
TRAMOS = ((1, 1, "1"), (2, 3, "2-3"), (4, 7, "4-7"), (8, 10**6, "8+"))


def exito_sin_claude(log: dict) -> bool:
    etapas = log.get("stages") or []
    return (not log.get("failed_stage") and not log.get("escalated_to_claude")
            and any(e.get("stage") == "6-pr" and e.get("ok") for e in etapas))


def tamano(logs: list[dict]) -> dict:
    """{tramo: [exitos, n]} por archivos del plan, y cuantas corridas tienen 4 o mas archivos."""
    filas = {nombre: [0, 0] for _, _, nombre in TRAMOS}
    for log in logs:
        t = log.get("plan_tamano")
        if not t:
            continue
        nombre = next(n for lo, hi, n in TRAMOS if lo <= t["archivos"] <= hi)
        filas[nombre][0] += exito_sin_claude(log)
        filas[nombre][1] += 1
    return {"tramos": filas, "grandes": filas["4-7"][1] + filas["8+"][1]}


def precision_por_umbral(pares: list[tuple[float, bool]], umbrales=(0.2, 0.4, 0.6, 0.8, 1.0)) -> dict:
    """{umbral: (precision, recall)} con aprobado = positivo y puntaje >= umbral = predice aprobado."""
    out = {}
    aprobados = sum(a for _, a in pares)
    for u in umbrales:
        pasa = [a for s, a in pares if s >= u]
        out[u] = (round(sum(pasa) / len(pasa), 2) if pasa else None,
                  round(sum(pasa) / aprobados, 2) if aprobados else None)
    return out


def pares_veredicto(veredictos: list[dict], por_fase: dict[str, dict], clave: str) -> list[tuple[float, bool]]:
    """(puntaje, aprobado) por veredicto con run-log; cobertura_nueva es {archivo: ratio} y cuenta el peor."""
    pares = []
    for v in veredictos:
        log = por_fase.get(v.get("branch", "").removeprefix("sdlc/"))
        s = log.get(clave) if log else None
        if isinstance(s, dict):
            s = min(s.values()) if s else None
        if s is not None and v.get("label") in ("aprobado", "rechazado"):
            pares.append((float(s), v["label"] == "aprobado"))
    return pares


def main(argv: list[str]) -> int:
    d = pathlib.Path(argv[0] if argv else "logs/sdlc")
    por_fase = {p.stem.removeprefix("run-log-"): json.loads(p.read_text(encoding="utf-8"))
                for p in sorted(d.glob("run-log-*.json"))}
    t = tamano(list(por_fase.values()))
    print("tamano-plan: exito sin Claude por archivos del plan")
    for nombre, (ok, n) in t["tramos"].items():
        print(f"  {nombre:>4}: {ok}/{n}" + (f" = {ok / n:.2f}" if n else ""))
    if t["grandes"] < MIN_GRANDES:
        print(f"  faltan {MIN_GRANDES - t['grandes']} corridas de 4 o mas archivos para revisar el umbral")
    vpath = d / "veredictos.jsonl"
    veredictos = [json.loads(x) for x in vpath.read_text(encoding="utf-8").splitlines() if x.strip()] if vpath.exists() else []
    for clave, gate in (("mutation_score", "mutacion"), ("cobertura_nueva", "codigo-muerto")):
        pares = pares_veredicto(veredictos, por_fase, clave)
        print(f"{gate}: {len(pares)} veredictos con {clave} (precision, recall de aprobado por umbral)")
        if pares:
            for u, (p, r) in precision_por_umbral(pares).items():
                print(f"  >= {u}: precision {p}, recall {r}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
