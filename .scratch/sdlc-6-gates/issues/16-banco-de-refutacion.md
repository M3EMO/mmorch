# Banco de refutación de tests de aceptación
Type: task
Status: open
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
