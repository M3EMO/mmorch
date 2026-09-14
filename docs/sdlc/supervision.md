# supervision.md — driver_py


## candidato 12:22:34
revision de spec (Claude, rc=0): Hice 0 preguntas. La spec ya cubre firmas, protocolo y casos borde. Coincide con los tests de aceptación.

## candidato 12:31:19
nivel 3 Claude (rc=0): **Qué cambié**, en oraciones cortas:

`mmorch/plugin_worker.py` estaba pisado por una copia accidental de `plugins.py`. No tenía worker real.

Lo reescribí. Ahora redirige stdout a stderr antes de importar el entry. Captura fallos de carga como `error: load failed: ...`. Reenvía `host_call` al host.

En `mmorch/plugins.py` agregué `cast` para el chequeo de tipos. Narrowea `msg` después de excluir timeout y EOF. Esto arregla los errores de mypy.

También restauré el proxy de `host_call`. La reescritura anterior lo había perdido. Ese proxy chequea capabilities y llama a `host_services`. Ya no serializa `host_services` por JSON. Eso hubiera crasheado con callables reales.

## candidato 12:41:31
revision del diff (Claude, rc=0): NOTE: verifiqué los 3 tests de `tests/test_sdlc_d6_plugins_robustez.py`. Los tres pasan. Probé casos borde extra: entry ausente, método no encontrado, crash duro del worker, resultado no serializable. En todos los casos el host no crashea. Siempre devuelve `{ok: False, error: ...}`. Nunca aparece la palabra "timeout" cuando no corresponde.

Dos observaciones menores. Ninguna es un defecto demostrable de R1/R2/R3.

- `mmorch/plugins.py` perdió su bloque `__main__`. Ese bloque era el self-check ejecutable previo. No rompe ningún test. `test_selfchecks.py` sigue con 50 módulos, por encima del umbral de 40. Pero se pierde el runnable check que pide la convención del repo.
- El comentario de exclusión de `plugin_worker` en `tests/test_selfchecks.py:26` quedó desactualizado. Dice "main() exige argv de job". El worker nuevo ya no lee `sys.argv`. No causa falla porque sigue excluido por nombre.

No escribo test de bloqueo. No encontré un defecto real y demostrable contra la TAREA.
