"""orchestration-34k: el planner de la etapa 3 es configurable (SDLC_PLANNER). Vacio = el WRITER en una llamada;
`claude:` y `cursor:` corren un agente de solo lectura en el worktree. Cero API: el agente y llm son dobles."""
import json
import os
import subprocess
import sys
import types

import pytest

import mmorch.sdlc as S


def _wt(tmp_path, monkeypatch):
    t = types.SimpleNamespace(name="p", task="planear", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": ["src/a.ts"], "suite": []}, wt=tmp_path,
                accept_cmd="python -c \"pass\"")
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")
    (tmp_path / "runs").mkdir(exist_ok=True)


def test_sin_planner_usa_el_writer(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    monkeypatch.delenv("SDLC_PLANNER", raising=False)
    vistos = []
    monkeypatch.setattr(S, "llm", lambda model, system, user, timeout=400: vistos.append(model) or "## Archivos")
    assert S._planner("sys", "user") == "## Archivos" and vistos == [S.WRITER]


def test_planner_claude_solo_lectura(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    monkeypatch.setenv("SDLC_PLANNER", "claude:sonnet")
    monkeypatch.setenv("CLAUDECODE", "1")
    vistos = {}

    def fake(argv, prompt, env, timeout):
        vistos.update(argv=argv, env=env, prompt=prompt)
        return 0, json.dumps({"result": "## Archivos\n- `src/a.ts`", "total_cost_usd": 0.05})
    monkeypatch.setattr(S, "_agent_run", fake)
    assert S._planner("sys", "SPEC") == "## Archivos\n- `src/a.ts`"
    a = vistos["argv"]
    assert a[a.index("--model") + 1] == "sonnet" and "--restricted" in a and "--strict-mcp-config" in a
    assert a[a.index("--tools") + 1] == "Read,Grep,Glob"
    assert "CLAUDECODE" not in vistos["env"] and "SPEC" in vistos["prompt"]
    assert S.state["planner"] == "claude:sonnet" and S.state["planner_usd"] == 0.05


def test_planner_cursor_toma_create_plan_y_bloquea_mcp(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    monkeypatch.setenv("SDLC_PLANNER", "cursor:grok-4.7-medium")
    monkeypatch.setattr(S, "_cursor_argv", lambda: ["node", "index.js"])
    vistos = {}

    def fake(argv, prompt, env, timeout):
        vistos["cli"] = json.loads((tmp_path / ".cursor" / "cli.json").read_text(encoding="utf-8"))
        vistos["argv"] = argv
        out = [json.dumps({"type": "tool_call", "subtype": "completed",
                           "tool_call": {"createPlanToolCall": {"args": {"plan": "## Archivos\n- `src/a.ts`"}}}}),
               json.dumps({"type": "result", "result": "listo", "usage": {"inputTokens": 10, "outputTokens": 5}})]
        return 0, "\n".join(out)
    monkeypatch.setattr(S, "_agent_run", fake)
    assert S._planner("sys", "SPEC") == "## Archivos\n- `src/a.ts`"
    assert vistos["cli"]["permissions"]["deny"] == ["Mcp(*:*)"]
    a = vistos["argv"]
    assert a[a.index("--mode") + 1] == "plan" and a[a.index("--model") + 1] == "grok-4.7-medium"
    assert not (tmp_path / ".cursor").exists() and S.state["planner_tokens"] == 15


def test_falla_tecnica_del_planner_es_runtimeerror(tmp_path, monkeypatch):
    """Cupo agotado o salida vacia = falla tecnica (como una API caida), nunca un plan malo que cuenta en contra."""
    _wt(tmp_path, monkeypatch)
    monkeypatch.setenv("SDLC_PLANNER", "claude:sonnet")
    monkeypatch.setattr(S, "_agent_run", lambda *a: (0, json.dumps({"is_error": True, "result": "usage limit"})))
    with pytest.raises(RuntimeError, match="usage limit"):
        S._planner("sys", "SPEC")
    monkeypatch.setenv("SDLC_PLANNER", "cursor:grok-4.7-medium")
    monkeypatch.setattr(S, "_cursor_argv", lambda: ["node", "index.js"])
    monkeypatch.setattr(S, "_agent_run", lambda *a: (0, ""))
    with pytest.raises(RuntimeError, match="sin plan"):
        S._planner("sys", "SPEC")
    assert not (tmp_path / ".cursor").exists()


def test_agent_run_timeout_mata_y_falla(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    with pytest.raises(RuntimeError, match="timeout"):
        S._agent_run([sys.executable, "-c", "import time; time.sleep(30)"], None, dict(os.environ), 1)


def test_to_stage_corta_antes(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    corridas = []
    for nombre in ("aceptacion", "spec", "spec_review", "plan", "build", "test", "review", "pr"):
        monkeypatch.setattr(S, nombre, lambda n=nombre: corridas.append(n))
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base"],
                   cwd=tmp_path, check=True)
    S.run(from_stage=3, to_stage=5)
    assert corridas == ["plan", "build", "test"]
