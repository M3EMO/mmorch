"""nightly — driver ALWAYS-ON del loop nocturno (Windows Task Scheduler, no Claude).

El scheduled-task de Claude solo corre con la app abierta; este script corre con la PC
prendida, sin Claude — invoca directo la librería (cero cupo total). Dos patas:

  1. nightly_evolve(): cosecha findings (code_review sobre archivos cambiados) -> propone
     -> sandbox+tests -> PR. Lock por archivo, nunca mergea (evolve.py, ya probado).
  2. autoresearch code_quality sobre mmorch/evolve.py, AISLADO en worktree (run_autoresearch
     edita in-place — sobre el repo vivo sería inaceptable desatendido; el worktree deja una
     branch mmorch/ar-quality-* SOLO si mejoró, para revisión humana, igual que los PRs).

Resultado a logs/nightly.jsonl — la capa de notificación (el task de Claude, 09:00) solo LEE
ese log y resume; no ejecuta nada.

Registrar:  schtasks /Create /TN mmorch-nightly /SC DAILY /ST 02:10 /F
            /TR "<venv>/python.exe <repo>/scripts/nightly.py"
"""
import json
import os
import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from dotenv import load_dotenv  # noqa: E402

load_dotenv(ROOT / ".env")

LOG = ROOT / "logs" / "nightly.jsonl"


def _log(rec: dict) -> None:
    LOG.parent.mkdir(parents=True, exist_ok=True)
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False, default=str) + "\n")


