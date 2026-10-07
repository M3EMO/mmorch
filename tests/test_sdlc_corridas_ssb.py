"""Defectos que dejaron las corridas de SSB (TypeScript, 2026-09-28 a 2026-10-01) despues de orchestration-7ys.

- F7a (mmorch/wt-2bbe938c): el plan listo los archivos en una tabla markdown y plan-allowlist dijo "plan omite" los 4.
- F4b (mmorch/wt-51062788): una nota "- `resolver.ts`: unico modulo" quedo como archivo fuera del techo.
- F3a (mmorch/wt-cef763b5): el test de F3b, commiteado antes y rojo por diseño, tumbaba G3 con un TS2307 ajeno.
- F7a2 (mmorch/wt-b2cea3cd): el re-pedido de cambio-minimo rompio la compilacion y la etapa cortaba sin escalera,
  con la nota "cambio-minimo: ok"; ademas los gates leian la version del coder previa a la escalera.
- Los prompts de spec y de Claude en la etapa 5 decian Python y pytest en un repo TypeScript.
Cero API: llm y el revisor son dobles; el "compilador" es un script Python que imita la salida de tsc.
"""
import json
import re
import subprocess
import types

import pytest

import mmorch.sdlc as S

R_ALL = " ".join(f"R{i}" for i in range(1, 11))


def _conf(tmp_path, monkeypatch, files, accept_files=None, accept_cmd="node --test tests/*.test.ts"):
    t = types.SimpleNamespace(name="ssb", task="servicio de escenas", accept_files=dict(accept_files or {}))
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": files, "techo": ["src/**/*.ts"], "suite": []},
                wt=tmp_path, accept_cmd=accept_cmd)
    S.CFG["ext"] = ["ts"]
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")


def _bloque(plan_md):
    """La seccion '## Archivos' del plan, como la recorta plan()."""
    m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
    return m.group(1) if m else plan_md


def _git(tmp_path, *args):
    subprocess.run(["git", *args], cwd=tmp_path, check=True, capture_output=True)


def _commit(tmp_path, archivos: dict[str, str]):
    for rel, texto in archivos.items():
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / rel).write_text(texto, encoding="utf-8")
    if not (tmp_path / ".git").exists():
        _git(tmp_path, "init", "-q")
    _git(tmp_path, "add", "-A")
    _git(tmp_path, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "base")


# --- punto 3: plan-allowlist ---------------------------------------------------------------------------------------

PLAN_F7A = """# Plan: F7a

## Archivos a tocar

| Archivo | Acción | Qué aporta |
|---|---|---|
| `src/escena/tipos.ts` | ampliar | `TimelineApp.cameras?` y `ClaveCamaraApp` (R1). |
| `src/escena/construir.ts` | ampliar | `timeline.cameras[id]` desde `camara.claves` (R1). Sin cambios para F1 (`src/escena/png.ts`). |
| `src/servicio/tipos.ts` | nuevo | `EscenaSpec`, `PlanoSpec`, `Look`, `Reporte` (R2). |
| `src/servicio/escena.ts` | nuevo | `construirEscena`, `editarEscena`, `leerEscena`, `verificarPlano` (R2–R10). |
| `tests/` | nuevo | Los tests de la tabla de Trazabilidad. |

No se escribe Python: TypeScript ejecutado por Node 24 sin build.

## Etapas de implementación
"""
F7A = ["src/escena/tipos.ts", "src/escena/construir.ts", "src/servicio/tipos.ts", "src/servicio/escena.ts"]


def test_plan_en_tabla_markdown_pasa_allowlist_y_trazabilidad(tmp_path, monkeypatch):
    _conf(tmp_path, monkeypatch, F7A)
    block = _bloque(PLAN_F7A)
    assert S._plan_files(block) == F7A   # la mencion `src/escena/png.ts` en otra celda no es un archivo del plan
    assert S.gate_plan_allowlist(PLAN_F7A, S._plan_files(block))[0]
    ok, nota = S.gate_traza_plan(f"## Requisitos\n{R_ALL}\n", block, F7A)   # "R2–R10" cubre R2..R10
    assert ok, nota


PLAN_F4B = """## Archivos
- `src/encuadre/resolver.ts` [R1, R2, R3]

Notas por archivo:

- `resolver.ts`: único módulo del feature. Importa `Vec3` de `../escena/tipos.ts`.

## Prueba
"""


