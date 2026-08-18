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
import time
from pathlib import Path

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
    return "\n\n".join(parts)[:max_chars]


def mine_repo(url: str, *, orch_root: str, today: str,
              llm_fn=None, verify_fn=None) -> dict:
    """Clona shallow a temp → juez mapea grafts → refutador filtra → nota al
    vault + candidatas → clon BORRADO. Retorna resumen de lo persistido."""
    root = Path(orch_root)
    tmp = Path(tempfile.mkdtemp(prefix="mmorch_mine_"))
    try:
        clone = subprocess.run(
            ["git", "clone", "--depth", "1", url, str(tmp / "r")],
            capture_output=True, text=True, encoding="utf-8",
            errors="replace", timeout=300)
        if clone.returncode != 0:
            return {"ok": False, "error": f"clone fallo: {clone.stderr[:150]}"}
        repo_dir = tmp / "r"
        sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=repo_dir,
                             capture_output=True, text=True).stdout.strip()[:12]
        ctx = _collect_context(repo_dir)

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
            "MIT/Apache/BSD, los grafts son solo-patron, sin codigo literal).\n"
            f"REPO {url} @ {sha}:\n{ctx}\n"
            'JSON: {"resumen": str, "licencia": str, "grafts": [{"titulo", '
            '"que", "aplica_a", "archivos_clave": [paths], "esfuerzo"}]}',
            _MINE_SCHEMA)

        grafts = out.get("grafts") or []
        if verify_fn is None:
            def verify_fn(g):
                from mmorch.loop_nightly import build_judges
                _, ver = build_judges()
                return not ver.refute({"lente": "integracion",
                                       "gist": f"{g['titulo']}: {g['que']}"}
                                      ).get("refuted", True)
        survivors = [g for g in grafts if verify_fn(g)]

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

        return {"ok": True, "repo": name, "sha": sha,
                "grafts": len(grafts), "sobrevivieron": len(survivors),
                "nota": str(nota.name), "candidatas": len(survivors[:3])}
    finally:
        shutil.rmtree(tmp, ignore_errors=True)   # la fruta SIEMPRE se borra


def consume_queue(orch_root: str, *, today: str, llm_fn=None,
                  verify_fn=None) -> dict:
    """1 repo por noche desde logs/repos_queue.txt (linea consumida se comenta)."""
    q = Path(orch_root) / "logs" / "repos_queue.txt"
    if (Path(orch_root) / "logs" / "loop_paused").exists():
        return {"skipped": "paused"}
    if not q.exists():
        return {"skipped": "sin cola"}
    lines = q.read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        url = line.strip()
        if not url or url.startswith("#"):
            continue
        r = mine_repo(url, orch_root=orch_root, today=today,
                      llm_fn=llm_fn, verify_fn=verify_fn)
        lines[i] = f"# {time.strftime('%Y-%m-%d')} minado: {url}"
        q.write_text("\n".join(lines) + "\n", encoding="utf-8")
        return r
    return {"skipped": "cola vacia"}


def discover_repos(*, orch_root: str, max_new: int = 3, http_fn=None) -> dict:
    """Auto-descubrimiento: busca en GitHub repos relevantes a las DIRECCIONES
    del roadmap + el foco de la reflexion, y encola los mejores no-minados.
    API publica de search (sin auth, 60 req/h — usamos 2-3). Queries salen de
    lo que el sistema YA decidio que le importa, no de moda."""
    import re
    import urllib.parse
    import urllib.request
    root = Path(orch_root)

    # queries deterministas desde el roadmap (titulos en negrita) + foco
    queries = []
    try:
        road = (root / "vault" / "roadmaps" / "roadmap.md").read_text(encoding="utf-8")
        queries += re.findall(r"\*\*([^*]{6,60})\*\*", road)[:4]
    except OSError:
        pass
    try:
        refl = (root / "logs" / "reflexiones.jsonl").read_text(
            encoding="utf-8").strip().splitlines()[-1]
        foco = json.loads(refl).get("foco_sugerido", "")[:80]
        if foco:
            queries.append(foco)
    except (OSError, IndexError, json.JSONDecodeError):
        pass
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
    for q in queries[:3]:
        try:
            data = http_fn("https://api.github.com/search/repositories?"
                           + urllib.parse.urlencode(
                               {"q": f"{q} language:python stars:>200 pushed:>2025-06-01",
                                "sort": "stars", "per_page": 5}))
        except Exception:
            continue
        for item in data.get("items", []):
            url = item.get("html_url", "")
            lic = (item.get("license") or {}).get("spdx_id", "")
            if url and url not in seen and lic in ("MIT", "Apache-2.0",
                                                   "BSD-3-Clause", "BSD-2-Clause"):
                found.append(url)
                seen.add(url)
    nuevos = found[:max_new]
    if nuevos:
        with open(q_path, "a", encoding="utf-8") as fh:
            for u in nuevos:
                fh.write(u + "\n")
    return {"queries": len(queries[:3]), "encolados": nuevos}
