# Lock por archivo del loop nocturno de PRs se pierde ante corrupción de evolve_open_prs.json

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: M
Eje: robustez
Evidence: mmorch/evolve.py:440-448,453-458,462-464,496-511,534-539

`_load_pr_state()` → JSON corrupto → `{}` sin log: desaparecen los locks por archivo y
`coordinated_evolve_round()` puede abrir un branch competidor sobre un archivo con PR
abierto — exactamente la carrera que el lock previene. `_save_pr_state()` es write_text
no atómico y el loop corre desatendido de noche: el crash nocturno produce el truncado
que el load traga. Colateral: se pierden los outcomes post-merge.

**Fix:** write atómico + log fuerte al detectar corrupción en load.

## Comments
`_load_pr_state`/`_save_pr_state` migrados a `iohelpers.load_json_tolerant` (log fuerte,
`{}` como default legítimo solo si el archivo no existe) y `atomic_write_json`. Cubierto
indirectamente por `tests/test_evolve_*.py` (siguen verdes); no agregué un test nuevo por
archivo — el patrón atómico/tolerante ya tiene su test dedicado en
`tests/test_iohelpers.py`.
