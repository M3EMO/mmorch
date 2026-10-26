"""Spec viva por area (SDD fase 2, tickets 01 a 06 de .scratch/sdd-fase-2/). Funciones puras sobre texto y sobre el
arbol del repo; el pipeline las cablea a sus etapas.

- Un area es una entrada de `[areas]` en sdlc.toml (`nombre = [globs]`); su spec vive en `docs/specs/<area>/spec.md`
  o en la ruta de `[spec_de_area]`. Sus deltas (`changes/`) y sus tests eliminados (`removidos/`) viven siempre en
  `docs/specs/<area>/`.
- Un requisito es un bloque: `- R<n>: regla` (o `- R<n> [prop]: regla`) mas sus lineas indentadas.
- Un delta tiene `## ADDED`, `## MODIFIED` y `## REMOVED` (motivo obligatorio). La fusion es por codigo.
- Contrato (rutas cubiertas) y Trazabilidad (`ruta::test`, un archivo por ID en `tests/sdlc/<area>/`) se derivan.
"""
from __future__ import annotations

import pathlib
import re
import subprocess

from mmorch.gates import calza  # una sola semantica para techo y areas: `*` cruza carpetas

_NOMBRE = re.compile(r"[a-z0-9_-]+")
_BLOQUE = re.compile(r"(?m)^- (R\d+)(?: \[prop\])?(?::|$)")
_SECCIONES_DELTA = ("ADDED", "MODIFIED", "REMOVED")
REGENERAR = "python -m sdlc specs --regenerar"


def areas(cfg: dict) -> dict[str, list[str]]:
    """`[areas]` de sdlc.toml. Un nombre fuera de `[a-z0-9_-]+` no puede ser carpeta: error del toml."""
    out = {str(k): [str(g) for g in v] for k, v in (cfg.get("areas") or {}).items()}
    malos = [k for k in out if not _NOMBRE.fullmatch(k)]
    if malos:
        raise ValueError(f"sdlc.toml [areas]: nombres invalidos {malos} (solo [a-z0-9_-])")
    return out


def areas_de(f: str, ar: dict[str, list[str]]) -> list[str]:
    return [a for a, globs in ar.items() if any(calza(f, g) for g in globs)]


def rutas_del_texto(texto: str, exts: list[str]) -> list[str]:
    """Rutas que nombra el texto de la tarea (mismo regex que `_need_files()` del bench)."""
    return list(dict.fromkeys(re.findall(r"[\w/.-]+\.(?:" + "|".join(re.escape(e) for e in exts) + r")\b", texto)))


def area_de_tarea(files: list[str], texto: str, ar: dict[str, list[str]], exts: list[str]) -> dict[str, list[str]]:
    """{area: archivos de la tarea en esa area}, ordenado. Los `files` del llamador ganan; sin `files`, las rutas del
    texto. Un archivo que calza con dos areas cuenta en las dos. Los archivos sin area no cuentan."""
    out: dict[str, list[str]] = {}
    for f in sorted(set(files or rutas_del_texto(texto, exts))):
        for a in areas_de(f, ar):
            out.setdefault(a, []).append(f)
    return dict(sorted(out.items()))


def ruta_spec(area: str, cfg: dict) -> str:
    return str((cfg.get("spec_de_area") or {}).get(area) or f"docs/specs/{area}/spec.md")


def division(por_area: dict[str, list[str]]) -> str:
    """division.md determinista: una feature por area, sin LLM (el corte ocurre antes de la etapa 1)."""
    return (f"# Division por areas\n\nLa tarea toca {len(por_area)} areas. Cada area es una feature aparte, con su "
            "propio test de aceptacion.\n" + "".join(f"\n## Area {a}\n\n" + "".join(f"- `{f}`\n" for f in fs)
                                                     for a, fs in por_area.items()))


