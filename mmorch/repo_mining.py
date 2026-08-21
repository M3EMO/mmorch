"""Minería de repos ajenos — aprender de cualquier repo SIN acumularlo.

Principio: guardar el JUGO (nota vault + candidatas + embeddings), no la
FRUTA (el clon se borra siempre). El codigo clave se referencia por URL+SHA,
jamas se copia entero. La integracion real pasa por el circuito de siempre:
candidata → dale humano → project-build → gate.

Cola: logs/repos_queue.txt (una URL por linea; # comenta). El nightly consume
1 por noche; scripts/minar.py <url> lo corre a demanda.

Licencia: ideas/patrones de cualquier lado; CODIGO literal solo si el juez
detecta licencia permisiva (MIT/Apache/BSD) — sino la candidata se marca
"solo-patron".
"""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
import threading
import time
from itertools import zip_longest
from pathlib import Path

from mmorch.iohelpers import atomic_write_json, load_json_tolerant

_PERSIST = threading.Lock()

_MAX_Q = 4  # cupo de busquedas por noche de discovery

_MINE_SCHEMA = {
    "type": "object",
    "properties": {
        "resumen": {"type": "string"},
        "licencia": {"type": "string"},
        "grafts": {"type": "array", "items": {
            "type": "object",
            "properties": {"titulo": {"type": "string"},
                           "que": {"type": "string"},
                           "aplica_a": {"type": "string"},
                           "archivos_clave": {"type": "array",
                                              "items": {"type": "string"}},
                           "esfuerzo": {"type": "string"}},
            "required": ["titulo", "que", "aplica_a"]}},
    },
    "required": ["resumen", "grafts"],
}


def _collect_context(repo_dir: Path, *, max_chars: int = 24000) -> str:
    """Destilado barato del clon: README + LICENSE + arbol + cabezas de los
    modulos mas grandes. Cap duro de chars (el juez no necesita la fruta)."""
    parts = []
    for name in ("README.md", "readme.md", "LICENSE", "LICENSE.md", "license"):
        p = repo_dir / name
        if p.exists():
            parts.append(f"== {name} ==\n" + p.read_text(
                encoding="utf-8", errors="ignore")[:4000])
    files = [f for f in repo_dir.rglob("*") if f.is_file()
             and ".git" not in f.parts]
    listing = "\n".join(str(f.relative_to(repo_dir)) for f in files[:200])
    parts.append(f"== ARBOL ({len(files)} archivos) ==\n{listing}")
    code = sorted((f for f in files
                   if f.suffix in (".py", ".ts", ".js", ".rs", ".go")),
                  key=lambda f: -f.stat().st_size)[:8]
    for f in code:
        parts.append(f"== {f.relative_to(repo_dir)} (cabeza) ==\n"
                     + f.read_text(encoding="utf-8", errors="ignore")[:2500])
    # PDFs sueltos (whitepaper, docs/architecture.pdf) — antes invisibles
    # del todo. Texto plano via pypdfium2 (sin torch, ver docs_extract.py);
    # candidata "Docling completo" queda pendiente de mas RAM.
    try:
        from mmorch.docs_extract import collect_pdfs
        pdfs = collect_pdfs(repo_dir)
        if pdfs:
            parts.append(pdfs)
    except ImportError:
        pass
    return "\n\n".join(parts)[:max_chars]


