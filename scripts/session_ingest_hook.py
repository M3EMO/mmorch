"""SessionEnd hook — mina workflow playbooks de la sesion que acaba de terminar.

100% LOCAL: corre `ingest_workflows`, que NO llama a ninguna API externa (parse +
outcome determinista + agregacion). NO toca la calibracion del router (que si mandaria
el request a Gemini) — esa queda manual/opt-in. Best-effort: cualquier error sale 0,
nunca bloquea el cierre de la sesion.

Recibe el payload del hook por stdin; usa el path explicito de la sesion terminada
(salta el cooldown de _resolve_latest, que es solo para el modo "latest").
"""
import json
import pathlib
import sys


def main() -> int:
    try:
        payload = json.loads(sys.stdin.read() or "{}")
    except Exception:
        return 0
    tp = payload.get("transcript_path") or payload.get("transcriptPath")
    if not tp or not pathlib.Path(tp).exists():
        return 0
    try:
        sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
        from mmorch.session_skills import ingest_workflows
        n = ingest_workflows(tp)
        sys.stderr.write(f"mmorch session-ingest: +{n} workflow obs from {pathlib.Path(tp).name}\n")
    except Exception as e:                       # nunca bloquear el cierre
        sys.stderr.write(f"mmorch session-ingest skipped: {type(e).__name__}: {e}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
