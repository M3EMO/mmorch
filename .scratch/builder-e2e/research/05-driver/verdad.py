"""Verdad del oraculo (ticket 05): referencia del ticket 03 y casos ocultos. Convencion: anualiza 12 meses, sin SAC,
soltero, tablas ARCA jul-dic 2026. Escribe casos.json, que el driver lee y el "coder" nunca ve."""
import json
import pathlib
import random

TOPE = 4_769_631.49           # ANSES Res 284/2026, oct 2026 (fuente secundaria)
GNI, DE2 = 5_585_736.93, 26_811_537.29   # ARCA art 30 jul-dic 2026 (verificado en el PDF)
ESC = [(0, 0, .05), (2_168_491.89, 108_424.59, .09), (4_336_983.77, 303_588.86, .12), (6_505_475.65, 563_807.89, .15),
       (9_758_213.49, 1_051_718.57, .19), (19_516_426.99, 2_905_779.13, .23), (29_274_640.48, 5_150_168.23, .27),
       (43_911_960.73, 9_102_244.70, .31), (65_867_941.10, 15_908_598.62, .35)]
PUBLICOS = [1_500_000, 3_000_000, 3_500_000, 5_000_000, 10_000_000]   # los ejemplos que el coder SI ve en la spec


def impuesto(gni):
    if gni <= 0:
        return 0.0
    d, fijo, t = [e for e in ESC if gni >= e[0]][-1]
    return fijo + t * (gni - d)


def base(g):
    return 12 * (g - 0.17 * min(g, TOPE)) - GNI - DE2


def neto(g):
    return g - 0.17 * min(g, TOPE) - impuesto(base(g)) / 12


def bruto_para(objetivo):
    """Bruto mensual cuya base imponible anual vale `objetivo` (base() es monotona: biseccion)."""
    lo, hi = 0.0, 1e9
    for _ in range(200):
        mid = (lo + hi) / 2
        lo, hi = (mid, hi) if base(mid) < objetivo else (lo, mid)
    return hi


def casos(semilla=20261015):
    bordes = [bruto_para(0), TOPE] + [bruto_para(d) for d, _, _ in ESC[1:]]
    gs = [round(b + k, 2) for b in bordes for k in (-1, 0, 1)]
    rnd = random.Random(semilla)
    gs += [round(rnd.uniform(100_000, 30_000_000), 2) for _ in range(40)]
    return [{"bruto": g, "neto": round(neto(g), 2)} for g in sorted(set(gs)) if g > 0 and g not in PUBLICOS]


if __name__ == "__main__":
    for (d0, f0, t0), (d1, f1, _) in zip(ESC, ESC[1:]):  # coherencia de la escala transcrita (relacion 6)
        assert abs(f0 + t0 * (d1 - d0) - f1) < 1, d1
    out = {"casos": casos(), "semilla_metamorfica": 777, "umbral": bruto_para(0)}
    pathlib.Path(__file__).with_name("casos.json").write_text(json.dumps(out, indent=1), encoding="utf-8")
    print(len(out["casos"]), "casos ocultos; umbral", round(out["umbral"], 2))
