"""Corre el driver sobre las 10 variantes en el contenedor sin red (mismo camino que el SDLC: sandbox.container_argv)
y mide la exactitud balanceada del veredicto contra esperado.json. Uso: .venv/bin/python correr.py"""
import json
import pathlib
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[4]))   # raiz de orchestration: paquete mmorch
from mmorch.sandbox import container_argv, engine  # noqa: E402

AQUI = pathlib.Path(__file__).resolve().parent
IMG = "builder-driver"   # Dockerfile de esta carpeta: Playwright 1.62.0 sobre la imagen oficial
esperado = json.loads((AQUI / "esperado.json").read_text(encoding="utf-8"))
filas = []
for nombre, debe in esperado.items():
    t0 = time.time()
    argv = container_argv(["python", "driver.py", f"productos/{nombre}", "casos.json"], image=IMG, cwd=str(AQUI),
                          mounts=[str(AQUI)], name=f"t05-{nombre}-{time.time_ns()}", eng=engine())
    p = subprocess.run(argv, capture_output=True, text=True, timeout=600)
    try:
        r = json.loads(p.stdout.strip().splitlines()[-1])
    except (ValueError, IndexError):
        r = {"veredicto": None, "error": (p.stdout + p.stderr)[-600:]}
    r.update(nombre=nombre, esperado=debe, segundos=round(time.time() - t0, 1))
    filas.append(r)
    caidos = [k for k, v in r.get("checks", {}).items() if not v]
    print(f"{nombre:24} esperado={debe!s:5} veredicto={r['veredicto']!s:5} {r['segundos']:5}s  falla: {caidos or r.get('error', '')}")

pos = [f for f in filas if f["esperado"]]
neg = [f for f in filas if not f["esperado"]]
tpr = sum(f["veredicto"] is True for f in pos) / len(pos)
tnr = sum(f["veredicto"] is False for f in neg) / len(neg)
print(f"\nexactitud balanceada {(tpr + tnr) / 2:.3f}  (aprueba correctos {tpr:.2f}, rechaza defectuosos {tnr:.2f})")
(AQUI / "resultados.json").write_text(json.dumps(filas, indent=1, ensure_ascii=False), encoding="utf-8")