def mine_repo(url: str, *, orch_root: str, today: str,
              llm_fn=None, verify_fn=None) -> dict:
    """Clona shallow a temp → juez mapea grafts → refutador filtra → nota al
    vault + candidatas → clon BORRADO. Retorna resumen de lo persistido."""
    root = Path(orch_root)
    tmp = Path(tempfile.mkdtemp(prefix="mmorch_mine_"))
    try:
        clone = subprocess.run(
            # --depth 50 (era 1): la historia acotada habilita mineria
            # JIT-defect (fix-commit -> funcion antes=mala/despues=buena) —
            # pares de entrenamiento gratis del mismo clon que ya bajamos
            ["git", "clone", "--depth", "50", url, str(tmp / "r")],
            capture_output=True, text=True, encoding="utf-8",
            errors="replace", timeout=300)
        if clone.returncode != 0:
            return {"ok": False, "error": f"clone fallo: {clone.stderr[:150]}"}
        repo_dir = tmp / "r"
        sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=repo_dir,
                             capture_output=True, text=True).stdout.strip()[:12]
        ctx = _collect_context(repo_dir)

        # la vara de calidad es TUYA, no la fama del repo: los principios de
        # codigo del dueño son la rubrica explicita del juez y el refutador
        # (popularidad = prior de descubrimiento, jamas veredicto de calidad)
        principios = ""
        try:
            principios = (root / "docs" / "coding-principles.md").read_text(
                encoding="utf-8")[:2200]
        except OSError:
            pass

        if llm_fn is None:
            from mmorch.loop_nightly import _llm_json

            def llm_fn(prompt, schema):
                return _llm_json(prompt, schema=schema)
        out = llm_fn(
            "Sos el minero de repos de mmorch (orquestador multi-modelo con "
            "bandits, verificacion cross-family, loop de ideas y flywheel de "
            "entrenamiento; proyectos del dueño: portfolio financiero, apps "
            "Tauri, sistemas de estudio). Analiza este repo AJENO y proponer "
            "GRAFTS concretos (modulos/funciones/patrones/comportamientos que "
            "valga la pena robar e integrar). Se critico: pocos grafts buenos "
            "> muchos vagos. Identifica la licencia (si no es permisiva "
            "MIT/Apache/BSD, los grafts son solo-patron, sin codigo literal). "
            "OJO: estrellas/popularidad NO son calidad — juzga cada graft "
            "contra ESTOS principios del dueño (un patron que los viola NO es "
            "graft aunque el repo sea famoso):\n"
            f"{principios}\n"
            f"REPO {url} @ {sha}:\n{ctx}\n"
            'JSON: {"resumen": str, "licencia": str, "grafts": [{"titulo", '
            '"que", "aplica_a", "archivos_clave": [paths], "esfuerzo"}]}',
            _MINE_SCHEMA)

        grafts = out.get("grafts") or []
        if verify_fn is None:
            def verify_fn(g):
                from mmorch.loop_nightly import build_judges
                _, ver = build_judges()
                # el refutador refuta grafts que violan los principios del
                # dueño (complejidad especulativa, acoplamiento, verbosidad)
                return not ver.refute(
                    {"lente": "integracion",
                     "gist": f"{g['titulo']}: {g['que']} — ¿respeta estos "
                             f"principios? {principios[:800]}"}
                ).get("refuted", True)
        survivors = [g for g in grafts if verify_fn(g)]

        # candidatos.md y la numeracion de candidatas son estado
        # compartido: con minado en paralelo dos hilos se pisan
        with _PERSIST:
            # persistir el JUGO: nota vault con citas URL+SHA (jamas codigo entero)
            import re
            name = url.replace("\\", "/").rstrip("/").split("/")[-1].removesuffix(".git")
            name = re.sub(r"[^A-Za-z0-9._-]", "-", name)[:60] or "repo"
            nota = root / "vault" / "research" / f"minado-{name}-{today}.md"
            cites = "\n".join(
                f"- **{g['titulo']}** ({g.get('esfuerzo', '?')}): {g['que']} — "
                f"aplica a {g['aplica_a']}. Archivos: "
                + ", ".join(f"[{a}]({url}/blob/{sha}/{a})"
                            for a in (g.get("archivos_clave") or [])[:4])
                for g in survivors)
            nota.write_text(
                f"---\ntitle: minado {name} {today}\nstatus: seed\n"
                f"tags: [mmorch, repo-mining]\nsources: [{url}]\ncreated: {today}\n"
                f"---\n\n{out.get('resumen', '')}\n\nLicencia: "
                f"{out.get('licencia', 'desconocida')}\n\n## Grafts "
                f"(sobrevivieron refutacion {len(survivors)}/{len(grafts)})\n\n"
                f"{cites}\n", encoding="utf-8")

            # candidatas al loop (la aprobacion humana es el dale de siempre)
            from mmorch.fuel import parse_archivadas, parse_candidatos, render_candidatos
            cand_path = root / "vault" / "roadmaps" / "candidatos.md"
            md = cand_path.read_text(encoding="utf-8")
            vig, arch = parse_candidatos(md), parse_archivadas(md)
            from datetime import date, timedelta
            vence = (date.fromisoformat(today) + timedelta(days=14)).isoformat()
            existing_today = sum(1 for e in vig + arch if e["id"].startswith(today))
            for i, g in enumerate(survivors[:3], start=existing_today + 1):
                vig.append({"id": f"{today}-{i:02d}", "fecha": today,
                            "vence": vence, "lente": "integracion",
                            "gist": f"graft de {name}: {g['titulo']} — {g['que']} "
                                    f"(aplica a {g['aplica_a']}; ver nota "
                                    f"minado-{name}-{today})",
                            "estado": "pendiente"})
            cand_path.write_text(render_candidatos(vig, arch), encoding="utf-8")
        # JUGO extra antes de borrar la fruta: pares JIT-defect del
        # historial ajeno (dataset.build_dataset ya existia, corria solo a
        # mano). El acto de arreglar de OTRO programador etiqueta gratis.
        jit_pairs = 0
        try:
            from mmorch.dataset import build_dataset
            import hashlib as _hl
            rows = build_dataset(repo_dir, max_commits=40, max_samples=200)
            if rows:
                fw = root / "flywheel" / "jit_ajenos.jsonl"
                fw.parent.mkdir(parents=True, exist_ok=True)
                vistos = set()
                try:
                    for ln in fw.read_text(encoding="utf-8").splitlines():
                        vistos.add(json.loads(ln).get("hash"))
                except OSError:
                    pass
                with open(fw, "a", encoding="utf-8") as fh:
                    for code, label in rows:
                        h = _hl.sha256(code.encode()).hexdigest()[:16]
                        if h in vistos:
                            continue
                        vistos.add(h)
                        fh.write(json.dumps(
                            {"hash": h, "code": code, "label": label,
                             "repo": url, "fecha": today},
                            ensure_ascii=False) + "\n")
                        jit_pairs += 1
        except Exception:
            pass   # side-channel: la mineria de pares jamas rompe el minado

        return {"ok": True, "repo": name, "sha": sha,
                "grafts": len(grafts), "sobrevivieron": len(survivors),
                "nota": str(nota.name), "candidatas": len(survivors[:3]),
                "jit_pairs": jit_pairs}
    finally:
        shutil.rmtree(tmp, ignore_errors=True)   # la fruta SIEMPRE se borra


