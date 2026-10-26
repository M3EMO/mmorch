"""Chequeos de los gates del SDLC (bd orchestration-bpn, 2026-10-23).

Un gate tiene tres partes. El chequeo decide verde o rojo y vive aca. La medicion (precision del chequeo sobre casos
etiquetados) vive en `mmorch/gates_bench.py`. La orquestacion (cuando corre el gate y que hace el pipeline con un rojo)
queda en `sdlc/pipeline.py`, que llama a estas funciones y registra el resultado con `rec_gate`.

Contrato: cada chequeo es una funcion pura con datos explicitos y devuelve `(ok, detalle)`, el contrato de `rec_gate`.
Este modulo importa solo stdlib y `mmorch.checkers`; nunca importa `sdlc` (R6 de tests/test_capas.py).
Decisiones: brainstorms/2026-10-23-gates-a-mmorch.md.
"""
from __future__ import annotations

import re
from collections import Counter


# orchestration-cuz (gates_bench, 5 semillas x 2000 casos): las copias con hasta 30% editado comparten >= 0.69 de
# lineas y los modulos hermanos <= 0.65; con 0.8 el gate dejaba pasar casi todas las copias con 20-30% editado.
# shortcut: medido sobre modulos Python sinteticos; revisar con hermanos reales (tests, DTOs Java) si aparece un falso rojo
UMBRAL_CLON = 0.66


def sin_clones(written: dict[str, str], umbral: float = UMBRAL_CLON) -> tuple[bool, str]:
    """D6: el coder escribio el contenido de plugins.py dentro de plugin_worker.py. Dos archivos del plan
    que comparten mas de `umbral` de sus lineas son un clon, no una feature. Determinista, USD 0."""
    files = list(written)
    for i, a in enumerate(files):
        la = {l.strip() for l in written[a].splitlines() if l.strip()}
        for b in files[i + 1:]:
            lb = {l.strip() for l in written[b].splitlines() if l.strip()}
            if la and lb and len(la & lb) / min(len(la), len(lb)) > umbral:
                return False, f"{a} y {b} comparten > {umbral:.0%} de lineas"
    return True, "ok"


