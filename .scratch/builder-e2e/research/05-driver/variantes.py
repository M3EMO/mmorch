"""Diez sitios de la calculadora (ticket 05): 3 correctos con formas distintas y 7 con un defecto plantado cada uno.
Contrato fijo del walkthrough: input #bruto, boton [data-testid=calcular], salida [data-testid=neto]."""
import json
import pathlib

AQUI = pathlib.Path(__file__).parent
ESC_JS = json.dumps([[0, 0, .05], [2168491.89, 108424.59, .09], [4336983.77, 303588.86, .12],
                     [6505475.65, 563807.89, .15], [9758213.49, 1051718.57, .19], [19516426.99, 2905779.13, .23],
                     [29274640.48, 5150168.23, .27], [43911960.73, 9102244.70, .31], [65867941.10, 15908598.62, .35]])

CALC_OK = f"""
const TOPE = 4769631.49, GNI = 5585736.93, DE2 = 26811537.29, ESC = {ESC_JS};
function impuesto(b) {{ if (b <= 0) return 0; const [d, f, t] = ESC.filter(e => b >= e[0]).pop(); return f + t * (b - d); }}
function neto(g) {{ const ap = APORTE * Math.min(g, TOPE); return g - ap - impuesto(12 * (g - ap) - GNI - DE2) / 12; }}
"""
HEAD_OK = ('<meta name="description" content="Calculá tu sueldo neto en Argentina: aportes de jubilación, PAMI, obra '
           'social y Ganancias con las tablas de ARCA de julio a diciembre de 2026.">\n'
           '<link rel="canonical" href="https://ejemplo.com.ar/sueldo-neto">')
FMT_PLANO = "n.toFixed(2)"
FMT_AR = "'$ ' + n.toLocaleString('es-AR', {minimumFractionDigits: 2, maximumFractionDigits: 2})"


def pagina(calc, *, aporte="0.17", fmt=FMT_PLANO, head=HEAD_OK, lang="es-AR", extra_js="", h1="Calculadora de sueldo neto"):
    return f"""<!doctype html>
<html lang="{lang}"><head><meta charset="utf-8"><title>Calculadora de sueldo neto 2026 en Argentina</title>
{head}
</head><body>
<h1>{h1}</h1>
<label>Sueldo bruto mensual <input id="bruto" type="number" step="0.01"></label>
<button data-testid="calcular">Calcular</button>
<p>Neto: <output data-testid="neto"></output></p>
<script>
const APORTE = {aporte};
{calc}
document.querySelector('[data-testid=calcular]').onclick = () => {{
  const n = neto(parseFloat(document.getElementById('bruto').value));
  document.querySelector('[data-testid=neto]').textContent = {fmt};
}};
{extra_js}
</script></body></html>
"""


SIN_TOPE = CALC_OK.replace("Math.min(g, TOPE)", "g")
TRAMO_MAL = CALC_OK.replace("e => b >= e[0]", "e => b > e[0] * 1.1")   # corre los bordes de la escala
HARDCODE = """
const PUBLICOS = {1500000: 1245000, 3000000: 2490000, 3500000: 2893757.86, 5000000: 3973040.42, 10000000: 7513307.95};
function neto(g) { return PUBLICOS[g] ?? g * 0.83; }   // pasa los ejemplos de la spec y nada mas
"""

VARIANTES = {   # nombre -> (html, veredicto esperado)
    "ok_plano": (pagina(CALC_OK), True),
    "ok_formato_ar": (pagina(CALC_OK, fmt=FMT_AR), True),
    "ok_h1_distinto": (pagina(CALC_OK, h1="Sueldo neto: cuánto cobrás en mano"), True),
    "bug_sin_tope": (pagina(SIN_TOPE), False),
    "bug_tramo": (pagina(TRAMO_MAL), False),
    "bug_aporte_16": (pagina(CALC_OK, aporte="0.16"), False),
    "bug_hardcode_publicos": (pagina(HARDCODE), False),
    "bug_consola": (pagina(CALC_OK, extra_js="console.error('fallo al cargar tablas');"), False),
    "bug_externo": (pagina(CALC_OK, head=HEAD_OK + '\n<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter">'), False),
    "bug_meta": (pagina(CALC_OK, head="", lang="en"), False),
}

if __name__ == "__main__":
    for nombre, (html, _) in VARIANTES.items():
        d = AQUI / "productos" / nombre
        d.mkdir(parents=True, exist_ok=True)
        (d / "index.html").write_text(html, encoding="utf-8")
    (AQUI / "esperado.json").write_text(json.dumps({k: v for k, (_, v) in VARIANTES.items()}, indent=1), encoding="utf-8")
    print(len(VARIANTES), "variantes")