def consume_queue(orch_root: str, *, today: str, llm_fn=None,
                  verify_fn=None, n: int = 3) -> dict:
    """`n` repos por noche desde logs/repos_queue.txt, minados EN PARALELO.

    El trabajo es I/O puro (git clone + llamadas a modelos), asi que los hilos
    ganan casi lineal: 3 repos tardan lo que el mas lento, no la suma. La
    escritura de candidatas va bajo _PERSIST. Cada linea consumida se comenta
    con su resultado; una que falla vuelve a quedar libre para otra noche."""
    q = Path(orch_root) / "logs" / "repos_queue.txt"
    if (Path(orch_root) / "logs" / "loop_paused").exists():
        return {"skipped": "paused"}
    if not q.exists():
        return {"skipped": "sin cola"}
    lines = q.read_text(encoding="utf-8").splitlines()
    pend = [(i, ln.strip()) for i, ln in enumerate(lines)
            if ln.strip() and not ln.strip().startswith("#")][:n]
    if not pend:
        return {"skipped": "cola vacia"}

    # minar en paralelo multiplica el gasto: el tope mensual manda
    if llm_fn is None:
        from mmorch.loop_nightly import _check_and_count_budget
        pend = [x for x in pend
                if _check_and_count_budget(2, logs_dir=str(q.parent),
                                           month=today[:7])]
        if not pend:
            return {"skipped": "presupuesto agotado"}

    from concurrent.futures import ThreadPoolExecutor
    with ThreadPoolExecutor(max_workers=len(pend)) as pool:
        futs = {i: pool.submit(mine_repo, url, orch_root=orch_root, today=today,
                               llm_fn=llm_fn, verify_fn=verify_fn)
                for i, url in pend}
    res = []
    for i, url in pend:
        try:
            r = futs[i].result()
        except Exception as e:
            r = {"ok": False, "error": str(e)[:150]}
        res.append(r)
        if r.get("ok"):   # el que falla NO se marca: se reintenta otra noche
            lines[i] = f"# {time.strftime('%Y-%m-%d')} minado: {url}"
    q.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return {"minados": sum(1 for r in res if r.get("ok")), "resultados": res}


