# supervision.md — driver_py


## candidato 11:44:50
revision de spec (Claude, rc=0): 0 preguntas.

## candidato 11:56:28
revision del diff (Claude, rc=0): Corrijo esas dos oraciones, sin repetir el resto:

- `test_mcp_server_reporta_version_de_mmorch` llama `version("mmorch")` sin parchear. No compara contra `mmorch_version()`. Ningún cambio en `mcp_server.py` puede arreglarlo sin tocar ese test.
- La spec traza solo `test_sdlc_d5_version.py`. Ese test viejo queda fuera de alcance. No es un defecto de este diff.
