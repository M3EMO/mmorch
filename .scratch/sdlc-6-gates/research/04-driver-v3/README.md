# Prototipo driver v3 (throwaway)

Pregunta: ¿el pipeline llega a aceptación verde con 0 Claude, a qué costo?
Comparar B US$1.13 / 22 min / 0 y C US$0.16 / 9 min / 2.

No es el engine vivo. Ticket 05 decide si se integra.

## Un comando

Worktree nuevo desde `d6853c9` (ya existe `ChatBot-abBase`):

```
git -C C:\Users\map12\Desktop\QueTePario\ChatBot worktree add -b ab/v3-sdlc C:\Users\map12\Desktop\QueTePario\ChatBot-abV3 d6853c9
```

Copiar `intent.md` desde abB. Luego:

```
C:\Users\map12\.claude\orchestration\.venv\Scripts\python.exe driver_v3.py
```

`--self-check` corre los 4 gates con fixtures, cero API.

`--from-stage N` retoma como el brazo B.

Fase ledger: `ab-sdlc-v3`. Run-log: `ChatBot-abV3/docs/sdlc/run-log.json` y copia aquí al terminar.

## 2026-09-11 (ticket 07)
- Etapa `5b-review`: Claude revisa el diff con `claude_exec.run_claude` (modo edit). Bloquea SOLO si deja `ReviewTest.java` y ese test falla; entonces corre una vuelta mas de `test()`.
- Topes por avance en el fix loop: `stall_rounds` (fallos no bajan) y `diff_novelty_min` (lineas nuevas / lineas cambiadas). Tope USD en `llm()`. Defaults en `docs/sdlc/sdlc.toml` del worktree.
- Metricas nuevas: `lines` (numstat), `suite_total`; `lint_new` y `mutation_score` quedan `null` en Java (ticket 13).
- Sin correr en vivo todavia: `--self-check` PASS. Primera corrida real = rate-limiter (S2).