def solapados(root, ar: dict[str, list[str]]) -> dict[str, list[str]]:
    """Archivos de `git ls-files` que caen en dos areas o mas (`*` de fnmatch cruza carpetas)."""
    ls = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, encoding="utf-8").stdout
    return {f: a for f in ls.splitlines() if len(a := areas_de(f, ar)) > 1}


# ---------------------------------------------------------------- requisitos


def _seccion(md: str, nombre: str) -> tuple[int, int] | None:
    """(inicio del contenido, fin) de `## nombre`; el fin es el proximo `## ` o el final."""
    m = re.search(rf"(?m)^## {nombre}\b[^\n]*\n", md)
    if not m:
        return None
    fin = re.search(r"(?m)^## ", md[m.end():])
    return m.end(), m.end() + fin.start() if fin else len(md)


def _bloques(texto: str) -> list[tuple[str, str]]:
    """[(ID, bloque)] en orden; un bloque va de su `- R<n>` al siguiente, sin las lineas en blanco del final."""
    ms = list(_BLOQUE.finditer(texto))
    return [(m.group(1), texto[m.start():ms[i + 1].start() if i + 1 < len(ms) else len(texto)].rstrip())
            for i, m in enumerate(ms)]


def requisitos(md: str) -> dict[str, str]:
    """{ID: bloque} de `## Requisitos` (o del texto entero si no hay seccion)."""
    s = _seccion(md, "Requisitos")
    return dict(_bloques(md[s[0]:s[1]] if s else md))


def ids_repetidos(md: str) -> list[str]:
    s = _seccion(md, "Requisitos")
    vistos = [i for i, _ in _bloques(md[s[0]:s[1]] if s else md)]
    return sorted({i for i in vistos if vistos.count(i) > 1})


def numero(rid: str) -> int:
    return int(rid[1:])


def libres(usados: set[int], n: int) -> list[int]:
    """Los `n` menores numeros libres (un hueco dura hasta el proximo ADDED; nunca se renumera)."""
    out: list[int] = []
    i = 1
    while len(out) < n:
        if i not in usados:
            out.append(i)
        i += 1
    return out


def resolver(spec_md: str, rid: str) -> tuple[str, list[str]]:
    """(bloque del requisito, tests que lo cubren segun la Trazabilidad, como `ruta::test`)."""
    return requisitos(spec_md)[rid], _filas(spec_md).get(rid, [])


def _filas(spec_md: str) -> dict[str, list[str]]:
    s = _seccion(spec_md, "Trazabilidad")
    filas: dict[str, list[str]] = {}
    for rid, celda in re.findall(r"(?m)^\|\s*(R\d+)\s*\|([^|]*)\|", spec_md[s[0]:s[1]] if s else ""):
        filas.setdefault(rid, []).extend(c.strip() for c in celda.split(",") if c.strip())
    return filas


def con_requisitos(md: str, bloques: dict[str, str]) -> str:
    """La spec con su seccion Requisitos rehecha desde `bloques`; el texto previo al primer bloque se conserva."""
    lista = "\n".join(bloques[i] for i in sorted(bloques, key=numero)) + "\n"
    s = _seccion(md, "Requisitos")
    if not s:
        return md.rstrip("\n") + "\n\n## Requisitos\n\n" + lista
    contenido = md[s[0]:s[1]]
    m = _BLOQUE.search(contenido)
    return md[:s[0]] + (contenido[:m.start()] if m else contenido) + lista + ("\n" if s[1] < len(md) else "") + md[s[1]:]


# ---------------------------------------------------------------- delta


