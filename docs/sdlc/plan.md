```markdown
## Archivos
- `mmorch/project_driver.py` [R1, R2, R3] [P]

## Prueba
`python -m pytest tests/test_sdlc_d8_project_driver_robustez.py -q`
```

### Notas de implementación (para `mmorch/project_driver.py`)

- **Contrato público**: mantener la firma `run_project_build(task, external_test=None, plan_fn=..., build_fn=..., gate_fn=..., commit_fn=None, integrate_fn=None) -> dict[str, Any]` y el docstring del módulo textual.
- **R1 (primer seam)**: envolver `plan_fn(task, external_test)` en `try/except`; es la única llamada antes de cualquier `build_fn`/`gate_fn`/`commit_fn`/`integrate_fn`. En la excepción retornar `{'status': 'escalate', 'reason': f'planner failed: {type(e).__name__}: {str(e)[:200]}', 'task': task[:80]}`.
- **R2 (commit por unidad)**: agregar el dict de resultado a `results` y confirmar `'status': 'built'` **antes** de invocar `commit_fn(name, unit_result)`; si es `None`, omitir. Si levanta, capturar, dejar `'status': 'built'` intacto, setear `unit_result['commit_error'] = f'{type(e).__name__}: {str(e)[:200]}'` solo en ese dict y continuar el loop sin abortar.
- **R3 (integración)**: llamar `integrate_fn(external_test, results)` al finalizar el loop; en excepción retornar `{'status': 'integration_failed', 'depth': depth, 'external_test': external_test, 'detail': f'integrate_fn {type(e).__name__}: {str(e)[:200]}', 'results': results}` con `results` ya completo.
- **Recursión**: los sub-builds deben reutilizar `run_project_build` con el mismo contrato de seams, propagando/heredando el manejo de R1–R3 y el `depth` de la invocación actual.
- **Bordes cubiertos**: truncados a 200 (`str(e)`) y 80 (`task`), `task == ''`, `external_test is None` (se propaga `None` a `plan_fn`, a `integrate_fn` y al retorno), `results == []`, fallo de commit en la última unidad, y `'commit_error'` ausente cuando `commit_fn is None`.