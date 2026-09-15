"""Gates puros de mmorch.sdlc (ticket 05, paso 1). Cero API, cero git: configure() con una task ficticia.

Cada caso viene de un defecto real del driver (research/07-resultados.md del mapa sdlc-6-gates):
- strip_fence: una valla ``` DENTRO de un string cortaba el archivo (D2, D11-diag2).
- one_file: el coder pego 3 archivos con cabeceras `# path` en una respuesta (r1 2026-09-11).
- plan-allowlist: tests/ prohibido salvo los que la feature exige (D14: test_capas.py del ratchet).
- cambio-minimo: el coder reescribio project_loop.py (-216 lineas) para agregar una guarda (D11).
- clarificaciones: Claude escribe "0 preguntas" y "afecta: Contrato" con formato libre (D11-diag).
"""
import types

import pytest

import mmorch.sdlc as S

INNER = 'x = "```\\n{cur}\\n```"\ny = 1'


@pytest.fixture
def run(tmp_path):
    t = types.SimpleNamespace(name="t", task="agrega una guarda; no se tocan tests", accept_files={})
    feat = {"repo": str(tmp_path), "files": ["pkg/a.py", "tests/test_capas.py"], "suite": ["tests"]}
    S.configure(t, contract=["a"], feat=feat, wt=tmp_path / "wt")
    return S


def test_strip_fence_solo_vallas_de_linea_entera(run):
    assert S.strip_fence("Aca va:\n```python\n" + INNER + "\n```\nlisto") == INNER
    assert S.strip_fence("```python\n" + INNER + "\n```") == INNER
    assert S.strip_fence(INNER) == INNER


def test_one_file_recorta_su_seccion(run):
    pegado = "# pkg/a.py\nA = 1\n# pkg/b.py\nB = 2\n"
    assert S.one_file(pegado, "pkg/a.py").strip() == "A = 1"
    assert S.one_file("solo = 1\n", "pkg/a.py") == "solo = 1\n"


def test_plan_allowlist_tests_ajenos_no_y_obligatorios_si(run):
    ok, _ = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `tests/test_capas.py` [R1]\n", ["pkg/a.py", "tests/test_capas.py"])
    assert ok
    ok, nota = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `tests/test_otro.py` [R1]\n", ["pkg/a.py"])
    assert not ok and "test_otro" in nota
    ok, nota = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n", ["pkg/a.py"])
    assert not ok and "omite" in nota


def test_plan_files_conserva_los_tests_obligatorios(run):
    assert S._plan_files("- `pkg/a.py`\n- `tests/test_capas.py`\n- `tests/test_otro.py`\n") == ["pkg/a.py", "tests/test_capas.py"]


def test_cambio_minimo_rechaza_reescritura_y_acepta_parche(run):
    orig = "\n".join(f"linea_{i} = {i}" for i in range(100)) + "\n"
    S._orig_files.clear(); S._orig_files["pkg/a.py"] = orig
    ok, nota = S.gate_cambio_minimo({}, {"pkg/a.py": "nuevo = 1\n"})
    assert not ok and "100 de 100" in nota
    ok, _ = S.gate_cambio_minimo({}, {"pkg/a.py": orig + "guarda = True\n"})
    assert ok


def test_cambio_minimo_pasa_si_la_tarea_pide_reescribir(run):
    S.TASK.task = "reescribi pkg/a.py entero"
    S._orig_files.clear(); S._orig_files["pkg/a.py"] = "a = 1\n" * 50
    assert S.gate_cambio_minimo({}, {"pkg/a.py": "b = 2\n"})[0]


def test_clarificaciones_acepta_cero_y_afecta_libre(run):
    assert S.gate_clarificaciones("## Clarificaciones\n\nHice 0 preguntas: la spec cubre todo.\n")[0]
    ok, nota = S.gate_clarificaciones("## Clarificaciones\n- P1: ¿x? | R: si | afecta: Contrato\n")
    assert ok and "1 preguntas" in nota
    assert not S.gate_clarificaciones("## Otra\n")[0]
    assert not S.gate_clarificaciones("## Clarificaciones\n\n")[0]


def test_files_from_toml_es_el_techo(tmp_path):
    with pytest.raises(ValueError):
        S._files_from_toml(str(tmp_path))
    (tmp_path / "sdlc.toml").write_text('files = ["pkg/a.py"]\n', encoding="utf-8")
    assert S._files_from_toml(str(tmp_path)) == ["pkg/a.py"]


def test_resolve_files_toml_es_techo_y_el_payload_solo_acota(tmp_path):
    with pytest.raises(ValueError):
        S._resolve_files(str(tmp_path), None)
    assert S._resolve_files(str(tmp_path), ["pkg/a.py"]) == ["pkg/a.py"]  # sin toml: lo que dice el llamador
    (tmp_path / "sdlc.toml").write_text('files = ["pkg/a.py", "pkg/b.py"]\n', encoding="utf-8")
    assert S._resolve_files(str(tmp_path), None) == ["pkg/a.py", "pkg/b.py"]
    assert S._resolve_files(str(tmp_path), ["pkg/b.py"]) == ["pkg/b.py"]
    with pytest.raises(ValueError, match="fuera del techo"):
        S._resolve_files(str(tmp_path), ["pkg/c.py"])


def test_stage_fallida_es_excepcion_con_etapa(run):
    @S.stage("9-falsa")
    def falsa():
        return False, "motivo"
    with pytest.raises(S.StageFailed) as e:
        falsa()
    assert e.value.stage == "9-falsa" and e.value.note == "motivo"
    assert S.state["stages"][-1]["ok"] is False


def test_build_feature_exige_oraculo(tmp_path):
    with pytest.raises(ValueError, match="accept"):
        S.build_feature("x", "t", str(tmp_path))