def test_nombre_sin_carpeta_se_resuelve_contra_el_plan(tmp_path, monkeypatch):
    _conf(tmp_path, monkeypatch, [])
    files = S._plan_files(_bloque(PLAN_F4B))
    assert files == ["src/encuadre/resolver.ts"]
    assert S.gate_plan_allowlist(PLAN_F4B, files)[0]


def test_nombre_sin_carpeta_se_resuelve_contra_el_repo_y_no_adivina_si_es_ambiguo(tmp_path, monkeypatch):
    _commit(tmp_path, {"src/encuadre/resolver.ts": "export {};\n", "src/escena/tipos.ts": "export {};\n",
                       "src/servicio/tipos.ts": "export {};\n"})
    _conf(tmp_path, monkeypatch, [])
    assert S._plan_files("- `resolver.ts` [R1]\n") == ["src/encuadre/resolver.ts"]
    ambiguo = "## Archivos\n- `tipos.ts` [R1]\n"
    files = S._plan_files(_bloque(ambiguo))
    assert files == ["tipos.ts"]
    ok, nota = S.gate_plan_allowlist(ambiguo, files)
    assert not ok and "tipos.ts" in nota


# --- punto 5: G3 corre tsc sobre todo el repo ----------------------------------------------------------------------

TSC_FALSO = """import pathlib
for p in sorted(pathlib.Path('.').rglob('*.ts')):
    for n, linea in enumerate(p.read_text(encoding='utf-8').splitlines(), 1):
        if 'ROTO' in linea:
            print(f"{p.as_posix()}({n},7): error TS2307: Cannot find module '{linea.split('ROTO')[1].strip()}'.")
import sys
sys.exit(1 if any('ROTO' in p.read_text(encoding='utf-8') for p in pathlib.Path('.').rglob('*.ts')) else 0)
"""


def _repo_con_test_ajeno(tmp_path, monkeypatch):
    """Base: src/a.ts compila; tests/f3b.test.ts (otra feature, rojo por diseño) no; tests/mia.test.ts es la aceptacion."""
    _commit(tmp_path, {"tsc_falso.py": TSC_FALSO, "src/a.ts": "export const a = 1;\n",
                       "tests/f3b.test.ts": "import { c } from ROTO ../src/clasificar.ts\n",
                       "tests/mia.test.ts": "import { a } from '../src/a.ts';\n"})
    _conf(tmp_path, monkeypatch, ["src/a.ts"], accept_files={"tests/mia.test.ts": "import { a } from '../src/a.ts';\n"})
    S.CFG["compile_cmd"] = "python tsc_falso.py"
    S.state["plan_files"] = ["src/a.ts"]


def test_compile_ignora_errores_previos_de_otra_feature(tmp_path, monkeypatch):
    _repo_con_test_ajeno(tmp_path, monkeypatch)
    (tmp_path / "src" / "a.ts").write_text("export const a = 2;\n", encoding="utf-8")
    ok, nota = S.gate_compile(["src/a.ts"])
    assert ok, nota
    assert "tests/f3b.test.ts" in nota   # queda anotado, no escondido
    assert S.gate_test_compile()[0]


def test_compile_sigue_rechazando_errores_nuevos(tmp_path, monkeypatch):
    _repo_con_test_ajeno(tmp_path, monkeypatch)
    (tmp_path / "src" / "a.ts").write_text("export const a = ROTO ./b.ts\n", encoding="utf-8")
    ok, nota = S.gate_compile(["src/a.ts"])
    assert not ok and "src/a.ts" in nota
    # el test de aceptacion de ESTA feature nunca entra al baseline: su error sigue bloqueando
    (tmp_path / "src" / "a.ts").write_text("export const a = 2;\n", encoding="utf-8")
    (tmp_path / "tests" / "mia.test.ts").write_text("import { b } from ROTO ../src/b.ts\n", encoding="utf-8")
    ok, nota = S.gate_test_compile()
    assert not ok and "tests/mia.test.ts" in nota
    assert (tmp_path / "src" / "a.ts").read_text(encoding="utf-8") == "export const a = 2;\n"   # el baseline restaura


# --- punto 1: el re-pedido de cambio-minimo tambien escala -------------------------------------------------------

ORIG = "".join(f"export const k{i} = {i};\n" for i in range(60))
COMPILA = "python -c \"import sys; sys.exit(1 if 'ROTO' in open('src/a.ts', encoding='utf-8').read() else 0)\""


