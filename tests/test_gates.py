"""Chequeos de mmorch/gates.py y su banco (bd orchestration-bpn). Cero API."""
from mmorch import gates, gates_bench

BASE = "import os\n\ndef f(a):\n    return a + 1\n\ndef g(b):\n    return b * 2\n\ndef h(c):\n    return c - 3\n"


def test_sin_clones_rechaza_una_copia_con_otro_encabezado():
    ok, detalle = gates.sin_clones({"a.py": BASE, "b.py": "# worker\n" + BASE})
    assert not ok and "a.py y b.py" in detalle and "66%" in detalle


def test_sin_clones_acepta_modulos_distintos_con_imports_comunes():
    assert gates.sin_clones({"a.py": BASE, "b.py": "import os\n\ndef z(q):\n    return q ** 2\n"}) == (True, "ok")


def test_sin_clones_ignora_archivos_vacios():
    assert gates.sin_clones({"a.py": BASE, "b.py": "\n  \n"}) == (True, "ok")


def test_sin_clones_el_borde_exacto_no_es_clon():
    # 4 de 5 lineas compartidas = 0.8 exacto: el gate exige > umbral
    a = "l1\nl2\nl3\nl4\nl5\n"
    assert gates.sin_clones({"a.py": a, "b.py": "l1\nl2\nl3\nl4\nOTRA\n"}, umbral=0.8)[0]
    assert not gates.sin_clones({"a.py": a, "b.py": "l1\nl2\nl3\nl4\nl5\nOTRA\n"}, umbral=0.8)[0]


def test_sin_clones_detecta_copias_editadas_y_respeta_hermanos():
    r = gates_bench.medir("sin-clones", n=400, seed=9)
    assert r["acierto_por_grupo"]["copia 20-30%"] == 1.0 and r["acierto_por_grupo"]["hermanos"] == 1.0


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


def test_traza_spec_exige_tabla_con_tests_existentes():
    spec = "## Requisitos\n- R1: a.\n- R2: b.\n\n## Trazabilidad\n\n| ID | tests |\n|---|---|\n| R1 | test_a |\n| R2 | `test_a`, test_b |\n"
    assert gates.traza_spec(spec, {"test_a", "test_b"}, True) == (True, "2 IDs cubiertos")
    assert gates.traza_spec(spec, {"test_a"}, True) == (False, "sin test: []; tests inexistentes: ['test_b']")


def test_traza_spec_sin_tests_nombrados_alcanzan_los_ids():
    assert gates.traza_spec("- R1: a.\n", set(), False)[0]
    assert gates.traza_spec("- sin ids\n", set(), False) == (False, "sin IDs R<n>")


def test_plan_allowlist_rechaza_test_ajeno_omision_y_fuera_del_techo():
    plan = "## Archivos\n- `pkg/a.py` [R1]\n- `tests/test_capas.py` [R1]\n"
    need, techo = ["pkg/a.py", "tests/test_capas.py"], ["pkg/**/*.py", "tests/test_capas.py"]
    assert gates.plan_allowlist(plan, ["pkg/a.py", "tests/test_capas.py"], need, techo, "tests/")[0]
    assert "test_otro" in gates.plan_allowlist(plan + "- `tests/test_otro.py`\n", need, need, techo, "tests/")[1]
    assert "omite" in gates.plan_allowlist(plan, ["pkg/a.py"], need, techo, "tests/")[1]
    assert "fuera del techo" in gates.plan_allowlist(plan, need + ["otro/x.py"], need, techo, "tests/")[1]


def test_calza_y_en_techo():
    assert gates.calza("src/App.java", "src/**/*.java") and gates.en_techo("x/y.py", [], [])
    assert not gates.en_techo("otro/x.py", ["pkg/a.py"], ["pkg/**/*.py"])


def test_cambio_minimo_rechaza_reescritura_y_acepta_parche_o_pedido():
    orig = {"pkg/a.py": "\n".join(f"x{i} = {i}" for i in range(100))}
    assert not gates.cambio_minimo(orig, {"pkg/a.py": "nuevo = 1\n"}, "agrega una guarda")[0]
    assert gates.cambio_minimo(orig, {"pkg/a.py": orig["pkg/a.py"] + "\nguarda = 1\n"}, "agrega una guarda") == (True, "ok")
    assert gates.cambio_minimo(orig, {"pkg/a.py": "nuevo = 1\n"}, "Reescribi a.py") == (True, "la tarea pide reescribir")


def test_docstring_intacto():
    assert gates.docstring_intacto({"a.py": "doc"}, {"a.py": "doc nuevo"}, "t") == (True, "ok")
    assert gates.docstring_intacto({"a.py": "doc"}, {"a.py": None}, "t") == (False, "docstring de modulo borrado en ['a.py']")
    assert gates.docstring_intacto({"a.py": "doc"}, {"a.py": None}, "cambia el docstring")[0]


def test_un_archivo_y_secciones():
    fre = r"((?:[\w.-]+/)*[\w.-]+\.(?:py))"
    code = "# pkg/a.py\nA = 1\n# pkg/b.py\nB = 2\n"
    assert gates.secciones(code, fre) == {"pkg/a.py": "A = 1", "pkg/b.py": "B = 2"}
    assert gates.un_archivo(code, "pkg/a.py", fre)[0] and not gates.un_archivo(code, "pkg/c.py", fre)[0]
    assert gates.secciones("solo = 1\n", fre) == {} and gates.un_archivo("solo = 1\n", "pkg/a.py", fre)[0]


def test_tipo_propio():
    assert gates.tipo_propio("public final class Opcion {\n    public static class Item {}\n}\n", "src/Opcion.java")[0]
    assert gates.tipo_propio("public class Negocio {}\n", "src/Opcion.java") == (False, "src/Opcion.java: declara public Negocio")
    assert gates.tipo_propio("public class Negocio {}\n", "src/x.ts")[0]


def test_bugs_medidos_quedan_en_uno():
    # orchestration-5s7, -1tj, -96p: cada hueco medido se cierra sin bajar la especificidad
    for g in ("plan-allowlist", "trazabilidad-plan", "spec-review"):
        r = gates_bench.medir(g, n=200, seed=4)
        assert r["sensibilidad"] == 1.0 and r["especificidad"] == 1.0, (g, r["acierto_por_grupo"])
    assert not gates.plan_allowlist("## Archivos\n- `/etc/x.py`\n", ["/etc/x.py"], [], [], "tests/")[0]


def test_secciones_reconoce_cabeceras_con_prefijo_y_backticks():
    fre = r"((?:[\w.-]+/)*[\w.-]+\.(?:py))"
    for cab in ("# File: {}", "# `{}`", "### {}", "// **{}**"):
        code = cab.format("pkg/a.py") + "\nA = 1\n" + cab.format("pkg/b.py") + "\nB = 2\n"
        assert gates.secciones(code, fre) == {"pkg/a.py": "A = 1", "pkg/b.py": "B = 2"}, cab


def test_tipo_propio_ve_la_anotacion_en_la_misma_linea():
    assert not gates.tipo_propio('@SuppressWarnings("x") public class Otro {}\n', "src/Opcion.java")[0]
    assert gates.tipo_propio("@Deprecated public final class Opcion {}\n", "src/Opcion.java")[0]