def discover_repos(*, orch_root: str, max_new: int = 3, http_fn=None) -> dict:
    """Auto-descubrimiento: busca en GitHub repos relevantes a las DIRECCIONES
    del roadmap + el foco de la reflexion, y encola los mejores no-minados.
    API publica de search (sin auth, 60 req/h — usamos 2-3). Queries salen de
    lo que el sistema YA decidio que le importa, no de moda."""
    import re
    import urllib.parse
    import urllib.request
    root = Path(orch_root)

    # queries deterministas desde el roadmap (titulos en negrita) + INTERESES
    # semilla (vault/roadmaps/intereses.txt, editable a mano) + foco
    # una lista POR FUENTE: el corte final es round-robin, no "las 3 primeras"
    # (antes el roadmap copaba el cupo y intereses/foco no se consultaban nunca)
    q_road: list[str] = []
    q_temas: list[str] = []
    q_foco: list[str] = []
    try:
        road = (root / "vault" / "roadmaps" / "roadmap.md").read_text(encoding="utf-8")
        q_road += re.findall(r"\*\*([^*]{6,60})\*\*", road)[:3]
    except OSError:
        pass
    try:
        temas = [t.split("#")[0].strip() for t in
                 (root / "vault" / "roadmaps" / "intereses.txt")
                 .read_text(encoding="utf-8").splitlines()
                 if t.split("#")[0].strip()]
        if temas:
            # rotacion semanal determinista: cada domingo 2 temas distintos
            week = int(time.time() // (7 * 86400))
            q_temas += [temas[(week + i) % len(temas)] for i in range(2)]
    except OSError:
        pass
    try:
        refl = (root / "logs" / "reflexiones.jsonl").read_text(
            encoding="utf-8").strip().splitlines()[-1]
        foco = json.loads(refl).get("foco_sugerido", "")[:80]
        if foco:
            q_foco.append(foco)
    except (OSError, IndexError, json.JSONDecodeError):
        pass

    # FRONTERA: temas ajenos adyacentes a los nuestros, sacados del grafo de
    # co-ocurrencia de topics de GitHub. Es la unica fuente que puede nombrar
    # un tema que nadie de este lado (ni el LLM) conoce — va primero.
    from mmorch.frontier import absorb, frontier
    logs_dir = str(root / "logs")
    conocidas = {q.lower() for q in q_road + q_temas + q_foco}
    nuevos_temas = frontier(logs_dir=logs_dir, k=2, exclude=conocidas)

    # BURSTS: el punto ciego del grafo son los temas tan nuevos que todavia no
    # tienen tag. arXiv los delata por ritmo de aparicion, no por etiqueta.
    from mmorch.bursts import bursting
    q_burst = [t for t in bursting(logs_dir=logs_dir, k=2)
               if t.lower() not in conocidas and t not in nuevos_temas]

    # round-robin: cada fuente entra al cupo, lo exogeno primero
    queries = []
    for fila in zip_longest(nuevos_temas, q_burst, q_road, q_temas, q_foco):
        queries += [q for q in fila if q]

    # cooldown: una query que dio 0 nuevos dos veces seguidas descansa 30 dias
    # (bandit "rotting": el tema se agota con los pulls y se recupera con el
    # tiempo). El presupuesto liberado es lo que financia la exploracion.
    cd_path = root / "logs" / "query_cooldown.json"
    cooldown = load_json_tolerant(cd_path, {})
    ahora = time.time()
    queries = [q for q in queries
               if not (cooldown.get(q, {}).get("zeros", 0) >= 2
                       and ahora - cooldown.get(q, {}).get("last", 0) < 30 * 86400)]

    if not queries:
        return {"skipped": "sin queries (roadmap vacio)"}

    if http_fn is None:
        def http_fn(url):
            req = urllib.request.Request(url, headers={
                "Accept": "application/vnd.github+json",
                "User-Agent": "mmorch-miner"})
            return json.loads(urllib.request.urlopen(req, timeout=15).read().decode())

    # dedup contra lo ya minado/encolado
    seen = set()
    q_path = root / "logs" / "repos_queue.txt"
    if q_path.exists():
        for line in q_path.read_text(encoding="utf-8").splitlines():
            seen.add(line.strip().lstrip("# ").split(" minado: ")[-1])
    for f in (root / "vault" / "research").glob("minado-*.md"):
        seen.add(f.stem)

    found = []
    adoptados: list[str] = []
    for q in queries[:_MAX_Q]:
        try:
            data = http_fn("https://api.github.com/search/repositories?"
                           + urllib.parse.urlencode(
                               {"q": f"{q} language:python stars:>200 pushed:>2025-06-01",
                                "sort": "stars", "per_page": 5}))
        except Exception:
            continue
        items = data.get("items", [])
        pasaron = []
        for item in items:
            url = item.get("html_url", "")
            lic = (item.get("license") or {}).get("spdx_id", "")
            if url and url not in seen and lic in ("MIT", "Apache-2.0",
                                                   "BSD-3-Clause", "BSD-2-Clause"):
                found.append(url)
                pasaron.append(item)
                seen.add(url)
        # el grafo se alimenta de TODO lo visto; "propio" solo lo que pasa filtros
        try:
            absorb(items, logs_dir=logs_dir)
            if pasaron:
                absorb(pasaron, logs_dir=logs_dir, own=True)
        except OSError:
            pass
        prev = cooldown.get(q, {})
        cooldown[q] = {"zeros": 0 if pasaron else prev.get("zeros", 0) + 1,
                       "last": ahora}
        # un tema de la FRONTERA que rinde deja de ser tanteo y pasa a interes
        # permanente (marcado "# auto" — Mateo lo edita o borra a mano)
        if pasaron and (q in nuevos_temas or q in q_burst):
            adoptados.append(q)
    atomic_write_json(cd_path, cooldown)
    if adoptados:
        try:
            with open(root / "vault" / "roadmaps" / "intereses.txt", "a",
                      encoding="utf-8") as fh:
                for t in adoptados:
                    fh.write(f"{t}  # auto {time.strftime('%Y-%m-%d')} (frontera)\n")
        except OSError:
            pass

    nuevos = found[:max_new]
    if nuevos:
        with open(q_path, "a", encoding="utf-8") as fh:
            for u in nuevos:
                fh.write(u + "\n")
    return {"queries": len(queries[:_MAX_Q]), "encolados": nuevos,
            "frontera": nuevos_temas, "bursts": q_burst,
            "adoptados": adoptados}
