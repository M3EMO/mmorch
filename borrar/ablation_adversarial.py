"""Ablacion §18.4 ADVERSARIO — cerrar la tesis cross-family de verdad.

El run anterior (objetivo, n=54) dio 0 false_pass para ambos: tareas checkeables no
tienen punto ciego de familia. Aca: benchmark HARD de errores SUTILES (donde un
verificador SI miss) + analisis de DECORRELACION + ensemble-vs-ensemble.

Hipotesis a testear (no asumir):
  H1  deepseek-as-verifier (same-family que el autor) tiene blind-spots (false_pass>0).
  H2  gemini (cross-family) CAZA los blind-spots de deepseek (cross-catch alto) -> los
      errores DECORRELACIONAN por familia.
  H3  ensemble cross-family-2 (deepseek+gemini) deja pasar MENOS bugs que ensemble
      same-family-2 (deepseek+deepseek variante) -> el invariante cross-family vale.

Si H2/H3 NO se cumplen, el invariante "cross-family obligatorio" pierde sustento
empirico y la propuesta de escoparlo gana. Anti-sicofancia: dejar que los datos hablen.

Labels hand-authored y verificados (Opus). seed fijo. ~30 casos.
"""
import sys
import pathlib
from dataclasses import dataclass

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.providers import call
from mmorch.patterns import _parse_verdict
from mmorch.config import family_of

R = ("La afirmacion es ENTERAMENTE correcta (factual, de calculo y de logica)? "
     "passed=true SOLO si no tiene ningun error. Refuta si hay cualquier fallo, "
     "por sutil que sea.")


@dataclass
class HC:
    art: str
    truth: bool          # True = la afirmacion ES correcta
    dom: str


# truth=False => hay un error plantado (sutil). truth=True => correcta.
HARD = [
    # --- finanzas (dominio portfolio) ---
    HC("La volatilidad mensual de un activo con vol anual 24% es 24/12 = 2%.", False, "fin-volscale"),
    HC("Un activo que cae 50% necesita subir 100% para volver al valor original.", True, "fin-drawdown"),
    HC("El CAGR de pasar de 100 a 144 en 2 anios es 22% anual.", False, "fin-cagr"),
    HC("Agregar un activo no correlacionado con igual Sharpe sube el Sharpe del portfolio.", True, "fin-diversif"),
    HC("Con retorno nominal 30% e inflacion 25%, el retorno real EXACTO es 5%.", False, "fin-real"),
    HC("Diversificar reduce el riesgo no-sistematico pero no el sistematico.", True, "fin-systematic"),
    HC("Interes compuesto: 100 al 10% anual durante 3 anios da 133.1.", True, "fin-compound"),
    # --- probabilidad / estadistica ---
    HC("Es mas probable que una persona sea cajera Y activista que solo cajera.", False, "prob-conjuncion"),
    HC("Test 99% preciso, enfermedad en 1 de 10000: un positivo implica ~99% de estar enfermo.", False, "prob-baserate"),
    HC("Con 23 personas, la probabilidad de un cumpleanios compartido supera 50%.", True, "prob-birthday"),
    HC("Tras salir cara 5 veces seguidas, la proxima tirada tiene mas chance de salir ceca.", False, "prob-gambler"),
    HC("La mediana es mas robusta a outliers que la media.", True, "stat-mediana"),
    HC("Un p-valor de 0.03 rechaza H0 a un nivel de significancia del 5%.", True, "stat-pvalue"),
    HC("El desvio estandar es la raiz cuadrada de la varianza.", True, "stat-std"),
    # --- codigo python (sutiles) ---
    HC("def f(x, l=[]): l.append(x); return l. Llamar f(1) y luego f(2) devuelve [2].", False, "py-mutdefault"),
    HC("En Python, 0.1 + 0.2 == 0.3 evalua a True.", False, "py-float"),
    HC("list(range(1,5)) es igual a [1,2,3,4].", True, "py-range"),
    HC("Para comparar si dos floats son iguales en Python conviene usar el operador ==.", False, "py-floateq"),
    HC("En Python {} crea un diccionario vacio, no un set.", True, "py-dict"),
    # --- logica ---
    HC("Todos los X son Y. Z es Y. Por lo tanto Z es X.", False, "logic-consequent"),
    HC("Si llueve la calle se moja. La calle esta mojada. Por lo tanto llovio.", False, "logic-affirm"),
    HC("P implica Q es logicamente equivalente a no-Q implica no-P.", True, "logic-contrapos"),
    HC("La negacion de 'todos los cisnes son blancos' es 'ningun cisne es blanco'.", False, "logic-negacion"),
    # --- factual near-miss ---
    HC("El Everest es la montania mas alta medida desde el centro de la Tierra.", False, "fact-everest"),
    HC("La luz del Sol tarda aproximadamente 8 minutos en llegar a la Tierra.", True, "fact-luz"),
    HC("Un anio luz mide el tiempo que tarda la luz en viajar durante un anio.", False, "fact-anioluz"),
    HC("La correlacion no implica causalidad.", True, "fact-corr"),
    # --- unidades / geometria ---
    HC("100 km/h equivale aproximadamente a 27.8 m/s.", True, "unit-speed"),
    HC("Duplicar el radio de un circulo duplica su area.", False, "geom-area"),
    HC("Un cubo de lado 2 tiene volumen 6.", False, "geom-cubo"),
]

