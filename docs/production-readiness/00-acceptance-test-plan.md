# 00 — Plan de acceptance tests para "production-ready" (ejecutables)

Fecha: 2026-08-27 · Repo: `C:/Users/map12/.claude/orchestration` · Consolidado de los
informes 01-06. Convenciones:

- `PY` = `.venv/Scripts/python.exe` del checkout (los gates NO existen en el Python de sistema).
- Cada AT tiene **comando concreto** + **criterio pass/fail binario**. Sin "parece que anda".
- Estado esperado HOY entre corchetes al final de cada AT — para saber qué se está comprando.
- Varios ATs verifican features que hoy NO existen (MMORCH_HOME, /health, ledger de automerge):
  eso es intencional — este plan define "production-ready", no fotografía el presente.

---

## A. Instalación limpia standalone

**AT-1 · Install desde cero en venv virgen**
```
python -m venv %TEMP%\mm_at1 && %TEMP%\mm_at1\Scripts\pip install "C:/Users/map12/.claude/orchestration[mcp,memory,checkers,server]"
%TEMP%\mm_at1\Scripts\python.exe -c "import mmorch; print(mmorch.__name__)"
```
PASS: pip termina exit 0 y el import imprime `mmorch` sin traceback (el import eager de los ~48 módulos incluido).
FAIL: cualquier error de pip o de import. [HOY: el install anda pero el paquete no incluye mcp_server/prompts/roles — ver AT-2/AT-3.]

**AT-2 · El estado NO se escribe dentro de la instalación (MMORCH_HOME)**
```
set MMORCH_HOME=%TEMP%\mm_home_at2
%TEMP%\mm_at1\Scripts\python.exe -c "from mmorch import record_outcome; record_outcome('at2-arm', 1.0)"
dir %TEMP%\mm_home_at2\logs\feedback.jsonl
dir %TEMP%\mm_at1\Lib\site-packages\logs 2>nul
```
PASS: `feedback.jsonl` existe bajo `%MMORCH_HOME%` Y no se creó ningún `logs/` bajo site-packages.
FAIL: escritura dentro de site-packages, o MMORCH_HOME ignorado. [HOY: FAIL — 27 módulos usan `Path(__file__).parents[1]`; MMORCH_HOME no existe.]

**AT-3 · Entry point MCP instalable**
```
%TEMP%\mm_at1\Scripts\mmorch-mcp --help
```
PASS: exit 0 (el server MCP vive dentro del paquete y tiene entry point).
FAIL: comando inexistente. [HOY: FAIL — mcp_server.py está fuera del paquete; solo existen mmorch-server/mmorch-sync.]

