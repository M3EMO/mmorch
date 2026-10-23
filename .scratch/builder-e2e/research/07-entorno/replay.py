"""Ticket 07: repite la corrida que fallo el 2026-10-15 (m1 de F2 con el bootstrap viejo: tsc chequeaba tests/ y la
imagen no trae los tipos de Node). Antes: 3 vueltas del coder + 2 escaladas a Claude + falla. Ahora la etapa 4 debe
reconocer el error del entorno y mandarlo directo al camino de entorno.
Uso (raiz de orchestration):  SDLC_PLANNER=claude:sonnet .venv/bin/python <este archivo> <clon_en_4a63126> <dir_wt>"""
import json
import pathlib
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[4]
sys.path.insert(0, str(RAIZ))
from sdlc.pipeline import StageFailed, build_feature  # noqa: E402

repo, wt = sys.argv[1], pathlib.Path(sys.argv[2])
contrato = RAIZ / ".scratch/builder-e2e/research/04-modular/contratos/parametros.contract.test.ts"
tarea = ("Crear src/parametros.ts que exporta `PARAMETROS: Parametros` (tipo de src/tipos.ts, importado con extension .ts). "
         "Valores: topeBaseImponible 4769631.49; gananciaNoImponible 5585736.93; deduccionEspecial 26811537.29; aportes "
         "jubilacion 0.11, pami 0.03, obraSocial 0.03. Escala anual (desde, fijo, tasa): (0, 0, 0.05), (2168491.89, 108424.59, "
         "0.09), (4336983.77, 303588.86, 0.12), (6505475.65, 563807.89, 0.15), (9758213.49, 1051718.57, 0.19), "
         "(19516426.99, 2905779.13, 0.23), (29274640.48, 5150168.23, 0.27), (43911960.73, 9102244.70, 0.31), "
         "(65867941.10, 15908598.62, 0.35). vigencia {desde: '2026-07', hasta: '2026-12'}; fuentes: dos URLs https de ARCA.")
try:
    st = build_feature("replay07-m1", tarea, repo, accept={"tests/parametros.contract.test.ts": contrato.read_text()},
                       files=["src/parametros.ts"], wt=wt, phase="replay07-m1")
except StageFailed as e:
    st = {"status": "stage_failed", "failed_stage": e.stage, "failed_note": e.note}
print("RES", json.dumps({k: st.get(k) for k in ("status", "failed_stage", "claude_calls", "escalated_to_claude")}
                        | {"rechazos": [g["gate"] for g in st.get("gate_rejects", [])],
                           "compile_ladder": len(st.get("compile_ladder", []))}, ensure_ascii=False))
