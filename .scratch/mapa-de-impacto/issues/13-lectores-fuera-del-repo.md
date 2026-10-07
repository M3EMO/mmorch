# Lectores fuera del repo: otro repo, cron o MCP que lee el mismo almacén
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cómo avisa el informe que un almacén del repo (archivo, tabla, clave) lo lee algo fuera del repo, y sube eso el acierto?

Casos reales conocidos: el MCP `portfolio-duckdb` lee la base de Portfolio; Cursor lee `logs/canal.jsonl` de orchestration; las tareas programadas leen logs. Ni los literales ni los tests del repo muestran a esos lectores.

Candidatos:

- Índice de literales entre repos hermanos de la máquina (los repos que el usuario abre en Claude Code y Cursor).
- Registro declarativo por repo: un archivo que lista almacenes y lectores externos, mantenido a mano.
- Escaneo de configuración de lectores: `settings.json`, `hooks.json`, configuración de MCP y tareas programadas.

Método: el banco del ticket 09 con el lector movido a un repo hermano; el oráculo corre el test oculto en ese repo. Condiciones: B (informe actual, un solo repo) y X (informe con el candidato). Criterio: X supera a B por >= 25 puntos en acopladas sin perder más de 10 en control.

## Answer

Aprobado (2026-09-28): P instalado, R fuera. Banco de 12 tareas, deepseek-v4-pro, 108 corridas, US$0.78, cero errores de API (commits 4494eb0, f135cab y el de resultados en `orch-spike`).

| Tareas | B | P | R |
|---|---|---|---|
| Acopladas, lector en prompt o hook (tipo P) | 0/12 | 12/12 | 0/12 |
| Acopladas, lector en repo hermano (tipo R) | 0/12 | 0/12 | 3/12 |
| Control | 12/12 | 12/12 | 11/12 |

- P: +100 puntos en su tipo, p = 3.7e-07, control intacto. Con P el agente editó el prompt o el hook en las 12 corridas; sin P, nunca.
- R: +25 puntos en su tipo, p = 0.11 (no significativo), control -8. Cumple la letra del criterio (>= 25) por el mínimo.
- Confusión en R: su informe dice "revisa esos modulos" y el de P dice "actualiza esos archivos". En s3 el informe R nombra `retention_days` en `../panel/cleanup.py:8` y el agente no tocó ese archivo en 3 de 3. No se sabe si el techo de R es el detector o la redacción; no estaba pre-registrado.
- Condición de instalación (etiqueta manual, sobre 148 módulos de `mmorch/` y `scripts/`): P da 20 líneas; 18 apuntan a lectores reales (tarea nocturna ×15, hook `never-edit-guard.js`, skill `project` ×2) y 2 son ruido (`settings.json` y `package.json` nombrados en skills de scaffold). 90%: cumple. R, en el dato previo, 0 de ~20: no cumple.
- Guarda del banco: un agente intentó escribir `/home/user/...`; el agente del banco lo rechaza por estar fuera del espacio de trabajo.
- Instalado (commit 873b1b0): `mmorch/impacto_externo.py`, sumado a `impacto.report` en `.py`, con cache del índice de configuración (1.75 s en frío, 0.14 s con cache). Igual al detector medido en 149 módulos reales y en las 12 tareas.
- Hallazgo lateral: el hook importaba `mmorch/__init__` (providers + openai, 8-30 s en esta máquina) y superaba su corte de 15 s, así que en la práctica fallaba abierto sin informe. Ahora carga solo los módulos de impacto (1.9-5.3 s), con test de guarda. Aceptado por el usuario el 2026-09-28.

## Notes

**2026-09-28 — relevamiento (antes del banco):** busqué los nombres de los almacenes de orchestration (60 nombres de archivo en literales de `mmorch/` y `scripts/`) en `~/.claude` (hooks, tareas programadas, scripts, skills, comandos, agentes), `~/.cursor` y los repos del Escritorio. Lectores reales encontrados: el prompt de la tarea programada `mmorch-evolve-nightly/SKILL.md` lee `logs/nightly.jsonl` (campos `ts`, `project_health`, `failing`, `errors`) y `logs/adjudications.json` (`by_project`, `status`, `card`); `skills/project/SKILL.md` nombra `run-log.json` y `local.db`. Ningún repo del Escritorio lee almacenes de orchestration: solo coincidencias de nombres genéricos (`s.json`, `mcp.json`). El usuario eligió medir los dos candidatos.

**2026-09-28 — pre-registro (Claude, antes de correr):**

- Detectores (`orch-spike/scripts/tareas_externos/extractor_externos.py`, commit 4494eb0, congelados antes de escribir las tareas):
  - P: nombres de archivo de datos en literales del escritor (sin docstrings), buscados como palabra en los archivos de configuración de agentes (tareas programadas, skills, hooks, settings).
  - R: los literales de datos del informe instalado, contra los módulos de los repos hermanos, con el mismo criterio de especificidad y literal común.
- Dato previo, visto antes de registrar: sobre `mmorch/nightly.py`, `canal.py` y `metrics.py`, R agrega unas 20 líneas y ninguna apunta a un lector real (`env`, `help`, `timeout`, `cost_usd`); tarda 21-33 s por archivo sin cache. P encuentra los dos lectores reales de `nightly.py` y `adjudicate.py` sin ruido.
- Banco: espacio de trabajo con `repo/` (30 señuelos), `panel/` y `otro/` (repos hermanos, 12 señuelos cada uno) y `home/.claude` (8 prompts señuelo). El agente puede leer y escribir con rutas `~/` y `../`; `list_files` y `grep` miran solo el repo. 12 tareas: 4 acopladas con lector en prompt o hook (tipo P: prompt de tarea programada ×2, skill, hook JavaScript que el oráculo corre con Node), 4 acopladas con lector en `panel/` (tipo R), 2 controles de cada tipo. Oráculo validado en las 12.
- Condiciones: B (informe instalado), P (B + detector P), R (B + detector R). deepseek-v4-pro, 3 repeticiones, 108 corridas, 3 workers; errores de API aparte.
- Criterio por candidato: P pasa si supera a B por >= 25 puntos en las acopladas tipo P sin perder más de 10 en los 4 controles; R, lo mismo en las acopladas tipo R.
- Instalación: además del banco, el candidato tiene que dar líneas que apunten a lectores reales en al menos la mitad de lo que reporta sobre los escritores de almacenes de orchestration (etiqueta manual, declarada). Por el dato previo, R no lo cumple hoy aunque pase el banco.