SKEPTIC_A = (
    "Sos un verificador adversarial. Refuta por default; el acuerdo no es confirmacion. "
    "Chequea factual, calculo y logica. Respondé SOLO JSON: "
    '{"passed": bool, "confidence": 0..1, "refutations": [string]}')
SKEPTIC_B = (
    "Actua como revisor critico independiente. Asumi que la afirmacion tiene un error "
    "escondido hasta probar lo contrario. Verifica numeros y la validez logica paso a paso. "
    'Devolve SOLO JSON: {"passed": bool, "confidence": 0..1, "refutations": [string]}')


def verify(model, art, system, temp):
    res = call(model, [{"role": "system", "content": system},
                       {"role": "user", "content": f"RUBRIC:\n{R}\n\nAFIRMACION:\n{art}\n\nJSON:"}],
               pattern="ablation-adv", node=f"v:{model}", phase="ablation-adv", temperature=temp)
    passed, conf, refs = _parse_verdict(res.text)
    return passed, res.cost_usd


def main():
    author = "deepseek-chat"
    n_bug = sum(1 for c in HARD if not c.truth)
    print(f"benchmark HARD: {len(HARD)} casos ({len(HARD)-n_bug} correctos, {n_bug} con error sutil)")
    print(f"author family: {family_of(author)} | cross = google (gemini)\n")

    rows = []  # por caso: verdicts de cada verificador
    cost = 0.0
    for c in HARD:
        dA, ca = verify("deepseek-chat", c.art, SKEPTIC_A, 0.0)     # same-family A
        dB, cb = verify("deepseek-chat", c.art, SKEPTIC_B, 0.4)     # same-family B (variante)
        g, cg = verify("gemini-2.5-flash", c.art, SKEPTIC_A, 0.0)   # cross-family
        cost += ca + cb + cg
        rows.append({"c": c, "dA": dA, "dB": dB, "g": g})

    # ---- single-verifier ----
    def acc(key):
        return sum(1 for r in rows if r[key] == r["c"].truth) / len(rows)
    # false_pass = bug (truth False) que el verificador dejo pasar (passed True)
    def fpset(key):
        return {r["c"].dom for r in rows if (not r["c"].truth) and r[key]}
    fp_dA, fp_dB, fp_g = fpset("dA"), fpset("dB"), fpset("g")

    print("=== SINGLE VERIFIER ===")
    print(f"deepseek-A (SAME):  acc={acc('dA'):.2f}  false_pass={len(fp_dA)}/{n_bug}  -> {sorted(fp_dA)}")
    print(f"deepseek-B (SAME'): acc={acc('dB'):.2f}  false_pass={len(fp_dB)}/{n_bug}  -> {sorted(fp_dB)}")
    print(f"gemini   (CROSS):   acc={acc('g'):.2f}  false_pass={len(fp_g)}/{n_bug}  -> {sorted(fp_g)}")

    # ---- decorrelacion ----
    def jac(a, b):
        return len(a & b) / len(a | b) if (a | b) else 1.0
    print("\n=== DECORRELACION DE BLIND-SPOTS (false_pass) ===")
    print(f"intra-familia  Jaccard(deepseekA, deepseekB) = {jac(fp_dA, fp_dB):.2f}  (alto = correlacionados)")
    print(f"inter-familia  Jaccard(deepseekA, gemini)    = {jac(fp_dA, fp_g):.2f}  (bajo = decorrelacionados)")
    # cross-catch: de los bugs que deepseek-A dejo pasar, cuantos cazo gemini?
    caught = fp_dA - fp_g  # deepseek miss y gemini NO miss = gemini lo cazo
    print(f"cross-catch: gemini cazo {len(caught)}/{len(fp_dA)} blind-spots de deepseek-A "
          f"-> {sorted(caught)}")
    same_catch = fp_dA - fp_dB  # deepseek-B (misma familia) cazo de los de A
    print(f"same-catch:  deepseek-B cazo {len(same_catch)}/{len(fp_dA)} blind-spots de deepseek-A")

    # ---- ensemble (minority-veto: passed solo si TODOS aprueban) ----
    def ens_fp(keys):
        s = set()
        for r in rows:
            if not r["c"].truth and all(r[k] for k in keys):  # todos lo dejaron pasar
                s.add(r["c"].dom)
        return s
    same2 = ens_fp(["dA", "dB"])      # same-family ensemble
    cross2 = ens_fp(["dA", "g"])      # cross-family ensemble
    print("\n=== ENSEMBLE-vs-ENSEMBLE (minority-veto) ===")
    print(f"same-family-2  (deepseekA + deepseekB): false_pass={len(same2)}/{n_bug}  -> {sorted(same2)}")
    print(f"cross-family-2 (deepseekA + gemini):    false_pass={len(cross2)}/{n_bug}  -> {sorted(cross2)}")

    print(f"\ncosto total run: ${cost:.4f} ({len(HARD)*3} calls)")


if __name__ == "__main__":
    main()
