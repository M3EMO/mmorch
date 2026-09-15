---
title: Loops de parche y escalación cuando falla un check (agentes vs CI)
created: 2026-09-10
tags: [research, mmorch, research, sdlc, gates, auto-repair]
status: seed
confidence: 0.8
sources: [https://proceedings.neurips.cc/paper_files/paper/2024/file/5a7c947568c1b1328ccc5230172e1e7c-Paper-Conference.pdf, https://swe-agent.com/latest/reference/model_config/, https://aider.chat/docs/usage/lint-test.html, https://github.com/Aider-AI/aider/blob/3ec8ec5a/aider/coders/base_coder.py, https://arxiv.org/html/2404.05427v2, https://docs.openhands.dev/openhands/usage/developers/evaluation-harness, https://github.com/OpenHands/benchmarks/blob/main/benchmarks/swebench/README.md, https://docs.github.com/en/actions/writing-workflows/workflow-syntax-for-github-actions, https://docs.github.com/en/pull-requests/reference/status-checks, https://docs.github.com/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners, https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands, https://cursor.com/docs/cloud-agent/capabilities, https://sre.google/workbook/error-budget-policy/, https://sre.google/workbook/alerting-on-slos/]
---
## Tronco
Cuando un check falla, CI clásico bloquea y avisa; agentes de código reintentan un parche LLM con tope chico (3–10) y luego cortan.

## Qué es
Pregunta: si un test/lint falla, ¿el estándar es reintentar el mismo comando, parchear con LLM, o avisar a una persona? Presupuestos reales de N.

## Evidencia / mecanismo

### Agentes (parche LLM, no ping humano)
- **SWE-agent (NeurIPS 2024):** ReAct. Edita rango de líneas, no reescribe el archivo entero. Tras cada `edit` corre un linter; si introduce error de sintaxis, **revierte** y pide otro edit. Tope paper: **USD 4** por instancia (~30–40 turnos `exit_cost`). Medianas resueltas: 12 turnos (GPT-4). Config actual: `per_instance_cost_limit=3.0`, `per_instance_call_limit=0` (sin tope de llamadas). Docs CLI usan USD 2. Al agotar costo: **submit automático**, no humano.
- **Aider:** lint/test post-edit. Error → pregunta «al Attempt to fix?» → reflection. **`max_reflections = 3`**. Sin `--yes-always` el humano confirma cada vuelta.
- **AutoCodeRover (ISSTA 2024):** retry de formato/apply/lint **3 veces**; con tests, loop de validación **como máximo 3**. Luego entrega el mejor parche.
- **OpenHands:** para al `max_iterations` (eval SWE-bench: 100 o 500; CLI ejemplo: 10). En eval: «NEVER ASK FOR HUMAN HELP»; tras ~3 mensajes de usuario puede `exit`.
- **Cursor Cloud (docs first-party):** auto-fix CI en PRs del agente (solo GHA). Corta a **10** follow-ups de CI, o si hay commit humano / check ya rojo en base.
- **Claude Code:** retries de API transitoria (default 10). No hay docs first-party de escalar test rojo a humano.

### CI clásico (avisar / bloquear, no auto-parche)
- **GitHub Actions `continue-on-error`:** el job/workflow **no falla**; no reintenta el check. Required checks: merge bloqueado si el check no es `success`/`skipped`/`neutral`.
- **Aviso humano:** CODEOWNERS pide review; «Require review from Code Owners». Anotaciones: `::error file=...` o Checks API (50 por request; Actions: 10 error + 10 warning por step).
- **PagerDuty:** no es el loop de test. Error budget first-party útil: **Google SRE Workbook**. Presupuesto agotado → freeze de releases (salvo P0/seguridad). Alertas de burn-rate (p.ej. 2% en 1 h) páginan **antes** del agotamiento.

## Aplicable a mmorch
Gate rojo: reasoner propone parche con N chico (3, como Aider/ACR). Aviso persona al agotar N/USD o zona roja. CI no reintenta en silencio. No juez LLM en pass/fail.

## Objeciones
Los N=3/10 son configs de paper/producto, no un estándar ISO. Nadie midió «avisar al humano vs parche LLM» con oráculo en el mismo gate. Cursor 10 es Teams-only.

## Veredicto
No hay un estándar único. CI: bloquear + anotar + CODEOWNERS. Agente: parche LLM con tope 3–10, luego stop/submit. Humano al freeze de budget (SRE) o al cortar el loop.