def leer_delta(md: str) -> dict:
    """{ADDED: {id: bloque}, MODIFIED: {...}, REMOVED: {id: motivo}, desconocidos: [...], secciones: {id: [secs]}}."""
    out: dict = {"ADDED": {}, "MODIFIED": {}, "REMOVED": {}, "desconocidos": [], "secciones": {}}
    for m in re.finditer(r"(?m)^## ([^\n]*)$", md):
        titulo = m.group(1).strip()
        sec = titulo.split()[0] if titulo else ""
        if sec == "Clarificaciones":
            continue
        if sec not in _SECCIONES_DELTA or titulo not in (sec, f"{sec} Requirements"):
            out["desconocidos"].append(f"## {titulo}")
            continue
        fin = re.search(r"(?m)^## ", md[m.end():])
        for rid, bloque in _bloques(md[m.end():m.end() + fin.start() if fin else len(md)]):
            out[sec][rid] = re.sub(r"^- R\d+(?: \[prop\])?:?\s*", "", bloque).strip() if sec == "REMOVED" else bloque
            out["secciones"].setdefault(rid, []).append(sec)
    return out


def gate_delta(base_md: str, delta_md: str, tests_por_id: dict[str, list[str]]) -> list[str]:
    """Errores del delta contra la spec base (vacio = pasa). Cada error nombra su motivo."""
    d, base = leer_delta(delta_md), requisitos(base_md)
    errores = [f"encabezado desconocido: {h}" for h in d["desconocidos"]]
    errores += [f"{i} aparece en dos secciones: {secs}" for i, secs in d["secciones"].items() if len(secs) > 1]
    for sec in ("MODIFIED", "REMOVED"):
        errores += [f"{i} en {sec} no existe en la base" for i in d[sec] if i not in base]
    esperados = [f"R{n}" for n in libres({numero(i) for i in base}, len(d["ADDED"]))]
    for i, e in zip(sorted(d["ADDED"], key=numero), esperados, strict=True):
        if i != e:
            errores.append(f"ADDED {i}: el menor numero libre es {e}")
    for sec in ("ADDED", "MODIFIED"):
        errores += [f"{sec} {i} sin test ruta::test" for i in d[sec] if not any("::" in t for t in tests_por_id.get(i, []))]
    errores += [f"REMOVED {i} sin motivo" for i, motivo in d["REMOVED"].items() if not motivo]
    errores += [f"MODIFIED sin cambio: {i}" for i, b in d["MODIFIED"].items() if base.get(i) == b]
    return errores


def fusionar(base_md: str, delta_md: str) -> str:
    """Aplica el delta a la spec base: agrega ADDED, reemplaza el bloque de MODIFIED, quita REMOVED. El resto de la spec
    queda byte a byte. Un delta que no calza con la base (aplicado dos veces, IDs ajenos) es un error, nunca un parche."""
    d, bloques = leer_delta(delta_md), requisitos(base_md)
    malos = [f"ADDED {i} ya existe en la base" for i in d["ADDED"] if i in bloques]
    malos += [f"{s} {i} no existe en la base" for s in ("MODIFIED", "REMOVED") for i in d[s] if i not in bloques]
    if malos:
        raise ValueError("el delta no calza con la spec base (¿aplicado dos veces?): " + "; ".join(malos))
    bloques = {i: b for i, b in bloques.items() if i not in d["REMOVED"]} | d["MODIFIED"] | d["ADDED"]
    return con_requisitos(base_md, bloques)


def ids_cambiados(base_md: str, fusionada_md: str) -> set[str]:
    a, b = requisitos(base_md), requisitos(fusionada_md)
    return {i for i in set(a) | set(b) if a.get(i) != b.get(i)}


def chequeo_diff(base_md: str, fusionada_md: str, delta_md: str) -> list[str]:
    """El diff base -> fusionada nombra exactamente los IDs del delta (atrapa tambien un bug de la fusion)."""
    d = leer_delta(delta_md)
    del_delta = set(d["ADDED"]) | set(d["MODIFIED"]) | set(d["REMOVED"])
    diff = ids_cambiados(base_md, fusionada_md)
    return [] if diff == del_delta else [f"el diff base->fusionada nombra {sorted(diff, key=numero)}, el delta "
                                         f"{sorted(del_delta, key=numero)}"]


# ---------------------------------------------------------------- tests del area y partes derivadas


def tests_en(texto: str, rel: str) -> list[str]:
    return re.findall(r"(?m)^(?:async )?def (test_\w+)" if rel.endswith(".py") else r"\b(test_\w+)", texto)


