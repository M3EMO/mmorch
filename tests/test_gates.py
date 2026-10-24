"""Chequeos de mmorch/gates.py y su banco (bd orchestration-bpn). Cero API."""
from mmorch import gates, gates_bench

BASE = "import os\n\ndef f(a):\n    return a + 1\n\ndef g(b):\n    return b * 2\n\ndef h(c):\n    return c - 3\n"


def test_sin_clones_rechaza_una_copia_con_otro_encabezado():
    ok, detalle = gates.sin_clones({"a.py": BASE, "b.py": "# worker\n" + BASE})
    assert not ok and "a.py y b.py" in detalle and "80%" in detalle


def test_sin_clones_acepta_modulos_distintos_con_imports_comunes():
    assert gates.sin_clones({"a.py": BASE, "b.py": "import os\n\ndef z(q):\n    return q ** 2\n"}) == (True, "ok")


def test_sin_clones_ignora_archivos_vacios():
    assert gates.sin_clones({"a.py": BASE, "b.py": "\n  \n"}) == (True, "ok")


def test_sin_clones_el_borde_exacto_no_es_clon():
    # 4 de 5 lineas compartidas = 0.8 exacto: el gate exige > umbral
    a = "l1\nl2\nl3\nl4\nl5\n"
    assert gates.sin_clones({"a.py": a, "b.py": "l1\nl2\nl3\nl4\nOTRA\n"})[0]
    assert not gates.sin_clones({"a.py": a, "b.py": "l1\nl2\nl3\nl4\nl5\nOTRA\n"})[0]


def test_bench_cada_chequeo_tiene_generador_e_incidentes_marcados():
    assert set(gates_bench.CHEQUEOS) == set(gates_bench.GENERADORES)
    for casos in gates_bench.INCIDENTES.values():
        assert all("reconstruido" in c and c["origen"] != "generado" for c in casos)


def test_bench_es_determinista_y_trae_las_dos_clases():
    a, b = gates_bench.medir("sin-clones", n=40, seed=3), gates_bench.medir("sin-clones", n=40, seed=3)
    assert a == b
    assert a["tp"] + a["fn"] > 0 and a["tn"] + a["fp"] > 0


def test_bench_ninguna_copia_supera_el_limite_de_edicion():
    import random
    rng = random.Random(0)
    for _ in range(200):
        orig = gates_bench._modulo(rng)
        _, frac = gates_bench._copia_editada(rng, orig)
        assert frac <= gates_bench._MAX_EDICION
