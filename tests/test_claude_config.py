"""scripts/claude_config.py: export en una PC e install en otra reescribe las rutas absolutas al home nuevo."""
import importlib.util
import json
from pathlib import Path

_spec = importlib.util.spec_from_file_location("claude_config", Path(__file__).resolve().parents[1] / "scripts" / "claude_config.py")
CC = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(CC)


def test_export_install_mueve_rutas_memoria_y_mcp_sin_secretos(tmp_path):
    a, b, bundle = tmp_path / "pc1" / "ana", tmp_path / "pc2" / "beto", tmp_path / "bundle"
    fa, fb = CC._formas(str(a)), CC._formas(str(b))
    ra = a / ".claude"
    (ra / "hooks").mkdir(parents=True)
    (ra / "settings.json").write_text(json.dumps({"cmd": f'"{a}\\.claude\\hooks\\x.js"'}), encoding="utf-8")
    (ra / "hooks" / "x.js").write_bytes(f'const ORCH = "{fa["fwd"]}/.claude/orchestration";\r\n'.encode())
    mem = ra / "projects" / f"{fa['key']}-Desktop-Claude"
    (mem / "memory").mkdir(parents=True)
    (mem / "memory" / "MEMORY.md").write_text("- [x](x.md)\n", encoding="utf-8")
    (mem / "sesion.jsonl").write_text("{}", encoding="utf-8")  # transcripcion: no viaja
    (ra / ".credentials.json").write_text("tok-a", encoding="utf-8")
    (a / ".claude.json").write_text(json.dumps({
        "mcpServers": {"m": {"command": f"{a}\\.claude\\orchestration\\python.exe", "env": {"API_KEY": "sk-123", "X": "1"}}},
        "projects": {f"{a}\\Desktop\\Portfolio": {"mcpServers": {"p": {"command": f"{fa['fwd']}/Desktop/Portfolio/py"}}}},
    }), encoding="utf-8")

    assert CC.exportar(ra, bundle, str(a), None, a / ".claude.json") == 3
    viajo = "".join(p.read_text(encoding="utf-8") for p in bundle.rglob("*") if p.is_file())
    assert "sk-123" not in viajo and "tok-a" not in viajo and str(a) not in viajo and "ana" not in viajo

    rb = b / ".claude"
    rb.mkdir(parents=True)
    (rb / "settings.json").write_text("{}", encoding="utf-8")
    (rb / ".credentials.json").write_text("tok-b", encoding="utf-8")
    pendientes = CC.instalar(bundle, rb, str(b), None, b / ".claude.json")

    assert json.loads((rb / "settings.json").read_text(encoding="utf-8"))["cmd"] == f'"{b}\\.claude\\hooks\\x.js"'
    assert (rb / "hooks" / "x.js").read_bytes() == f'const ORCH = "{fb["fwd"]}/.claude/orchestration";\r\n'.encode()
    assert (rb / "projects" / f"{fb['key']}-Desktop-Claude" / "memory" / "MEMORY.md").exists()
    assert (rb / ".credentials.json").read_text(encoding="utf-8") == "tok-b"
    assert next(rb.glob("backups/install-*/settings.json")).read_text(encoding="utf-8") == "{}"
    cfg = json.loads((b / ".claude.json").read_text(encoding="utf-8"))
    assert cfg["mcpServers"]["m"] == {"command": f"{b}\\.claude\\orchestration\\python.exe", "env": {"API_KEY": "{{SECRETO}}", "X": "1"}}
    assert cfg["projects"][f"{b}\\Desktop\\Portfolio"]["mcpServers"]["p"]["command"] == f"{fb['fwd']}/Desktop/Portfolio/py"
    assert any("SECRETO" in p for p in pendientes) and any(".env" in p for p in pendientes)
