# Banco de refutación de tests de aceptación
Type: task
Status: resolved (2026-09-17)
Blocked by: 14
Map: ../map.md

## Question

Armar el banco congelado que mide la skill "refutar tests" (ticket 14, D3/D6), para que la medicion del ticket 17 tenga
un oraculo por ejecucion sin juez LLM. Cada caso trae: el test de aceptacion aprobado (sin el requisito que faltaba),
la version con defecto, la version corregida y el comando que corre un test propuesto contra cada version.

Casos iniciales:
- leadlag (Portfolio): test R1..R4 de `fb8b2b4`; defecto `mmorch/wt-e570693d`; corregida `6a7e2d0` (recortada).
- reposicion (ChatBot, Maven): test R1..R4 de `eff1b86`; defecto `mmorch/wt-8d07232f`; corregida `mmorch/wt-f8223a1e`.
- export-mastery (Estudio, vitest): sin defecto conocido; mide falsas alarmas contra la version mergeada (`1bc5782`).

Hecho cuando: un runner evalua un test propuesto por caso y devuelve acierto / falsa alarma / invalido (no compila),
con una prueba que use los tests reales que ya separan las versiones (R5 de leadlag, test de revision de reposicion).
AFK. El banco vive fuera de los repos de producto y crece con cada defecto post-build.

## Answer (2026-09-17)

- `mmorch/refutacion.py`: `Banco` (contexto) abre un worktree detached, sparse y sembrado por version, corre el test
  propuesto con el comando del caso y lo clasifica: invalido (no compila/recolecta), falsa_alarma (falla con la
  corregida), acierto (falla con el defecto y pasa con la corregida), neutro. Una materializacion por version, reusada;
  al salir desengancha los sembrados y borra los worktrees (sin ramas nuevas).
- Entrada: `python -m mmorch.cli refutacion <caso> <archivo>` (exit 0 solo si acierta). Tests: `tests/test_refutacion.py`
  (repo sintetico con las 4 clases + CLI).
- Definicion del banco en `logs/refutacion/banco.json` (LOCAL: mmorch es publico y los casos apuntan a repos privados).
  Cada caso trae tarea, ficha, test aprobado sin el requisito faltante (`test_ref`), formato esperado, shas de defecto y
  corregida, comando, patron de invalido y el hueco en una linea. En otra PC hay que copiarlo a mano, como `.env`.
- Verificacion real con tests conocidos (5/5): leadlag R5 -> acierto (27 s); reposicion, colision de clave en JUnit ->
  acierto (61 s); export-mastery, test aprobado -> neutro (19 s); export-mastery con fecha equivocada -> falsa_alarma;
  test que no compila -> invalido. El `node_modules` de Estudio quedo intacto tras borrar el worktree sembrado.
- Costo por evaluacion: 4 a 61 s segun el caso (Maven el mas lento). Una medicion D6 completa (3 casos x 3 corridas x
  hasta 3 tests x 2 versiones) ronda 10 a 20 min por runtime.