def archivos_por_id(root, area: str) -> dict[str, list[str]]:
    """{ID: [rutas]} de `tests/sdlc/<area>/test_R<n>_*` (un archivo por ID; dos es un error del pre-commit)."""
    d = pathlib.Path(root) / "tests" / "sdlc" / area
    out: dict[str, list[str]] = {}
    for p in sorted(d.glob("test_R*_*")) if d.is_dir() else []:
        m = re.match(r"test_(R\d+)_", p.name)
        if m and p.is_file():
            out.setdefault(m.group(1), []).append(p.relative_to(root).as_posix())
    return out


def tests_por_id(root, area: str) -> dict[str, list[str]]:
    return {i: [f"{rel}::{t}" for rel in rels for t in dict.fromkeys(tests_en((pathlib.Path(root) / rel).read_text(encoding="utf-8"), rel))]
            for i, rels in archivos_por_id(root, area).items()}


def rutas_de_plan(plan_md: str) -> list[str]:
    """Rutas entre backticks de los items de `## Archivos` de un plan."""
    s = _seccion(plan_md, "Archivos")
    return re.findall(r"(?m)^\s*[-*]\s*`([^`]+)`", plan_md[s[0]:s[1]] if s else "")


def _reemplazar(md: str, nombre: str, cuerpo: str) -> str:
    s = _seccion(md, nombre)
    if not s:
        return md.rstrip("\n") + f"\n\n## {nombre}\n\n{cuerpo}"
    return md[:s[0]] + "\n" + cuerpo + ("\n" if s[1] < len(md) else "") + md[s[1]:]


def regenerar(spec_md: str, root, area: str, rutas_extra=()) -> str:
    """Contrato = rutas que ya cubria la spec, las de los planes en `changes/` y `rutas_extra` (archivos caracterizados);
    sin firmas: viven en el plan. Trazabilidad = `ruta::test` por ID, desde `tests/sdlc/<area>/`. Idempotente."""
    rutas = set(rutas_cubiertas(spec_md)) | set(rutas_extra)
    for p in sorted((pathlib.Path(root) / "docs" / "specs" / area / "changes").glob("*.plan.md")):
        rutas.update(rutas_de_plan(p.read_text(encoding="utf-8")))
    contrato = ("Rutas cubiertas por la spec, derivadas por codigo (`" + REGENERAR + "`).\n\n"
                + "".join(f"- `{r}`\n" for r in sorted(rutas)))
    tests = tests_por_id(root, area)
    filas = "".join(f"| {i} | {', '.join(tests.get(i, []))} |\n" for i in sorted(requisitos(spec_md), key=numero))
    traza = (f"Derivada por codigo desde `tests/sdlc/{area}/` (`" + REGENERAR + "`).\n\n| ID | tests |\n|---|---|\n" + filas)
    return _reemplazar(_reemplazar(spec_md, "Contrato", contrato), "Trazabilidad", traza)


def rutas_cubiertas(spec_md: str) -> list[str]:
    s = _seccion(spec_md, "Contrato")
    return re.findall(r"(?m)^- `([^`]+)`", spec_md[s[0]:s[1]] if s else "")


def gate_traza_area(spec_md: str, root) -> list[str]:
    """Trazabilidad del area: cada requisito cita >= 1 test como `ruta::test` que existe; un test, una sola fila."""
    errores: list[str] = []
    visto: dict[str, str] = {}
    filas = _filas(spec_md)
    for i in sorted(requisitos(spec_md), key=numero):
        if not filas.get(i):
            errores.append(f"{i} sin test")
    for i, citas in filas.items():
        for c in citas:
            if "::" not in c:
                errores.append(f"{i}: el test {c} esta citado sin ruta (ruta::test)")
                continue
            rel, t = c.split("::", 1)
            p = pathlib.Path(root) / rel
            if not p.is_file() or t not in tests_en(p.read_text(encoding="utf-8"), rel):
                errores.append(f"{i}: {rel}::{t} no existe")
            if c in visto and visto[c] != i:
                errores.append(f"{c} aparece en dos filas ({visto[c]} y {i}): un test cubre un solo ID")
            visto[c] = i
    return errores