def _build(tmp_path, monkeypatch, respuestas):
    (tmp_path / "src").mkdir(parents=True, exist_ok=True)
    (tmp_path / "src" / "a.ts").write_text(ORIG, encoding="utf-8")
    _conf(tmp_path, monkeypatch, ["src/a.ts"], accept_cmd="python -c \"pass\"")
    S.CFG["compile_cmd"] = COMPILA
    S.state["plan_files"] = ["src/a.ts"]
    (tmp_path / "docs" / "sdlc").mkdir(parents=True)
    (tmp_path / "docs" / "sdlc" / "spec.md").write_text("spec", encoding="utf-8")
    (tmp_path / "docs" / "sdlc" / "plan.md").write_text("plan", encoding="utf-8")
    monkeypatch.setattr(S, "gate_test_compile", lambda: (True, "sin tests"))

    def fake(model, system, user, timeout=400):
        if "diagnosticador" in system:
            return json.dumps({"files": ["src/a.ts"], "instructions": {"src/a.ts": "quita ROTO"}})
        for clave, texto in respuestas.items():
            if clave in system or clave in user:
                return texto
        raise AssertionError(f"prompt sin respuesta: {user[:80]}")
    monkeypatch.setattr(S, "llm", fake)


def test_repedido_de_cambio_minimo_que_no_compila_entra_al_fix_loop(tmp_path, monkeypatch):
    _build(tmp_path, monkeypatch, {
        "sin explicacion ni markdown": "export const k0 = 0;\n",          # borra 59 lineas: cambio-minimo
        "borro codigo existente": ORIG + "export const nuevo = ROTO;\n",  # conserva, pero no compila
        "Corregi": ORIG + "export const nuevo = 1;\n"})
    S.build()   # antes: StageFailed("4-build", "cambio-minimo: ok")
    assert S.state["stages"][-1]["ok"]
    assert "nuevo = 1" in (tmp_path / "src" / "a.ts").read_text(encoding="utf-8")


def test_gates_despues_de_la_escalera_leen_lo_que_compila(tmp_path, monkeypatch):
    corto_roto = "export const k0 = ROTO;\n"
    _build(tmp_path, monkeypatch, {
        "sin explicacion ni markdown": corto_roto, "Corregi": corto_roto, "Aplica ESTA instruccion": corto_roto,
        "borro codigo existente": ORIG + "export const nuevo = ROTO;\n"})

    def claude(prompt, **kw):   # Claude arregla con el cambio minimo
        (tmp_path / "src" / "a.ts").write_text(ORIG + "export const nuevo = 1;\n", encoding="utf-8")
        return {"ok": True, "result": "ok", "returncode": 0}
    monkeypatch.setattr(S, "_revisor", claude)
    S.build()   # antes: cambio-minimo miraba la version rota del coder y el re-pedido pisaba el arreglo de Claude
    assert S.state["stages"][-1]["ok"]
    assert not [r for r in S.state["gate_rejects"] if r["gate"] == "cambio-minimo"]


# --- punto 2: prompts segun el lenguaje del repo -----------------------------------------------------------------

class _Corte(Exception):
    pass


def test_prompts_de_spec_y_de_claude_siguen_al_lenguaje(tmp_path, monkeypatch):
    _conf(tmp_path, monkeypatch, ["src/a.ts"], accept_files={"tests/mia.test.ts": "it('test_R1_x', () => {});\n"})
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "mia.test.ts").write_text("it('test_R1_x', () => {});\n", encoding="utf-8")
    S.state["plan_files"] = ["src/a.ts"]
    pedidos = []

    def fake(model, system, user, timeout=400):
        pedidos.append(system + user)
        raise _Corte
    monkeypatch.setattr(S, "llm", fake)
    with pytest.raises(_Corte):
        S.spec()
    assert "Python" not in pedidos[0] and "TypeScript" in pedidos[0]

    monkeypatch.setattr(S, "_revisor", lambda prompt, **kw: pedidos.append(prompt) or {"ok": True, "returncode": 0})
    monkeypatch.setattr(S, "accept", lambda: (True, "1 passed"))
    S.claude_fix("fallo")
    assert "node --test tests/*.test.ts" in pedidos[-1] and "pytest" not in pedidos[-1]
    assert S._rol() == "Sos un programador TypeScript senior. "
    S.state["plan_files"] = []
    assert S._rol() == "Sos un programador TypeScript senior. "   # sin plan: el lenguaje del repo, no Python
