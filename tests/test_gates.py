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


def test_clarificaciones_cuenta_preguntas_respondidas():
    ok, detalle = gates.clarificaciones("## Clarificaciones\n- P1: cap? | R: SUPUESTO: float | afecta: R1\n## Otra\n")
    assert ok and detalle == "1 preguntas respondidas"


def test_clarificaciones_acepta_cero_declaradas():
    assert gates.clarificaciones("## Clarificaciones\n- sin preguntas: la spec cubre el barrido.\n") == (True, "0 preguntas")


def test_clarificaciones_rechaza_sin_seccion_vacia_o_mas_de_cinco():
    assert gates.clarificaciones("## Casos\n") == (False, "falta ## Clarificaciones")
    assert not gates.clarificaciones("## Clarificaciones\n- P1: cap? | R:  | afecta: R1\n")[0]
    seis = "".join(f"- P{i}: q? | R: si | afecta: R1\n" for i in range(1, 7))
    assert gates.clarificaciones("## Clarificaciones\n" + seis) == (False, "6 preguntas validas (esperado 1..5)")


def test_bench_los_casos_aparte_no_entran_a_la_metrica_principal():
    r = gates_bench.medir("spec-review", n=60, seed=1)
    assert r["aparte"] == ["espacios", "negrita", "numerada"]
    assert r["n"] == r["tp"] + r["fn"] + r["tn"] + r["fp"] < 60 + r["incidentes"]
    assert set(r["aparte"]) <= set(r["acierto_por_grupo"])


def test_traza_plan_acepta_rango_y_nombre_sin_carpeta():
    block = "- `pkg/a.py` [R1–R3]\n\nNotas:\n- `b.py`: usa R4\n"
    assert gates.traza_plan("R1 R2 R3 R4", block, ["pkg/a.py", "pkg/b.py"]) == (True, "ok")


def test_traza_plan_rechaza_id_huerfano_y_archivo_sin_id():
    ok, detalle = gates.traza_plan("R1 R2", "- `pkg/a.py` [R1]\n- `pkg/b.py`\n", ["pkg/a.py", "pkg/b.py"])
    assert not ok and detalle == "IDs sin unidad: ['R2']; archivos sin ID: ['pkg/b.py']"


def test_filas_tabla_quita_los_bordes():
    assert gates.filas_tabla("| a | b |\n|---|---|\ntexto\n") == [" a | b ", "---|---"]


def test_bench_traza_plan_ningun_archivo_verde_queda_sin_ids():
    import random
    for c in gates_bench._casos_traza_plan(random.Random(5), 80):
        if not c["rojo"]:
            assert "[]" not in c["args"][1] and "()" not in c["args"][1]
