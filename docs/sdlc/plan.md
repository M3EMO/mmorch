## Archivos
- `mmorch/plugin_worker.py` [R1, R3] [P]
- `mmorch/plugins.py` [R1, R2, R3] [P]

## Prueba
`python -m pytest tests/test_sdlc_d6_plugins_robustez.py -q`

---

### Detalle de cada archivo (no vinculante, referencia de ejecucion)

**`mmorch/plugin_worker.py`** — captura `_STDOUT = sys.stdout` al importar; `main()` ejecuta `sys.stdout = sys.stderr` antes de cargar el entry, parsea el request, carga el entry con `_load_entry`, resuelve `method`, ejecuta `fn(args, host_services)` y serializa el resultado, enviando `{"type": "error", ...}` con `load failed:` / `call failed:` por `_STDOUT` y `return 1` en cada fallo. Cierre con `raise SystemExit(main())`. Unico escritor de `_STDOUT`.

**`mmorch/plugins.py`** — `load_manifest` (devuelve dict con `dir` resuelto y `entry` del json), `invoke` (Popen con `cwd=manifest["dir"]`, `PYTHONPATH` con `REPO_ROOT`, deadline + `_pump`/`_send`/`_next_message`, kill + `join` en `finally`, traduccion de `_send=False` a `plugin worker unavailable` y de EOF a `plugin worker died with code <rc>`, timeout como `plugin timeout after {timeout:g}s`), `_decode_line` tolerante, `_send` que absorbe `BrokenPipeError`/`OSError`/`ValueError`, `_pump` unico lector de stdout, `_Eof`/`EOF`, `_next_message` y `_map_error_payload` sin prefijos.

## Prueba
`python -m pytest tests/test_sdlc_d6_plugins_robustez.py -q`