def main() -> None:
    rec: dict = {"ts": time.time()}

    try:
        from mmorch.evolve import nightly_evolve
        rec["evolve"] = nightly_evolve()
    except Exception as e:
        rec["evolve_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # autoresearch CONFIGURABLE por env (audit 2026-07: el target fijo evolve.py-maintainability
    # no tenía headroom -> baseline==best toda noche. Ahora vos apuntás donde HAYA headroom sin
    # tocar código). Default = fortalecer tests/mut_signature.py contra mutantes (mutation_score).
    #   MMORCH_AR_TASK    enunciado para el coder
    #   MMORCH_AR_TARGET  archivo que edita el loop (relativo al repo)
    #   MMORCH_AR_SCORER  script scorer (recibe el target como argv, imprime 'score: N')
    #   MMORCH_AR_ROUNDS  rounds (default 12) / MMORCH_AR_PATIENCE (default 5)
    #   MMORCH_AR_TARGET_SCORE  corta apenas el best la alcanza (default 1.0), no gasta todos los rounds
    # default = optimizar el system-prompt del coder contra pass-rate de una batería edge-heavy
    # (el ÚNICO target con headroom real medido: baseline ~0.5-0.9 y prompt-sensible; code_quality
    # y mutation_score sobre módulos daban 1.0 fijo — audit 2026-07).
    # rotacion de target (fix estancamiento medido: coder_prompt 6 corridas en
    # 0.8889 plano — evaluador saturado). Dias pares: coder_prompt; impares: el
    # prompt del madurador de ideas contra su scorer congelado (baseline 0.854,
    # headroom real). La reflexion nocturna pidio exactamente esto.
    if time.localtime().tm_yday % 2 == 0:
        _ar_t, _ar_s = "prompts/coder_prompt.txt", "scripts/score_coder_prompt.py"
    else:
        _ar_t, _ar_s = "prompts/idea_madurar.txt", "scripts/score_idea_maturation.py"
    ar_target = os.getenv("MMORCH_AR_TARGET", _ar_t)
    ar_scorer = os.getenv("MMORCH_AR_SCORER", _ar_s)
    ar_task = os.getenv("MMORCH_AR_TASK",
                        f"Reescribí el system-prompt de {ar_target} para que un coder Python resuelva "
                        "MÁS de la batería de tasks algorítmicas edge-heavy (casos borde: intervalos "
                        "que tocan, división entera hacia cero, puntuación, matrices no-cuadradas). "
                        "El prompt debe seguir pidiendo SOLO la función en un bloque de código. El "
                        "scorer mide pass-rate por ejecución — no expliques, mejorá el prompt.")
    try:
        from mmorch.autoresearch import run_autoresearch
        from mmorch.worktree_driver import open_worktree
        wt = open_worktree(str(ROOT), prefix="mmorch/ar")
        wt.seed([".venv"])   # el pre-commit hook (ruff gate) usa .venv/Scripts/python.exe relativo
                              # -> sin esto cae a python de sistema sin ruff, la gate falla, el commit
                              # se pierde silencioso (bug medido 2026-07, ver worktree_driver.capture)
        improved = False
        try:
            r = run_autoresearch(
                ar_task, ar_target,
                scorer_cmd=f'"{sys.executable}" {ar_scorer} {ar_target}',
                cwd=wt.path, maximize=True, target=float(os.getenv("MMORCH_AR_TARGET_SCORE", "1.0")),
                max_rounds=int(os.getenv("MMORCH_AR_ROUNDS", "12")),
                patience=int(os.getenv("MMORCH_AR_PATIENCE", "5")), scorer_timeout=700)
            improved = (r.baseline is not None and r.best_score is not None
                        and r.best_score > r.baseline)
            if improved:
                wt.capture(f"autoresearch {ar_target}: {r.baseline} -> {r.best_score}")
            rec["autoresearch"] = {"target": ar_target, "baseline": r.baseline, "best": r.best_score,
                                   "rounds": r.rounds, "improved": improved,
                                   "branch": wt.branch if improved else None}
        finally:
            wt.close(keep_branch=improved)   # branch queda SOLO si mejoró (revisión humana)
    except Exception as e:
        rec["autoresearch_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # DESTILADO en volumen (audit 2026-07-08: 408 episodios vs 9 notas — el nudge destila de a 5
    # cada 10 closes, el backlog crece más rápido; ritmo de entrada >> ritmo de destilación).
    # La noche es el momento barato para ponerse al día: 50/noche, MISMO watermark que el nudge
    # (nudge.json distill_upto) — un solo estado, dos ritmos (goteo diurno + lote nocturno).
    try:
        from mmorch.memory import _connect, _DB_PATH, distill_backlog
        from mmorch.nudge import _load as _nudge_load, _STATE as _NUDGE_STATE
        st = _nudge_load(_NUDGE_STATE)
        upto = int(st.get("distill_upto", 0))
        # GATE barato-primero (patron dream.rs de grok-build, 2026-07): contar ANTES de
        # destilar — si hay < min_new episodios nuevos, saltear la pasada entera (cada
        # destilado son llamadas gen+verify; una noche sin actividad no debe gastarlas).
        min_new = int(os.getenv("MMORCH_DISTILL_MIN_NEW", "10"))
        _c = _connect(_DB_PATH)
        try:
            n_new = _c.execute("SELECT COUNT(*) FROM episodic WHERE id > ? "
                               "AND kind NOT IN ('consolidation')", [upto]).fetchone()[0]
        finally:
            _c.close()
        if n_new < min_new:
            rec["distill"] = {"skipped": f"solo {n_new} episodios nuevos (< {min_new})"}
        else:
            d = distill_backlog(after_id=upto, limit=50)
            st["distill_upto"] = d["last_id"]
            _NUDGE_STATE.parent.mkdir(parents=True, exist_ok=True)
            _NUDGE_STATE.write_text(json.dumps(st, ensure_ascii=False), encoding="utf-8")
            rec["distill"] = d
            # tier 3 (HCA): con material nuevo destilado, refrescar la vista global
            # comprimida (1 llamada) — solo si el destilado persistio algo.
            if d.get("persisted", 0) > 0:
                from mmorch.memory import refresh_digest
                rec["digest"] = refresh_digest("global")
    except Exception as e:
        rec["distill_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # WORKFLOW RACE (evolución de estrategia, no solo de código): 1 task del bench por noche
    # (rotando por día — cota de costo), 3 variantes del engine compiten, el ganador por firma
    # alimenta el workflow-bandit. held-out NUNCA entra acá (anti-contaminación).
    try:
        from mmorch.bench import selection_tasks
        from mmorch.workflow_race import race
        sel = selection_tasks()
        # CURRICULUM por frecuencia (2026-07, patron "foco en topics de alta frecuencia"):
        # el compute nocturno se concentra en las FORMAS de task que mas aparecen en el uso
        # real (n del sig-bandit por firma), en vez de rotacion plana. Deterministico: dia %
        # suma-de-pesos camina la distribucion (task frecuente = mas noches). Sin datos del
        # bandit (frio / error) degrada a pesos iguales = la rotacion plana de antes.
        try:
            from mmorch.feedback import ThompsonBandit
            from mmorch.intuition import _SIG_BANDIT
            from mmorch.signature import key as sig_key
            stats = ThompsonBandit(_SIG_BANDIT).stats()

            def _freq(bt):
                sk = sig_key(bt.task)
                return int(sum(s["n"] for a, s in stats.items() if a.endswith("#" + sk)))
            weights = [1 + _freq(bt) for bt in sel]
        except Exception:
            weights = [1] * len(sel)
        r = int(time.time() // 86400) % sum(weights)
        t = sel[0]
        for t, w in zip(sel, weights, strict=True):   # noqa: B007 — t queda con la elegida
            r -= w
            if r < 0:
                break
        # EVOLUTION-SIM (backlog #1 repo-mining 2026-07): la race corre sobre la POBLACION
        # viva (no las 3 hardcodeadas) y el resultado la evoluciona — el ganador se reproduce
        # (hijo mutado/cruzado), el perdedor cronico muere (con evidencia del bandit).
        # Fitness emergente de la distribucion de bench tasks, nunca un escalar frozen.
        from mmorch.workflow_evolve import evolve_population, load_population
        rec["workflow_race"] = race(t, variants=load_population())
        rec["workflow_race"]["curriculum_weights"] = dict(zip([b.name for b in sel], weights, strict=True))
        ev = evolve_population(rec["workflow_race"].get("winner"))
        rec["workflow_race"]["evolution"] = {"born": ev["born"], "died": ev["died"],
                                             "population": sorted(ev["population"])}
    except Exception as e:
        rec["workflow_race_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # LEARN-FROM-REPOS (read-only): si MMORCH_LEARN_REPOS lista repos PÚBLICOS (coma-separados),
    # cosecha findings de cada uno a logs/external_findings.jsonl. NUNCA abre PR ni toca esos
    # repos — solo material de aprendizaje (repos ajenos = read-only; PRs solo en el tuyo).
    repos = [u.strip() for u in os.getenv("MMORCH_LEARN_REPOS", "").split(",") if u.strip()]
    if repos:
        try:
            from mmorch.evolve_findings import learn_from_repos
            rec["learn"] = learn_from_repos(repos)
        except Exception as e:
            rec["learn_error"] = f"{type(e).__name__}: {str(e)[:200]}"

    # cola de re-check de arbitrajes (blind-spot #2: descartes del árbitro nunca auditados).
    # nightly solo SURFACEA la cola — el re-juicio es del orquestador (Opus), no de un cron.
    try:
        from mmorch.arbitration import pending_recheck, stats as arb_stats
        rec["arbitration"] = {"pending_recheck": len(pending_recheck()),
                              **{k: v for k, v in arb_stats().items()
                                 if k in ("dismissed_without_evidence_rate", "total")}}
    except Exception as e:
        rec["arbitration_error"] = f"{type(e).__name__}: {str(e)[:120]}"

    # loop de ideas (spec loop-cerrado F5): adjudicacion + candidatas + cards.
    # fail-soft interno; este try es la segunda red.
    try:
        from mmorch.loop_nightly import run_idea_loop
        rec["idea_loop"] = run_idea_loop(repo_dir=str(ROOT),
                                         today=time.strftime("%Y-%m-%d"))
    except Exception as e:
        rec["idea_loop"] = {"error": f"{type(e).__name__}: {str(e)[:200]}"}

    # bug-hunt semanal (lunes): mutation-survivors = mapa de donde un bug logico
    # viviria sin ser detectado. SIEMPRE en worktree aislado (muta archivos).
    if time.localtime().tm_wday == 0:
        try:
            from mmorch.bughunt import hunt, make_reviewer
            from mmorch.worktree_driver import open_worktree
            bh_wt = open_worktree(str(ROOT), prefix="mmorch/bh")
            try:
                bh = hunt(bh_wt.path, review_fn=make_reviewer())
                worst = sorted((m for m in bh["map"] if m.get("survived")),
                               key=lambda m: -m["survived"] / max(m["mutants"], 1))[:10]
                rec["bughunt"] = {"scanned": bh["scanned"],
                                  "worst": [{"module": m["module"],
                                             "survived": m["survived"],
                                             "mutants": m["mutants"]} for m in worst],
                                  "findings": bh["findings"],
                                  "errors": bh["errors"][:5]}
            finally:
                bh_wt.close(keep_branch=False)
        except Exception as e:
            rec["bughunt_error"] = f"{type(e).__name__}: {str(e)[:150]}"

    # hardening loop (toda noche menos lunes): el peor modulo del ultimo mapa
    # de caza recibe tests anti-mutante del engine, gate = re-caza con menos
    # sobrevivientes + suite verde. Review branch, merge humano.
    if time.localtime().tm_wday != 0:
        try:
            from mmorch.hardening import harden
            rec["hardening"] = harden(str(ROOT),
                                      today=time.strftime("%Y-%m-%d"))
        except Exception as e:
            rec["hardening"] = {"error": f"{type(e).__name__}: {str(e)[:150]}"}

    # salud por proyecto: suite roja en un repo del registry = bug que nadie vio
    try:
        from mmorch.health import check_projects
        from mmorch.projects import _load as _load_projects
        # orchestration se excluye: su suite (15 min) corre en cada build/gate
        # igual, y acá solo daba TimeoutExpired (medido 1ra corrida)
        _projs = {n: p for n, p in _load_projects().items()
                  if pathlib.Path(p).resolve() != ROOT}
        # timeout 900s: la suite de Portfolio tarda ~8 min sana; colgada >15 min
        # es señal (y auto_repair la levanta a la noche siguiente)
        # Portfolio financiero: 1634 tests, 12 archivos numericos pesados sin
        # vectorizar (test_baseline_hmm.py solo = 2m40s) — medido en vivo,
        # 900s no alcanzaba SIN que nada estuviera colgado. 1800s = mismo
        # presupuesto que el tren le da a la suite propia de mmorch.
        rec["project_health"] = check_projects(_projs, timeout=1800.0)
    except Exception as e:
        rec["project_health"] = {"error": f"{type(e).__name__}: {str(e)[:150]}"}

    # reparacion cross-repo: suite roja de un proyecto del registry -> REPAIR
    # en un worktree de ESE repo con SU venv como gate; review branch alla,
    # siempre amarillo (jamas automerge en territorio ajeno)
    try:
        from mmorch.project_repair import repair_projects
        rec["project_repair"] = repair_projects(str(ROOT), rec=rec,
                                                today=time.strftime("%Y-%m-%d"))
    except Exception as e:
        rec["project_repair"] = {"error": f"{type(e).__name__}: {str(e)[:150]}"}

    # el latido va aca (dead-man's switch temprano); el RECORD se escribe al
    # final: escribirlo antes dejaba fuera del digest todo lo que sigue
    try:
        from mmorch.health import beat
        beat("nightly", logs_dir=str(ROOT / "logs"), detail="ok")
    except Exception:
        pass
    # slim: 1 modulo por noche se adelgaza (verbosidad/dup, API intacta,
    # suite como juez); la branch amarilla la levanta el tren solo
    try:
        from mmorch.slim import slim_one
        rec["slim"] = slim_one(str(ROOT), today=time.strftime("%Y-%m-%d"))
    except Exception as e:
        rec["slim"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

    # self-audit: el juez de mmorch se mira a si mismo, 1 modulo/noche
    # (rubrica = coding-principles.md, refutador cross-family, findings ->
    # candidatas del circuito de siempre). Motivo: el mismo bug (releer disco
    # en vez de usar el rec en memoria) aparecio 3 veces esta semana y ninguno
    # se detecto solo.
    try:
        from mmorch.self_audit import run_one as _audit_one
        rec["self_audit"] = _audit_one(str(ROOT), today=time.strftime("%Y-%m-%d"))
    except Exception as e:
        rec["self_audit"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

    # sintesis semanal (domingos): compara los ultimos ~10 audits de modulo
    # buscando el patron repetido que un audit de un solo archivo no puede ver
    if time.localtime().tm_wday == 6:
        try:
            from mmorch.self_audit import audit_global
            rec["self_audit_global"] = audit_global(str(ROOT),
                                                     today=time.strftime("%Y-%m-%d"))
        except Exception as e:
            rec["self_audit_global"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

        # arquitectura (domingos, sin LLM): ciclos de imports, candidatos a
        # god-module, co-cambio en git sin import (violacion CCP retroactiva
        # — asi se hubiera visto el bug de hoy antes de que pasara), y
        # señales estaticas de contaminacion entre tests. Solo lectura, va
        # al log/vault — self_audit sigue siendo el unico que propone cambios
        try:
            from mmorch.architecture import scan as _arch_scan
            arch = _arch_scan(str(ROOT))
            rec["architecture"] = {
                "ciclos": len(arch["ciclos"]),
                "god_modules": len(arch["god_module_candidates"]),
                "co_change_sin_import": len(arch["co_change_sin_import"]),
                "test_pollution": len(arch["test_pollution_candidates"]),
            }
            with open(ROOT / "logs" / "architecture.jsonl", "a", encoding="utf-8") as fh:
                fh.write(json.dumps({"fecha": time.strftime("%Y-%m-%d"), **arch},
                                    ensure_ascii=False) + "\n")
        except Exception as e:
            rec["architecture"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

    # cosecha de arXiv TODAS las noches (cubetas semanales de bigramas): el
    # burst es un ratio contra las semanas propias, asi que necesita un ritmo
    # de muestreo parejo. Barato: 5 requests con sleep de 3s.
    try:
        from mmorch.bursts import harvest
        rec["arxiv"] = harvest(logs_dir=str(ROOT / "logs"))
    except Exception as e:
        rec["arxiv"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

    # descubrimiento semanal (domingos): GitHub search con queries de la
    # frontera de topics + bursts de arXiv + roadmap + intereses + foco
    if time.localtime().tm_wday == 6:
        try:
            from mmorch.repo_mining import discover_repos
            rec["repo_discovery"] = discover_repos(orch_root=str(ROOT))
        except Exception as e:
            rec["repo_discovery"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

    # mineria de repos ajenos: 3 EN PARALELO desde logs/repos_queue.txt —
    # clona efimero, destila grafts al vault + candidatas, borra el clon
    try:
        from mmorch.repo_mining import consume_queue
        rec["repo_mining"] = consume_queue(str(ROOT), n=3,
                                           today=time.strftime("%Y-%m-%d"))
    except Exception as e:
        rec["repo_mining"] = {"error": f"{type(e).__name__}: {str(e)[:150]}"}

    # smoke de subsistemas: uso correcto de cada pieza, barato y read-only;
    # historia en logs/smoke.jsonl (el digest reporta los rojos)
    try:
        import subprocess as _sp2
        _sp2.run([sys.executable, str(ROOT / "scripts" / "smoke.py")],
                 capture_output=True, timeout=300)
        _last = (ROOT / "logs" / "smoke.jsonl").read_text(
            encoding="utf-8").strip().splitlines()[-1]
        rec["smoke"] = json.loads(_last)
    except Exception as e:
        rec["smoke"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

    # merge train: las amarillas del dia se conglomeran en UNA branch con gate
    # de integracion sobre la union -> un solo click humano por dia
    try:
        import subprocess as _sp
        from mmorch.merge_train import run_train
        _base = _sp.run(["git", "rev-parse", "--abbrev-ref", "HEAD"],
                        cwd=str(ROOT), capture_output=True,
                        text=True).stdout.strip()
        rec["merge_train"] = run_train(str(ROOT), base=_base,
                                       today=time.strftime("%Y-%m-%d"))
    except Exception as e:
        rec["merge_train"] = {"error": f"{type(e).__name__}: {str(e)[:150]}"}

    # reflexion: mmorch mira su propia trayectoria (ultimas 7 noches) y elige
    # foco — capa "pensar sobre si mismo" (goal Jarvis 2026-08-15)
    try:
        from mmorch.loop_nightly import reflect
        rec["reflexion"] = reflect(logs_dir=str(ROOT / "logs"),
                                   today=time.strftime("%Y-%m-%d"))
    except Exception as e:
        rec["reflexion"] = {"error": f"{type(e).__name__}: {str(e)[:120]}"}

    # flywheel: refrescar training/ cada noche (capturas corren todo el dia;
    # esto solo re-exporta el acumulado a formato entrenable)
    try:
        import subprocess as _sp
        _sp.run([sys.executable, str(ROOT / "scripts" / "export_training_data.py")],
                capture_output=True, timeout=300)
    except Exception:
        pass

    # rastro durable de errores silenciosos: independiente del formato de
    # nightly.jsonl (uno por noche), este es append-only por HALLAZGO, asi
    # que un error queda visible aunque nadie corra reflect() sobre esa noche
    try:
        from mmorch.auto_repair import findings_from_record
        with open(ROOT / "logs" / "silent_errors.jsonl", "a", encoding="utf-8") as fh:
            for f in findings_from_record(rec):
                fh.write(json.dumps({"fecha": time.strftime("%Y-%m-%d"), **f},
                                    ensure_ascii=False) + "\n")
    except Exception:
        pass

    # auto-reparacion: ahora corre AL FINAL sobre el rec EN MEMORIA de esta
    # misma corrida (antes leia nightly.jsonl y reparaba lo de ANOCHE, porque
    # el registro de hoy todavia no se habia escrito) — mismo dia: un error
    # que aparece temprano en la corrida se repara esa misma madrugada
    try:
        from mmorch.auto_repair import repair
        rec["auto_repair"] = repair(str(ROOT), today=time.strftime("%Y-%m-%d"), rec=rec)
    except Exception as e:
        rec["auto_repair"] = {"error": f"{type(e).__name__}: {str(e)[:150]}"}

    _log(rec)

    # digest local (no depende de la app de Claude): logs/digest_last.md
    try:
        from mmorch.loop_nightly import write_local_digest
        write_local_digest(rec, logs_dir=str(ROOT / "logs"))
    except Exception:
        pass
    print(json.dumps(rec, ensure_ascii=False, default=str))


if __name__ == "__main__":
    main()
