"""juez_sombra: el juez visual corre por la CLI de Claude con cupo (decision 2026-10-17). Cero API: subprocess falso."""
import json
import types

import mmorch.juez_sombra as J


def test_juez_visual_va_por_la_cli_aislada_con_la_imagen_y_suma_cupo(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path))
    for d in ("a", "b"):
        (tmp_path / d).mkdir()
        for f in ("escritorio.png", "movil.png"):
            (tmp_path / d / f).write_bytes(b"\x89PNG")
    vistos = []

    def run(argv, input, **k):
        vistos.append((argv, json.loads(input)))
        mejor = "primero" if len(vistos) == 1 else "segundo"   # A gana en los dos ordenes
        fin = {"type": "result", "is_error": False, "total_cost_usd": 0.01,
               "result": json.dumps({"mejor": mejor, "motivo": "el neto resalta"})}
        return types.SimpleNamespace(stdout='{"type": "system"}\n' + json.dumps(fin) + "\n", stderr="")

    monkeypatch.setattr(J.subprocess, "run", run)
    d = J.comparar(tmp_path / "a", tmp_path / "b", "visual")
    assert d["mejor"] == "A" and d["modelo"] == "claude:claude-sonnet-5-5" and d["cupo_usd"] == 0.02
    argv, entrada = vistos[0]
    assert argv[argv.index("--model") + 1] == "claude-sonnet-5-5"
    assert "--restricted" in argv and argv[argv.index("--tools") + 1] == ""   # sin hooks del usuario ni herramientas
    bloques = entrada["message"]["content"]
    assert [b["type"] for b in bloques] == ["text", "image", "image", "image", "image"]
    assert bloques[1]["source"] == {"type": "base64", "media_type": "image/png", "data": "iVBORw=="}