def clarificaciones(spec_md: str) -> tuple[bool, str]:
    """Salida de la revision de spec (ticket 11, de clarify.md): seccion con 0..5 preguntas respondidas."""
    m = re.search(r"## Clarificaciones\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    if not m:
        return False, "falta ## Clarificaciones"
    qs = re.findall(r"(?m)^- P\d+: .+\| R: \S.*\| afecta: \S+", m.group(1))  # D11-diag: "afecta: Contrato" tambien vale
    # orchestration-96p: una pregunta sin respuesta (o con TBD) no se tapa con otra bien escrita
    abiertas = [l for l in re.findall(r"(?m)^- (P\d+):.*$", m.group(1))
                if not re.search(rf"(?m)^- {l}: .+\| R: (?!(?i:tbd|\?+|pendiente)\s*\|)\S.*\| afecta: \S+", m.group(1))]
    if abiertas:
        return False, f"preguntas sin respuesta: {abiertas}"
    if not qs and re.search(r"sin preguntas|0 preguntas|ninguna pregunta|no hay preguntas", m.group(1), re.I):
        return True, "0 preguntas"  # D11-diag: Claude escribio "0 preguntas" en vez del literal
    if not 1 <= len(qs) <= 5:
        return False, f"{len(qs)} preguntas validas (esperado 1..5)"
    return True, f"{len(qs)} preguntas respondidas"


def filas_tabla(block: str) -> list[str]:
    """Filas de una tabla markdown (sin los `|` de los bordes)."""
    return re.findall(r"(?m)^\s*\|(.*)\|\s*$", block)


def traza_plan(spec_md: str, plan_block: str, files: list[str]) -> tuple[bool, str]:
    """Trazabilidad lado plan: cada R<n> de la spec aparece en algun item; cada archivo cita >= 1 R<n> de la spec. Un
    item es una linea de lista o una fila de tabla; un rango `R2–R10` cita R2..R10 (SSB F7a). Una mencion en prosa no
    cubre un ID y un ID que la spec no tiene no traza un archivo (orchestration-1tj)."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    plan_block = re.sub(r"\bR(\d+)\s*[–-]\s*R(\d+)\b",
                        lambda m: ", ".join(f"R{i}" for i in range(int(m[1]), int(m[2]) + 1)), plan_block)
    items = re.findall(r"(?m)^\s*[-*]\s*(.*)$", plan_block) + filas_tabla(plan_block)
    citados = {r for it in items for r in re.findall(r"\bR\d+\b", it)}
    huerfanos = sorted(ids - citados)

    def nombra(it: str, f: str) -> bool:  # la ruta, o su nombre suelto si el plan lo escribio sin carpeta (F4b)
        return f in it or bool(re.search(r"(?<![\w/.-])" + re.escape(f.rsplit("/", 1)[-1]) + r"(?![\w.-])", it))
    sin_id = [f for f in files if not any(nombra(it, f) and set(re.findall(r"\bR\d+\b", it)) & ids for it in items)]
    if huerfanos or sin_id:
        return False, f"IDs sin unidad: {huerfanos}; archivos sin ID: {sin_id}"
    return True, "ok"


def traza_spec(spec_md: str, tests: set[str], hay_aceptacion: bool) -> tuple[bool, str]:
    """Trazabilidad lado spec (ticket 02): IDs R<n> y tabla que cita tests existentes por nombre. Sin tests nombrados
    ni archivos de aceptacion, el oraculo es accept_cmd y alcanzan los IDs."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    if not ids:
        return False, "sin IDs R<n>"
    if not tests and not hay_aceptacion:
        return True, f"{len(ids)} IDs; sin tests nombrados (accept_cmd es el oraculo)"
    m = re.search(r"## Trazabilidad\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    rows = re.findall(r"(?m)^\|\s*(R\d+)\s*\|([^|]*)\|", m.group(1) if m else "")
    cov = {rid: set(re.findall(r"test_\w+", cell)) for rid, cell in rows}
    sin_test = sorted(i for i in ids if not cov.get(i))
    inexist = sorted(t for ts in cov.values() for t in ts if t not in tests)
    if sin_test or inexist:
        return False, f"sin test: {sin_test}; tests inexistentes: {inexist}"
    return True, f"{len(ids)} IDs cubiertos"


def calza(f: str, pat: str) -> bool:
    """fnmatch con `**/` opcional: `pkg/**/*.py` incluye `pkg/a.py` (fnmatch solo exigia un directorio intermedio)."""
    import fnmatch
    return fnmatch.fnmatch(f, pat) or fnmatch.fnmatch(f, pat.replace("**/", ""))


def en_techo(f: str, need: list[str], techo: list[str]) -> bool:
    return f in need or not techo or any(calza(f, pat) for pat in techo)  # sin techo (bench, FEATURES viejas): todo vale


def plan_allowlist(plan_md: str, files: list[str], need: list[str], techo: list[str], tests_prefix: str) -> tuple[bool, str]:
    """El plan no escribe tests ajenos, incluye los archivos obligatorios y no sale del techo de sdlc.toml."""
    m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
    block = m.group(1) if m else plan_md
    # un cableo puede listar tests/test_capas.py (esta en FEAT["files"]); cualquier otro tests/ sigue prohibido
    ajenos = [t for t in re.findall(r"(?im)^\s*[-*]+\s*`?(" + re.escape(tests_prefix) + r"[\w/.-]*)", block) if t not in need]
    if ajenos:
        return False, f"plan lista {ajenos} como archivo a escribir"
    faltan = [f for f in need if f not in files]
    if faltan:
        return False, f"plan omite {faltan}"
    # orchestration-5s7: `*` de fnmatch cruza `/`, asi que `pkg/../../x.py` calzaba con `pkg/**/*.py`
    salen = [f for f in files if f.startswith(("/", "\\")) or ".." in re.split(r"[/\\]", f) or re.match(r"[A-Za-z]:", f)]
    if salen:
        return False, f"plan lista rutas que salen del repo: {salen}"
    fuera = [f for f in files if not en_techo(f, need, techo)]
    if fuera:
        return False, f"plan lista archivos fuera del techo de sdlc.toml: {fuera}"
    return True, f"{files}"


def pide(tarea: str, raiz: str) -> bool:
    """La tarea nombra `raiz` sin negarla: "sin reescribir" o "no toques ningun docstring" no piden el cambio
    (orchestration-rlg, -weo)."""
    t = tarea.lower()
    # shortcut: negacion y verbos de conservacion por regex; una prohibicion con otra forma ("quedan igual") pasa
    return raiz in t and not re.search(r"\b(?:no|sin|nunca|ni|deja|dej[aá]|conserva|conserv[aá]|manten[eé]|respeta)\b"
                                       r"[^.;:\n]{0,30}" + raiz, t)


def cambio_minimo(orig: dict[str, str], written: dict[str, str], tarea: str) -> tuple[bool, str]:
    """D11: el coder reescribio project_loop.py (-216 lineas) para agregar una guarda. Sobre un archivo que ya existia,
    borrar mas del 30% de sus lineas sin que la tarea pida reescribir es un cambio no pedido. El minimo era 40 lineas
    y un archivo chico se podia reescribir entero: ahora el minimo es min(40, la mitad del archivo, con piso 5). USD 0."""
    if pide(tarea, "reescrib"):
        return True, "la tarea pide reescribir"
    malos = []
    for f, texto in orig.items():
        if f not in written or not texto.strip():
            continue
        old_lines = [l for l in texto.splitlines() if l.strip()]
        new_set = {l.strip() for l in written[f].splitlines()}
        borradas = sum(1 for l in old_lines if l.strip() not in new_set)
        # piso de 5: cambiar la unica linea de un archivo de 1 linea es un parche (test_sdlc_build_escalera)
        if borradas > max(int(0.3 * len(old_lines)), min(40, max(len(old_lines) // 2, 5))):
            malos.append(f"{f}: {borradas} de {len(old_lines)} lineas originales desaparecieron")
    return not malos, "ok" if not malos else "; ".join(malos)


def _vacio(doc: str | None) -> bool:
    """Un docstring ausente o reducido a un marcador (`TODO`, `.`) cuenta como borrado (orchestration-weo)."""
    return not doc or len(re.sub(r"\W", "", doc)) < 3 or doc.strip().upper().rstrip(".:") in {"TODO", "TBD", "FIXME", "XXX"}


def docstring_intacto(antes: dict[str, str | None], despues: dict[str, str | None], tarea: str) -> tuple[bool, str]:
    """D4: el coder borro el docstring del modulo (22 lineas) y ningun test lo ve. Determinista, USD 0:
    un docstring de modulo que existia antes tiene que seguir existiendo, salvo que la tarea hable de docstrings."""
    if pide(tarea, "docstring"):
        return True, "la tarea lo cubre"
    perdidos = [f for f, d in antes.items() if d and _vacio(despues.get(f))]
    return not perdidos, "ok" if not perdidos else f"docstring de modulo borrado en {perdidos}"


def secciones(code: str, file_re: str) -> dict[str, str]:
    """Secciones `# ruta` / `// ruta` de una respuesta del coder; {} si no tiene cabeceras de archivo."""
    # orchestration-6qb: tambien `# File: ruta`, `### ruta` y la ruta entre backticks o negritas
    parts = re.split(r"(?m)^(?:#+|//)\s*(?:(?:file|archivo|path|ruta)\s*:\s*)?[`*]*" + file_re + r"[`*]*\s*$", code,
                     flags=re.I)
    return {parts[i]: parts[i + 1].strip() for i in range(1, len(parts), 2)} if len(parts) >= 3 else {}


def un_archivo(code: str, rel: str, file_re: str) -> tuple[bool, str]:
    """Gate un-archivo (r1 2026-09-11): el coder imito las cabeceras "# path" del prompt y pego 3 archivos en uno.
    Con varias secciones, una tiene que ser la del archivo pedido."""
    secs = secciones(code, file_re)
    if not secs:
        return True, f"{rel}: un solo archivo"
    return rel in secs, f"{rel}: recorte de {list(secs)}"


# ponytail: solo tipos de primer nivel en columna 0 (el estilo de javac y de los formateadores); uno indentado no cuenta
# orchestration-i79: una anotacion en la misma linea (`@Deprecated public class X`) tambien cuenta
_TIPO_PUBLICO = re.compile(r"(?m)^(?:@[\w.]+(?:\([^)\n]*\))?\s+)*public\s+(?:(?:abstract|final|sealed|non-sealed|strictfp)\s+)*"
                           r"(?:class|interface|enum|record|@interface)\s+(\w+)")


def tipo_propio(code: str, rel: str) -> tuple[bool, str]:
    """orchestration-7b6: en Java el tipo publico tiene el nombre del archivo. Pedido Opcion.java, el coder devolvio la
    clase Negocio (y Opcion en Bot.java, 519 -> 101 lineas)."""
    if not rel.endswith(".java"):
        return True, f"{rel}: no es Java"
    stem = rel.rsplit("/", 1)[-1][:-len(".java")]
    ajenos = [t for t in _TIPO_PUBLICO.findall(code) if t != stem]
    return (False, f"{rel}: declara public {ajenos[0]}") if ajenos else (True, f"{rel}: tipo propio")


def baseline_intacto(snap: dict[str, str], ahora: dict[str, str | None]) -> tuple[bool, str]:
    """Los tests de aceptacion congelados no cambian: mismo hash que en la foto (None = el archivo ya no existe)."""
    bad = [rel for rel, h in snap.items() if ahora.get(rel) != h]
    return not bad, "ok" if not bad else f"toco {bad}"


def tamano(t: dict, max_archivos: int, max_tokens: int) -> tuple[bool, str]:
    """Baranda de tamaño (orchestration-e4d): las 107 corridas medidas tienen <= 8 archivos y <= 16k tokens; fuera de
    ese rango no hay datos. `t` = archivos, tokens y escalon del plan (sdlc/pipeline._tamano)."""
    ok = t["archivos"] <= max_archivos and t["tokens"] <= max_tokens
    detalle = f"{t['archivos']} archivos, {t['tokens']} tokens ({t['escalon']})"
    if not ok:
        detalle += f": supera {max_archivos} archivos o {max_tokens} tokens; dividi la tarea en features mas chicas"
    return ok, detalle


# --------------------------------------------------------------------------- fase 3: decisiones sobre salidas medidas
# (D4: la ejecucion -sh, sandbox, mutantes, baselines- queda en sdlc/pipeline.py; aca solo se decide sobre su salida)


def errores_compilador(log: str) -> set[str]:
    """Errores sin linea ni columna: `ruta(l,c): error ...` (tsc), `ruta:l:c: ...` (javac, go, gcc) o `error[E..]: msg`
    con la ruta en la linea `--> ruta:l:c` (rustc, orchestration-fkp). Sin warnings."""
    out = {f"{m[1]}: {m[2].strip()}"
           for m in re.finditer(r"(?m)^\s*(\S+?\.\w+)(?:\(\d+,\d+\)|:\d+(?::\d+)?):?\s+(.+)$", log)
           if not m[2].lower().startswith("warning")}
    return out | {f"{m[2]}: error: {m[1].strip()}"
                  for m in re.finditer(r"(?m)^error(?:\[\w+\])?: (.+)\n\s*--> (\S+?):\d+:\d+", log)}


def errores_nuevos(ahora: set[str], base: set[str]) -> tuple[bool, list[str]]:
    """(sin errores nuevos, errores previos descontados): un error que la base ya tenia no bloquea (SSB F3a)."""
    previos = sorted(ahora & base)
    return len(previos) == len(ahora), previos


def fallos(log: str) -> set[str]:
    """Tests FAILED/ERROR de un log de pytest con -rfE (D8: los ERROR de setup tambien cuentan). El nodeid llega hasta
    el ` - mensaje`: un parametro con espacios (`test_p[caso 1]`) no se corta (orchestration-fkp)."""
    return {m.strip() for m in re.findall(r"(?m)^(?:FAILED|ERROR) (.+?)(?: - .*)?$", log)}


def regresion(base: set[str], ahora: set[str], aislado: str | None) -> tuple[list[str], list[str]]:
    """(fallos nuevos, flaky): un fallo nuevo es regresion solo si se repite aislado (D2 r1). `aislado` = log de la
    corrida aislada de los nuevos; None = no se corrio."""
    nuevos = sorted(ahora - base)
    if not nuevos or aislado is None:
        return nuevos, []
    flaky = sorted(set(nuevos) - fallos(aislado))
    return sorted(set(nuevos) - set(flaky)), flaky


def conteo_hallazgos(log: str) -> dict[str, int]:
    """{archivo: hallazgos} de un log de `ruff --output-format concise` y `mypy`."""
    cuenta: dict[str, int] = {}
    for m in re.finditer(r"(?m)^(\S+?):\d+:(?:\d+: |\s*error:)", log):
        f = m.group(1).replace("\\", "/")
        cuenta[f] = cuenta.get(f, 0) + 1
    return cuenta


def hallazgos(log: str) -> Counter[tuple[str, str]]:
    """Hallazgos de ruff/mypy como (archivo, mensaje) sin linea ni columna: uno movido de linea sigue siendo el mismo."""
    out: Counter[tuple[str, str]] = Counter()
    for m in re.finditer(r"(?m)^(\S+?):\d+:(?:\d+:)?\s*((?:error:\s*)?.+)$", log):
        if m[2].startswith(("note:", "warning:")):
            continue
        out[(m[1].replace("\\", "/"), re.sub(r"\bline \d+", "line N", m[2].strip()))] += 1
    return out


def lint(log_base: str, log: str, files: list[str]) -> tuple[bool, str]:
    """Ticket 1 de portfolio-circuito: sin hallazgos NUEVOS por archivo contra la base. Compara hallazgos, no cantidades:
    uno arreglado y otro nuevo en el mismo archivo es rojo (orchestration-fkp)."""
    base, ahora = conteo_hallazgos(log_base), conteo_hallazgos(log)
    base_ahora = {f: [base.get(f, 0), ahora.get(f, 0)] for f in files}
    nuevos = [f"{f}: {msg}" for (f, msg), _ in (hallazgos(log) - hallazgos(log_base)).items() if f in files]
    if not nuevos:
        return True, f"sin hallazgos nuevos (base, ahora): {base_ahora}"
    return False, f"hallazgos nuevos (base, ahora): {base_ahora}; {sorted(nuevos)[:10]}"


def mutacion(killed: int, total: int, minimo: float | None, vacuas: list, tope: int | None) -> tuple[bool, str]:
    """Ticket 08 D2: el test de aceptacion mata mutantes del codigo final. Bloquea SOLO con `mutation_min`."""
    if not total:
        return True, "sin mutantes posibles"
    score = round(killed / total, 3)
    if minimo is not None and score < float(minimo):
        return False, f"mutation score {score} < {minimo} ({killed}/{total})"
    if minimo is not None and vacuas:   # con mutation_min corren todos los mutantes: sin muertes, la propiedad es vacua
        return False, f"propiedades vacuas (no matan ningun mutante): {vacuas}"
    return True, f"mutation score {score} ({killed}/{total})" + ("" if minimo is not None else f" (observa: sin mutation_min, muestra de {tope})")


def codigo_muerto(medida: dict[str, tuple[int, list[int]]], minimo: float) -> tuple[bool, str]:
    """2026-09-17: fraccion de lineas ejecutables de los .py nuevos que corre la aceptacion; rojo bajo `minimo`.
    `medida` = {archivo: (lineas ejecutables, lineas sin ejecutar)}."""
    cobertura = {f: round(1 - len(falta) / max(total, 1), 3) for f, (total, falta) in medida.items()}
    bajos = {f: medida[f][1] for f, c in cobertura.items() if c < minimo}
    detalle = "; ".join(f"{f}: {cobertura[f]} corre, lineas sin ejecutar {falta}" for f, falta in bajos.items())
    return not bajos, detalle or f"cobertura de archivos nuevos {cobertura} >= {minimo}"
