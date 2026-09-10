# Cierre del ab y tabla final
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Cerrar el A/B del 2026-09-10: resultado del intento 5 de A (job 0df98b5c07), tabla final A/B/C con aceptación/USD/min/intervenciones, mover `ab_results.json`, `ab_sdlc_b.py` y `synth_seed47_findings.md` del scratchpad a `docs/ab-sdlc-2026-09-10/`, actualizar la sección Measured del README y escribir la nota de vault. AFK. Desbloquea la lectura de evidencia de 05 y 07 desde el repo.

## Answer

- A (engine): rojo en 5 intentos, US$0.81, ~120 min, 4 fixes de engine. El intento 5 aterrizó `Bot.java` (571 líneas) y quedó a un método de verde: `Catalog.load throws IOException`, el mismo defecto que C corrigió con 4 líneas. Defectos nuevos: gen_model no se propaga a la recursión; max_depth de la forma VERIFY no se sobreescribe; sin gate test-compile; la integración no realimenta al coder.
- B (script): verde en v2, US$1.13, 22 min, 0 intervenciones humanas (2 de arnés). v1 se rompió a sí mismo (fix loop ciego).
- C (híbrido): verde, US$0.16, 9 min, 2 intervenciones de Claude (4 min), las dos reemplazables por gates deterministas.
- Artefactos en el repo: `docs/ab-sdlc-2026-09-10/` (README con la tabla, ab_results.json, driver, spec/plan de reasoner, run-log de B, supervision de C, hallazgos synth seed 47). README "Measured" actualizado. Nota de vault escrita (tag mmorch).
- Lo que no prueba: una feature, un repo, un día; los tres brazos recibieron correcciones durante el experimento. Las 3 features del ticket 06 son la validación.
