"""builder-e2e ticket 10: un coder `claude:<modelo>` corre por la CLI de Claude, aislado y sin herramientas. Cero API."""
import json
import types

import mmorch.sdlc as S


def test_coder_claude_va_por_la_cli_aislada_y_suma_su_cupo(tmp_path, monkeypatch):
    t = types.SimpleNamespace(name="c", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": ["a.py"], "suite": []}, wt=tmp_path,
                accept_cmd="python -c pass", coder="claude:claude-haiku-5-5")
    vistos = []

    def fake_run(argv, prompt, env, timeout):
        vistos.append((argv, prompt))
        return 0, json.dumps({"result": "print(1)\n", "total_cost_usd": 0.0123})

    monkeypatch.setattr(S, "_agent_run", fake_run)
    assert S.llm(S.CODER, "Sos un programador Python senior.", "escribi a.py") == "print(1)\n"
    S.llm(S.CODER, "sys", "otra vez")
    argv, prompt = vistos[0]
    assert argv[argv.index("--model") + 1] == "claude-haiku-5-5"
    assert "--restricted" in argv and argv[argv.index("--tools") + 1] == ""   # sin hooks del usuario y sin herramientas
    assert argv[argv.index("--system-prompt") + 1] == "Sos un programador Python senior." and prompt == "escribi a.py"
    assert S.state["claude_coder_usd"] == 0.0246 and S.state["coder"] == "claude:claude-haiku-5-5"
