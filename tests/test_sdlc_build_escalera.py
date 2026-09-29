"""orchestration-7ys: la etapa build escala (reasoner x2 -> Claude) cuando no compila, y va directo a Claude
cuando falta el compilador. Cero API: llm y el revisor son dobles; el "compilador" es un script Python."""
import json
import types

import mmorch.sdlc as S

ROTO = "export const f = (x: number): number => ROTO;\n"
BIEN = "export const f = (x: number): number => x + 1;\n"
COMPILA = "python -c \"import sys; sys.exit(1 if 'ROTO' in open('src/a.ts', encoding='utf-8').read() else 0)\""
SIN_TSC = "python -c \"import sys; print('To get access to the TypeScript compiler, tsc, run npm install'); sys.exit(1)\""


def _wt(tmp_path, monkeypatch, compile_cmd):
    (tmp_path / "src").mkdir(parents=True, exist_ok=True)
    (tmp_path / "src" / "a.ts").write_text(ROTO, encoding="utf-8")
    t = types.SimpleNamespace(name="b", task="sumar uno", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": ["src/a.ts"], "suite": []}, wt=tmp_path,
                accept_cmd="python -c \"pass\"")
    S.CFG["ext"] = ["ts"]
    S.CFG["compile_cmd"] = compile_cmd
    S.state["plan_files"] = ["src/a.ts"]
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")
    return lambda: S.gate_compile(S.state["plan_files"])


def _llm(coder_code):
    def fake(model, system, user, timeout=400):
        if "diagnosticador" in system:
            return json.dumps({"files": ["src/a.ts"], "instructions": {"src/a.ts": "reemplaza ROTO"}})
        return coder_code
    return fake


def test_rol_y_compilador_ausente():
    S.state["plan_files"] = ["src/a.ts", "src/b.tsx", "x.py"]
    assert S._rol() == "Sos un programador TypeScript senior. "
    S.state["plan_files"] = ["Main.java"]
    assert S._rol().startswith("Sos un programador Java")
    assert S._toolchain_missing("'tsc' no se reconoce como un comando interno o externo")
    assert S._toolchain_missing("bash: mvn: command not found")
    assert not S._toolchain_missing("src/a.ts(3,1): error TS2322: Type 'string' is not assignable to type 'number'.")


def test_escalera_de_build_arregla_con_el_reasoner(tmp_path, monkeypatch):
    compiles = _wt(tmp_path, monkeypatch, COMPILA)
    monkeypatch.setattr(S, "llm", _llm(BIEN))
    ok, nota, _ = S._compile_ladder("error: ROTO no existe", compiles)
    assert ok and nota == "reasoner build 1 compila" and not S.state["escalated_to_claude"]
    assert list((tmp_path / "runs" / "diag-cases").glob("*-compile-*.json"))   # caso replayable


def test_escalera_de_build_llega_a_claude(tmp_path, monkeypatch):
    compiles = _wt(tmp_path, monkeypatch, COMPILA)
    monkeypatch.setattr(S, "llm", _llm(ROTO))   # el coder no logra arreglarlo
    pedidos = []

    def claude(prompt, **kw):
        pedidos.append(prompt)
        (tmp_path / "src" / "a.ts").write_text(BIEN, encoding="utf-8")
        return {"ok": True, "result": "arregle a.ts", "returncode": 0}
    monkeypatch.setattr(S, "_revisor", claude)
    ok, nota, _ = S._compile_ladder("error: ROTO no existe", compiles)
    assert ok and nota == "Claude build compila" and S.state["escalated_to_claude"]
    assert "no compila" in pedidos[0] and "src/a.ts" in pedidos[0]


def test_sin_compilador_claude_arregla_el_entorno(tmp_path, monkeypatch):
    compiles = _wt(tmp_path, monkeypatch, SIN_TSC)
    (tmp_path / "src" / "a.ts").write_text(BIEN, encoding="utf-8")
    ok, log = compiles()
    assert not ok and S._toolchain_missing(log)

    def claude(prompt, **kw):   # corrige compile_cmd en sdlc.toml, sin tocar el codigo
        (tmp_path / "sdlc.toml").write_text(f"compile_cmd = {json.dumps(COMPILA)}\n", encoding="utf-8")
        return {"ok": True, "result": "compile_cmd corregido", "returncode": 0}
    monkeypatch.setattr(S, "_revisor", claude)
    ok, _ = S._claude_compile(log, compiles, env=True)
    assert ok and S.CFG["compile_cmd"] == COMPILA and S.state["escalated_to_claude"]


def test_build_cableado_llega_a_claude(tmp_path, monkeypatch):
    compiles = _wt(tmp_path, monkeypatch, COMPILA)
    del compiles
    (tmp_path / "docs" / "sdlc").mkdir(parents=True)
    (tmp_path / "docs" / "sdlc" / "spec.md").write_text("spec", encoding="utf-8")
    (tmp_path / "docs" / "sdlc" / "plan.md").write_text("plan", encoding="utf-8")
    monkeypatch.setattr(S, "llm", _llm(ROTO))

    def claude(prompt, **kw):
        (tmp_path / "src" / "a.ts").write_text(BIEN, encoding="utf-8")
        return {"ok": True, "result": "ok", "returncode": 0}
    monkeypatch.setattr(S, "_revisor", claude)
    monkeypatch.setattr(S, "gate_test_compile", lambda: (True, "sin tests"))
    S.build()   # el decorador @stage registra la etapa en state["stages"]
    etapa = S.state["stages"][-1]
    assert etapa["stage"] == "4-build" and etapa["ok"] and S.state["escalated_to_claude"]
    assert S.state.get("compile_ladder") and not any(x["ok"] for x in S.state["compile_ladder"])