def mover(root, area: str, rel: str, rid: str, tipo: str, motivo: str, etiqueta: str, contenido: bytes | None = None) -> str:
    """Mueve un test de `rid` a `docs/specs/<area>/removidos/` (fuera de la suite) sin pisar un movido anterior, y
    suma la linea al README. `contenido` = bytes previos cuando el archivo ya fue reescrito en su lugar."""
    src = pathlib.Path(root) / rel
    dst_dir = pathlib.Path(root) / "docs" / "specs" / area / "removidos"
    dst_dir.mkdir(parents=True, exist_ok=True)
    nombre, n = pathlib.Path(rel), 1
    dst = dst_dir / nombre.name
    while dst.exists():
        n += 1
        dst = dst_dir / f"{nombre.stem}-{n}{nombre.suffix}"
    dst.write_bytes(contenido if contenido is not None else src.read_bytes())
    if contenido is None:
        src.unlink()
    readme = dst_dir / "README.md"
    previo = readme.read_text(encoding="utf-8") if readme.exists() else (
        f"# Requisitos eliminados del area {area}\n\nTests fuera de la suite. Un ID eliminado se puede reusar.\n\n")
    readme.write_text(previo + f"- {rid} ({tipo}, {etiqueta}): {motivo.rstrip('.')}. Test: `{dst.name}`\n", encoding="utf-8")
    return dst.relative_to(root).as_posix()


# ---------------------------------------------------------------- pre-commit


def _git(root, *args) -> tuple[bool, str]:
    r = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.returncode == 0, r.stdout


def _specs(root) -> dict[str, str]:
    import tomllib
    p = pathlib.Path(root) / "sdlc.toml"
    cfg = tomllib.loads(p.read_text(encoding="utf-8")) if p.exists() else {}
    return {a: ruta_spec(a, cfg) for a in areas(cfg) if (pathlib.Path(root) / ruta_spec(a, cfg)).is_file()}


def chequear(root) -> list[str]:
    """Pre-commit (ticket 06): partes derivadas al dia, IDs unicos, un archivo por ID y requisitos que cambian solo con
    un archivo nuevo en `changes/`. Nunca escribe."""
    errores = []
    agregados = _git(root, "diff", "--cached", "--name-only", "--diff-filter=A")[1].splitlines()
    for area, rel in _specs(root).items():
        md = (pathlib.Path(root) / rel).read_text(encoding="utf-8")
        if regenerar(md, root, area) != md:
            errores.append(f"{rel}: Contrato o Trazabilidad viejos; corre `{REGENERAR}`")
        errores += [f"{rel}: {i} repetido en la spec" for i in ids_repetidos(md)]
        errores += [f"tests/sdlc/{area}/: {i} tiene {len(r)} archivos (un archivo por ID): {r}"
                    for i, r in archivos_por_id(root, area).items() if len(r) > 1]
        ok_h, en_head = _git(root, "show", f"HEAD:{rel}")
        ok_i, en_indice = _git(root, "show", f":{rel}")
        if ok_i and requisitos(en_head if ok_h else "") != requisitos(en_indice) \
                and not any(f.startswith(f"docs/specs/{area}/changes/") for f in agregados):
            errores.append(f"{rel}: un requisito cambio sin un archivo nuevo en docs/specs/{area}/changes/ "
                           "(un requisito cambia solo por delta)")
    return errores


def regenerar_repo(root) -> list[str]:
    hechos = []
    for area, rel in _specs(root).items():
        p = pathlib.Path(root) / rel
        md = p.read_text(encoding="utf-8")
        nuevo = regenerar(md, root, area)
        if nuevo != md:
            p.write_text(nuevo, encoding="utf-8")
            hechos.append(rel)
    return hechos
