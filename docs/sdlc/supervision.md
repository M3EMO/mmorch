# supervision.md — driver_py


## candidato 13:10:30
revision de spec (Claude, rc=0): Hice 1 pregunta.

## candidato 13:19:48
revision del diff (Claude, rc=0): BLOCK: `fleet_run` clasifica mal 404 vs 502.

El código matchea "no existe" o "not found" en CUALQUIER error de `forward`. R3 exige 404 solo si el error habla de host no registrado. Un error remoto no relacionado (p.ej. "recurso remoto not found") da 404 en vez de 502. El test `test_R3_fleet_run_error_no_relacionado_con_host_debe_ser_502` en `tests/test_review_sdlc.py` lo demuestra: falla con 404 real vs 502 esperado.
