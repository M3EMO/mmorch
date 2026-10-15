"""Prototipo del ticket 04: tres modulos en cadena por el SDLC real y una integracion final con el driver del ticket 05.
El encadenado (merge de cada rama en main) lo hace este script: es el hueco 3 del ticket 02.
Uso (desde la raiz de orchestration):  SDLC_PLANNER=claude:sonnet .venv/bin/python <este archivo>"""
import json
import pathlib
import subprocess
import sys
import time

RAIZ = pathlib.Path(__file__).resolve().parents[4]
sys.path.insert(0, str(RAIZ))
from mmorch.sandbox import container_argv, engine  # noqa: E402
from mmorch.sdlc import StageFailed, build_feature  # noqa: E402

AQUI = pathlib.Path(__file__).resolve().parent
REPO = pathlib.Path.home() / "Proyectos/F2-seo-calculadoras/prototipo-04"
WTS = pathlib.Path("/tmp/claude-1000/-home-mpera--claude-orchestration/176fc735-f46a-46e8-a9a4-710e23d080af/scratchpad/wt04")
CONTRATOS = AQUI / "contratos"
FUENTES = ("https://www.afip.gob.ar/gananciasYBienes/ganancias/personas-humanas-sucesiones-indivisas/deducciones/documentos/"
           "Deducciones-personales-art-30-jul-dic-2026.pdf y https://www.afip.gob.ar/gananciasYBienes/ganancias/"
           "personas-humanas-sucesiones-indivisas/declaracion-jurada/documentos/Tabla-Art-94-LIG-per-jul-a-dic-2026.pdf")

MODULOS = [   # orden de la rubrica valor x esfuerzo x riesgo: la verdad primero, el motor despues, la interfaz al final
    ("m1-parametros", ["src/parametros.ts"], "parametros.contract.test.ts",
     "Crear src/parametros.ts que exporta `PARAMETROS: Parametros` (tipo de src/tipos.ts, importado con extension .ts). "
     "Valores aprobados en el walkthrough: topeBaseImponible 4769631.49 (ANSES oct 2026); gananciaNoImponible 5585736.93 y "
     "deduccionEspecial 26811537.29 (ARCA art. 30, jul-dic 2026); aportes jubilacion 0.11, pami 0.03, obraSocial 0.03. "
     "Escala anual del art. 94 (desde, fijo, tasa): (0, 0, 0.05), (2168491.89, 108424.59, 0.09), (4336983.77, 303588.86, 0.12), "
     "(6505475.65, 563807.89, 0.15), (9758213.49, 1051718.57, 0.19), (19516426.99, 2905779.13, 0.23), "
     "(29274640.48, 5150168.23, 0.27), (43911960.73, 9102244.70, 0.31), (65867941.10, 15908598.62, 0.35). "
     f"vigencia {{desde: '2026-07', hasta: '2026-12'}}; fuentes: {FUENTES}."),
    ("m2-calculo", ["src/calculo.ts"], "calculo.contract.test.ts",
     "Crear src/calculo.ts que exporta `calcularNeto: CalcularNeto` (src/tipos.ts). Convencion aprobada: base de aportes = "
     "min(bruto, topeBaseImponible); cada aporte = tasa x base; base anual de Ganancias = 12 x (bruto - aportes) - "
     "gananciaNoImponible - deduccionEspecial; si es <= 0 no hay impuesto; si no, impuesto anual = fijo + tasa x (base - desde) "
     "del tramo con mayor `desde` <= base; ganancias mensual = impuesto anual / 12; neto = bruto - aportes - ganancias. "
     "Sin SAC, soltero, sin hijos. Funcion pura, sin redondeos intermedios."),
    ("m3-interfaz", ["src/app.ts", "index.html"], "ui.contract.test.ts",
     "Crear index.html (lang es-AR) y src/app.ts. La pagina tiene un solo h1, title de 30 a 60 caracteres, meta description "
     "de 70 a 160 caracteres, link canonical https://ejemplo.com.ar/sueldo-neto, un input id='bruto', un boton "
     "data-testid='calcular' y salidas data-testid='neto', 'jubilacion', 'pami', 'obra-social' y 'ganancias' en pesos "
     "con formato es-AR. Carga solo <script type=\"module\" src=\"dist/app.js\"></script> (dist sale de tsc -p "
     "tsconfig.build.json); nada externo: sin fuentes, CDN ni analytics. app.ts usa calcularNeto y PARAMETROS al hacer "
     "click en calcular. Texto breve en español que explica los aportes y Ganancias con la fecha de vigencia de las tablas."),
]

CAMPOS = ("status", "failed_stage", "failed_note", "usd_run", "claude_calls", "escalated_to_claude", "planner", "planner_usd")


def git(*a):
    return subprocess.run(["git", "-C", str(REPO), *a], capture_output=True, text=True)


filas = []
for nombre, files, contrato, tarea in MODULOS:
    t0 = time.time()
    accept = {f"tests/{contrato}": (CONTRATOS / contrato).read_text(encoding="utf-8")}
    try:
        st = build_feature(nombre, tarea, str(REPO), accept=accept, files=files, wt=WTS / nombre, phase=f"f2-{nombre}")
    except StageFailed as e:
        st = {"status": "stage_failed", "failed_stage": e.stage, "failed_note": e.note}
    fila = {k: st.get(k) for k in CAMPOS} | {
        "modulo": nombre, "minutos": round((time.time() - t0) / 60, 1),
        "gate_rejects": len(st.get("gate_rejects", [])), "rechazos": [g["gate"] for g in st.get("gate_rejects", [])],
        "compile_ladder": len(st.get("compile_ladder", [])), "reasoner_rounds": len(st.get("reasoner_rounds", [])),
        "test_rounds": len(st.get("test_rounds", []))}
    filas.append(fila)
    print(json.dumps(fila, ensure_ascii=False), flush=True)
    (AQUI / "resultados.json").write_text(json.dumps(filas, indent=1, ensure_ascii=False), encoding="utf-8")
    if st.get("status") != "built":
        print("corte: el modulo no quedo construido", flush=True)
        break
    m = git("-c", "user.name=Mateo", "-c", "user.email=map12082004@gmail.com", "merge", "--no-ff", "-m",
            f"merge {nombre} (encadenado manual del prototipo)", f"sdlc/f2-{nombre}")
    git("worktree", "remove", "--force", str(WTS / nombre))
    if m.returncode:
        print("corte: merge fallo", m.stdout + m.stderr, flush=True)
        break
else:   # integracion: compila el sitio y lo maneja el driver del ticket 05 contra los 70 casos ocultos
    t05 = AQUI.parent / "05-driver"
    for argv in (container_argv("tsc -p tsconfig.build.json", image="f2-sdlc", cwd=str(REPO), mounts=[str(REPO)],
                                name=f"t04-build-{time.time_ns()}", eng=engine()),
                 container_argv(["python", str(t05 / "driver.py"), str(REPO), str(t05 / "casos.json")], image="builder-driver",
                                cwd=str(t05), mounts=[str(t05), str(REPO)], name=f"t04-driver-{time.time_ns()}", eng=engine())):
        p = subprocess.run(argv, capture_output=True, text=True, timeout=900)
        print((p.stdout + p.stderr)[-2500:], flush=True)
    filas.append({"integracion": (p.stdout.strip().splitlines() or ["{}"])[-1]})
    (AQUI / "resultados.json").write_text(json.dumps(filas, indent=1, ensure_ascii=False), encoding="utf-8")
