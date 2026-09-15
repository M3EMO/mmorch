---
title: auditoria mmorch/bughunt.py 2026-08-31
status: seed
tags: [mmorch, self-audit]
created: 2026-08-31
---

El módulo implementa un bug-hunter lógico mediante mutation testing. La arquitectura general es sólida (worktree para aislamiento, restauración en finally), pero hay problemas estructurales significativos con la duplicación de lógica de worktree y un bug real en el manejo de errores que puede dejar el módulo en estado corrupto.

## Findings (sobrevivieron refutacion 7/7 — 2 estructurales, 2 bugs, 2 de principios)

- **Duplicación de lógica de worktree con evolve y merge_train** [alta/estructural]: El docstring dice 'Mismo aislamiento que ya usan evolve y merge_train' pero el código duplica la lógica de open_worktree, seed, y close en lugar de extraerla a un helper compartido. Si worktree_driver cambia su API (ej: seed requiere parámetros diferentes), este módulo se rompe silenciosamente. Impacto: Acoplamiento oculto — tres módulos dependen de la misma API sin un punto único de cambio.
- **survivor_diffs cap a 40 líneas sin indicador de truncamiento** [baja/estructural]: diff_lines[:40] corta el diff pero no agrega un marcador '...' o '[truncated]'. El reviewer LLM no sabe que el diff está incompleto y puede evaluar un mutante basándose en información parcial. Impacto: Evaluaciones de riesgo potencialmente incorrectas.
- **survivors_for no restaura el archivo si write_text lanza excepción** [alta/bug]: El try/finally solo cubre la escritura del mutante y la llamada a run_fn, pero si module_path.write_text(mutant) lanza una excepción (ej: disco lleno, permisos), el finally no se ejecuta porque la excepción ocurre ANTES de entrar al try interno. El archivo queda con el mutante escrito y el código original se pierde. El except Exception externo captura pero no restaura. Impacto: Corrupción silenciosa del módulo bajo condiciones de I/O error.
- **module_pairs no valida que el test file sea ejecutable** [baja/bug]: Solo verifica que test_file.is_file() pero no que sea un archivo Python válido o que contenga tests. Un archivo test_foo.py vacío o con solo imports pasará el filtro y luego survivors_for reportará 'suite roja de base' o peor, falsos sobrevivientes. Impacto: Ruido en el mapa de sobrevivientes.
- **Violación de Deep modules en survivors_for** [media/principio]: La función tiene 50 líneas y mezcla 4 responsabilidades: (1) leer/restaurar archivo, (2) ejecutar tests, (3) generar diffs, (4) formatear resultados. El ast.unparse para normalizar está inline con un comentario largo explicando por qué. Esto debería ser un módulo separado _normalize_code() o similar. Impacto: Alto cognitive load para el lector — hay que mantener 4 concerns en la cabeza simultáneamente.
- **make_reviewer acopla a loop_nightly._llm_json sin inyección** [baja/principio]: El import de _llm_json es un acoplamiento directo a un módulo privado de otro módulo. Si loop_nightly cambia su API interna, make_reviewer se rompe. Debería inyectarse la función de LLM como parámetro. Impacto: Fragilidad ante cambios en módulos no relacionados.
- **run_fn con timeout fijo de 120s no es configurable por módulo** [media/otro]: El timeout de 120 segundos es un default razonable pero no hay forma de override por módulo. Un módulo con tests lentos (ej: integración) siempre fallará con timeout aunque el suite sea válido. El parámetro timeout existe pero solo se usa en el default run_fn, no se puede pasar un timeout diferente por módulo. Impacto: Falsos negativos en el mapa de sobrevivientes para módulos con tests lentos.