**AT-4 · Keys resueltas por ubicación del paquete, no por cwd**
```
cd %TEMP% && C:/Users/map12/.claude/orchestration/.venv/Scripts/python.exe -c "import mmorch.providers as p; import os; print('DEEPSEEK_API_KEY' in os.environ)"
```
PASS: imprime `True` con cwd arbitrario (el `.env` se encontró igual).
FAIL: `False` — `load_dotenv()` dependió del cwd. [HOY: riesgo real; providers.py:19 resuelve por cwd — es el bug #1 de portabilidad a Cursor.]

## B. Arranque MCP (Claude Code y Cursor)

**AT-5 · Handshake MCP por stdio (protocolo real, sin cliente)**
```
PY scripts/at_mcp_handshake.py
```
donde el script manda por stdin del proceso `PY mcp_server.py` los mensajes JSON-RPC `initialize` + `tools/list` y lee la respuesta (timeout 30s).
PASS: responde `initialize` con serverInfo y `tools/list` devuelve una lista con >= 40 tools, cada una con `name` e `inputSchema`.
FAIL: timeout, crash al arrancar, o lista vacía. [HOY: debería pasar; el script auxiliar hay que escribirlo (~30 líneas).]

**AT-6 · Registrado en Claude Code**
```
claude mcp list
```
PASS: la línea de `mmorch` figura con estado conectado (✓) — o, headless: `claude -p "llamá mmorch_budget_status y pegá el JSON" --allowedTools mcp__mmorch__mmorch_budget_status` devuelve un JSON con key `month`.
FAIL: server ausente o failed. [HOY: PASS — es el setup actual.]

**AT-7 · Registrado en Cursor con perfil core**
Procedimiento: escribir `~/.cursor/mcp.json` con el bloque del informe 06 §A.1 + `MMORCH_MCP_PROFILE=core` en `env`; abrir Cursor → Settings → MCP.
PASS binario: (a) el server aparece "green/connected", (b) la cuenta de tools listadas es <= 40, y (c) invocar `mmorch_budget_status` desde el chat de Cursor devuelve JSON con `month`.
FAIL: server rojo, >40 tools expuestas, o la tool no responde. [HOY: FAIL — 46 tools y sin perfil core; además AT-4 pendiente.]

**AT-8 · Test de contrato de las tools (congelar la tabla)**
```
PY -m pytest tests/test_mcp_contract.py -q
```
Test (a escribir, análogo a test_server_smoke): enumera las tools registradas, compara contra una tabla congelada nombre→params requeridos, y verifica que TODA tool con input inválido devuelve JSON `{"error": ...}` (no excepción cruda) para al menos: `mmorch_check` con checker inexistente, `mmorch_cascade` con steps malformado.
PASS: suite verde. FAIL: tool dropeada/renombrada o excepción cruda escapando. [HOY: FAIL — el test no existe y el contrato de error es inconsistente (02 §5.1).]

## C. Smoke de tools core

**AT-9 · Smoke E2E cross-family con gasto real (manual, ~$0.01)**
```
PY smoke_test.py
```
PASS: exit 0 — fan_out (DeepSeek) genera, adversarial_verify (Gemini) refuta el bug plantado; familias del generador y verificador son distintas en el output.
FAIL: cualquier excepción, o el verificador NO detecta el bug plantado. [HOY: PASS si las keys viven; AT-10 verifica las keys.]

**AT-10 · Todas las keys del pool vivas**
```
PY -c "from mmorch import call, DEFAULT_INTUITION_POOL; [print(m, call(m, [{'role':'user','content':'di ok'}], max_tokens=5).text[:20]) for m in DEFAULT_INTUITION_POOL]"
```
PASS: cada modelo del pool responde sin excepción (costo ~$0.001).
FAIL: MissingKeyError o 401 en cualquiera. [HOY: SOSPECHA DE FAIL — ZHIPU_API_KEY reportada muerta (401) hace 47 días, sin re-verificar (05 #8).]

**AT-11 · Tools deterministas core sin gasto**
```
PY -c "from mmorch import check; r=check('arithmetic', expr='2+2', expected=4); assert r.passed, r; print('ok')"
PY -c "from mmorch.memory import write_note, recall; nid=write_note('at11','fact de prueba AT-11'); rs=recall('fact de prueba', scope='at11', k=3); assert any('AT-11' in n.text for n in rs), rs; print('ok')"
```
PASS: ambos imprimen `ok`. FAIL: assert o excepción (incluye DuckDB lockeado por otro proceso — que es en sí un finding). [HOY: PASS esperado.]

**AT-12 · Presupuesto gatea de verdad**
```
set MMORCH_MAX_MONTHLY_USD=0.000001
PY -c "from mmorch import call, BudgetExceeded
try: call('deepseek-chat',[{'role':'user','content':'hola'}]); print('FAIL: no gateo')
except BudgetExceeded: print('ok')"
```
PASS: imprime `ok` (la call se rechaza ANTES de gastar). FAIL: la call sale. [HOY: PASS esperado — providers.py:129.]

## D. Gates estáticos y suite

**AT-13 · ruff en 0**
```
PY -m ruff check .
```
PASS: "All checks passed", exit 0. [HOY: PASS.]

**AT-14 · mypy en 0 (con versión pineada)**
```
PY -m mypy mmorch mcp_server.py --ignore-missing-imports
PY -c "import mypy.version; print(mypy.version.__version__)"
```
PASS: 0 errores Y la versión instalada satisface el pin de pyproject (`mypy>=2.1,<3` una vez pineado). Nota: el scope INCLUYE mcp_server.py.
FAIL: cualquier error o versión fuera de pin. [HOY: FAIL — 10 errores en 3 archivos, mcp_server fuera del scope, sin pin.]

**AT-15 · El hook ACTIVO ejecuta ambos gates**
```
bash -c "grep -q mypy .beads/hooks/pre-commit && echo ok"
```
más una verificación de verdad de ejecución: introducir un error de tipos trivial en un archivo temporal trackeado, `git commit` → debe ser RECHAZADO; revertir.
PASS: el commit con error de tipos no entra. FAIL: entra. [HOY: FAIL — el hook activo corre solo ruff.]

**AT-16 · Suite completa verde**
```
PY -m pytest tests/ -q
```
PASS: 0 failed, 0 error (718+ tests; ~7 min). FAIL: cualquier rojo. [HOY: colección limpia verificada; resultado de la corrida completa a confirmar.]

**AT-17 · Runner de self-checks (los 30 módulos huérfanos)**
```
PY scripts/run_selfchecks.py
```
Script (a escribir, ~15 líneas): itera `PY -m mmorch.<mod>` sobre la lista de 30 módulos del informe 03 §2.1, subprocess con timeout 60s cada uno.
PASS: 30/30 exit 0. FAIL: cualquier assert/timeout. [HOY: FAIL — el runner no existe; los self-checks son cobertura latente.]

**AT-18 · CI remoto en verde**
```
gh run list --repo M3EMO/mmorch --limit 1
```
PASS: el último run de un workflow ruff+mypy+pytest en push está `completed success`.
FAIL: no hay workflow o está rojo. [HOY: FAIL — .github/ no existe; único enforcement no-bypasseable con --no-verify.]

## E. Health de sistema

**AT-19 · healthy=True alcanzable y verdadero**
```
PY -c "from mmorch.health import report; import json,sys; r=report(logs_dir='logs'); print(json.dumps(r,indent=2)); sys.exit(0 if r['healthy'] else 1)"
```
PASS: exit 0 con nightly/server/digest todos `alive` (requiere que server y digest EMITAN beat — hoy solo nightly tiene emisor).
FAIL: exit 1 o cualquier componente dead/never. [HOY: FAIL — healthy=False crónico: nightly DEAD ~41h, server/digest NEVER estructural (03 §3.1).]

**AT-20 · Endpoint GET /health del server**
```
curl -s -o NUL -w "%{http_code}" -H "Authorization: Bearer %MMORCH_SERVER_TOKEN%" http://127.0.0.1:8787/health
```
PASS: 200 con JSON que incluya estado por componente y por proveedor.
FAIL: 404 o server caído. [HOY: FAIL — la ruta no existe (server.py:804-833); la lógica sí (health.py).]

**AT-21 · Un solo comando system-check con veredicto único**
```
PY scripts/system_check.py ; echo exit=%ERRORLEVEL%
```
Encadena ruff + mypy + pytest -q + scripts/smoke.py + health.report(), y sale ≠0 si CUALQUIERA falla — incluido `healthy=False` (cerrando la trampa del smoke actual que da ✓ con el sistema no-sano).
PASS: exit 0 = sistema entero verde; cualquier degradación = exit ≠0.
[HOY: FAIL — el comando no existe; smoke.py 13/13 convive con healthy=False.]

**AT-22 · Watchdog del server no pelea el puerto**
```
powershell -c "Get-NetTCPConnection -LocalPort 8787 -State Listen | Measure-Object | % Count"
bash -c "tail -5 logs/server_forever.err | grep -c 10048 || true"
```
PASS: exactamente 1 listener en 8787 Y cero errores 10048 en el tail reciente.
FAIL: 0 o 2+ listeners, o 10048 en loop. [HOY: FAIL — tail en loop de bind 10048.]

## F. Seguridad

**AT-23 · Server sin token = no arranca (o rechaza todo)**
```
set MMORCH_SERVER_TOKEN=
PY -m uvicorn mmorch.server:app --port 18787   (proceso aparte)
curl -s -o NUL -w "%{http_code}" http://127.0.0.1:18787/state
```
PASS: el server rehúsa arrancar sin token, O toda ruta no-home devuelve 401.
FAIL: 200 sin credencial. [HOY: FAIL — token vacío = "modo dev" sin auth (server_core.py:20-25).]

**AT-24 · Zona roja bloqueada en el camino vivo**
```
PY -c "from mmorch.evolve import zone_of; assert zone_of('GOAL.md','')=='red'; assert zone_of('goal.py','')=='red'; assert zone_of('mmorch/config.py','')=='red'; assert zone_of('mmorch/textutil.py','x=1')!='red'; print('ok')"
PY -c "import json; from mmorch import self_evolve; r=self_evolve('GOAL.md','cualquier finding',do_apply=False); assert r.get('zone')=='red' or r.get('refused_red'), r; print('ok')"
```
PASS: ambos `ok` — paths rojos rechazados, path normal no. FAIL: cualquier assert. [HOY: PASS esperado — zone_of sí está en el camino vivo (evolve.py:580).]

**AT-25 · Tamper-halt de GOAL frena el loop real**
Procedimiento (reversible): `copy GOAL.md GOAL.md.bak` → agregar una línea a GOAL.md → correr:
```
PY -c "from mmorch.goal import goal_guard; goal_guard()"
PY scripts/nightly.py --dry-run   (o el paso evolve del nightly)
```
→ restaurar GOAL.md.
PASS: `goal_guard()` levanta `GoalTampered` Y el nightly ABORTA con el GOAL adulterado (no solo el guard aislado).
FAIL: el guard salta pero el nightly corre igual. [HOY: MITAD — el guard funciona (hash `2d2d924b3df25697` verificado) pero el nightly NUNCA lo llama (04 §3.1): la segunda mitad FALLA.]

**AT-26 · Gate de secretos cubre contenido, no solo nombre de path**
```
PY -c "import json, mcp_server as m"  → invocar mmorch_review_code con code="AWS_SECRET_ACCESS_KEY='AKIA...'" y con path="id_rsa"
```
PASS: ambos rechazados con `{"error"}` sin que el contenido salga a la API externa.
FAIL: cualquiera de los dos sale. [HOY: FAIL — el regex cubre solo nombres de path y no cubre id_rsa/*.pem (02 §4.8).]

**AT-27 · never-edit guard activo**
Desde una sesión de Claude Code, pedir editar `~/.claude/orchestration/GOAL.hash`.
PASS: el hook PreToolUse `never-edit-guard.js` bloquea la tool. FAIL: la edición pasa. [HOY: PASS esperado. Nota: NO viaja a Cursor — documentar como límite, no como bug.]

## G. Loop de auto-evolución gateado

**AT-28 · Nightly vivo (dead-man's switch)**
```
PY -c "from mmorch.health import check; import sys; s=check(logs_dir='logs'); sys.exit(0 if s['nightly']['status']=='alive' else 1)"
```
PASS: exit 0 (beat de nightly < 26h). FAIL: dead/never. [HOY: FAIL — overdue ~15.7h sobre el límite.]

**AT-29 · Pipeline nocturno produce y gatea (verdad de logs, no de docstrings)**
```
bash -c "tail -1 logs/nightly.jsonl | python -c 'import json,sys; r=json.load(sys.stdin); assert \"evolve\" in json.dumps(r); print(\"ok\")'"
```
más inspección binaria: la última corrida registra candidatos evaluados con `zone` y resultado de suite; los rechazos aparecen en `logs/evolve_red.jsonl` con razón.
PASS: última corrida < 26h con estructura completa y cada candidato gateado por zona+tests.
FAIL: log viejo, corrida sin gates, o candidato aplicado sin pasar por sandbox. [HOY: FAIL por frescura (nightly muerto); la estructura sí cumple.]

**AT-30 · Cero auto-apply fuera del carril verde + ledger obligatorio**
```
bash -c "git -C . log --merges --author-date-order --format='%an %s' -20 | grep -iv humano"   (inspección: ningún merge automático sin ledger)
PY -c "from pathlib import Path; import json,sys; p=Path('logs/automerge_ledger.jsonl'); sys.exit(0 if p.exists() else 1)"
```
PASS binario: TODO merge a la rama de trabajo es humano O tiene línea correspondiente en `automerge_ledger.jsonl` con `zone:"green"` y suite verde registrada. Si el automerge nunca corrió, el ledger igual debe existir vacío-inicializado tras el primer intento.
FAIL: merge automático sin ledger. [HOY: vacuamente PASS (0 auto-applies jamás) pero el ledger NO existe — el carril verde nunca ejecutó (04 §3.5); para "production-ready" se exige 1 automerge verde real con ledger.]

**AT-31 · Kill-switch pausa todo**
```
type nul > logs\loop_paused
PY -c "from mmorch.auto_repair import repair; r=repair(); print(r)"      → debe reportar paused/skip
PY -c "from mmorch.merge_train import run_train; print(run_train())"    → idem
del logs\loop_paused
```
PASS: con el archivo presente, auto_repair/automerge/merge_train/idea-loop reportan pausa y NO abren branches ni mergean.
FAIL: cualquier acción con el switch puesto. [HOY: PASS esperado — chequeado en los 4 módulos (04 §3.6).]

## H. Rollback

**AT-32 · Rollback estructural round-trip**
```
PY -c "from mmorch.evolve import snapshot_change, apply_change, rollback; from pathlib import Path
p=Path('%TEMP%/at32.py'); p.write_text('x = 1\n')
ch=snapshot_change(str(p), 'x = 2\n'); apply_change(ch); assert p.read_text()=='x = 2\n'
rollback(ch); assert p.read_text()=='x = 1\n'; print('ok')"
```
PASS: `ok` — byte-idéntico tras rollback. FAIL: assert. [HOY: PASS esperado (implementado y testeado) — pero notar que en producción la reversibilidad real es git (04 §3.3).]

**AT-33 · Rollback por git del carril de producción (revert de un PR del loop)**
Procedimiento sobre una branch del nightly ya mergeada (o mergear una de las `mmorch/tren-*` verdes pendientes):
```
git revert -m 1 <merge_commit> --no-edit
PY -m pytest tests/ -q
git revert HEAD --no-edit   (des-revert para dejar el árbol como estaba)
```
PASS: el revert aplica limpio y la suite queda verde — el camino de deshacer un cambio auto-generado está probado de punta a punta.
FAIL: conflicto en el revert o suite roja. [HOY: NUNCA ENSAYADO — hay material (los trenes 08-22/23/25 esperan merge); ejecutar una vez es parte de la definición de done.]

**AT-34 · Backup/restore del estado**
```
robocopy logs %TEMP%\mm_backup_at34 /E
set MMORCH_HOME=%TEMP%\mm_restore_at34   (con el estado restaurado ahí)
PY -c "from mmorch.memory import stats; from mmorch import budget_status; print(stats()); print(budget_status())"
```
PASS: sobre la copia restaurada, memory stats y budget devuelven los mismos conteos que el original (el estado es portable y auto-contenido).
FAIL: excepción o conteos distintos. [HOY: FAIL — sin MMORCH_HOME no hay forma soportada de apuntar a un estado restaurado; y chat.db/workflow.db viven FUERA de logs/ (se pierden en este backup — ese es exactamente el punto).]

---

## Resumen de brecha (estado esperado hoy)

| Grupo | ATs | PASS hoy | FAIL hoy |
|---|---|---|---|
| A instalación | 1-4 | 1 | 2, 3, 4 |
| B MCP | 5-8 | 5, 6 | 7, 8 |
| C smoke core | 9-12 | 9, 11, 12 | 10 (a verificar ZHIPU) |
| D gates/tests | 13-18 | 13, 16* | 14, 15, 17, 18 |
| E health | 19-22 | — | 19, 20, 21, 22 |
| F seguridad | 23-27 | 24, 27 | 23, 25 (mitad), 26 |
| G evolución | 28-31 | 31 | 28, 29, 30 (ledger) |
| H rollback | 32-34 | 32 | 33 (no ensayado), 34 |

**~13/34 en verde hoy.** Production-ready = 34/34, con AT-21 (`system_check` exit 0)
como el comando único que un tercero puede correr sin leer nada más.
