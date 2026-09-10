# Prototipo driver v3
Type: prototype
Status: resolved
Blocked by: 02, 03
Map: ../map.md

## Question

Driver v3 = B v2 + los 4 gates nuevos + escalación por niveles (ticket 03), corriendo sobre la MISMA feature del A/B (port de demo.py, baseline d6853c9) desde cero. Pregunta que responde: ¿el pipeline llega a aceptación verde con 0 intervenciones de Claude y a qué costo? Comparar contra B ($1.13, 22 min) y C ($0.16, 9 min). Registrar por gate: pasó/falló, vueltas, USD. Artefacto: `research/04-driver-v3/` con el run-log. Es el único ticket que ejecuta; lo hace para decidir el 05.

## Comments

- 2026-09-10 19:25 (sesión del usuario): primera corrida, rama `ab/v3-sdlc` c6b1ad5, desde la etapa 3: verde, 10 llamadas, US$0.34, 6.4 min, 0 vueltas, sin escalación. **Sesgada**: el prompt del build traía la pista "Catalog.load NO declara throws" (el bug de ayer) y el gate `firmas-vs-test` era un regex de ese mismo bug. No mide si los gates atrapan lo que el coder hace mal.
- 2026-09-10 19:40 (Claude): driver rearmado — pista fuera del prompt, `firmas-vs-test` borrado (`mvn test-compile` lo cubre y generaliza), regresión por unidad en el fix loop (revierte la vuelta que hace fallar tests que pasaban). Self-check PASS. Corrida limpia desde la etapa 2 en `ChatBot-abV3c`, rama `ab/v3-clean`, fase `ab-sdlc-v3c`. Esta es la medición del ticket.

## Answer

**Sí: el pipeline llega a aceptación verde con 0 intervenciones de Claude.** Corrida limpia (sin la pista del bug en el prompt, sin el gate sobreajustado), desde la etapa 2, rama `ab/v3-clean` 9b0631c, verificada a mano: Tests run 5, Failures 0.

| brazo | aceptación | llamadas | USD | min | intervenciones Claude | vueltas de fix |
|---|---|---|---|---|---|---|
| B script (v2) | verde | 31 | 1.13 | 22 | 0 | 2 |
| C híbrido | verde | 9 | 0.16 | 9 | 2 (4 min) | 0 |
| **v3 limpio** | **verde** | **9** | **0.31** | **7.3** | **0** | **0** |
| v3 sesgado (19:25) | verde | 10 | 0.34 | 6.4 | 0 | 0 |

Gates que corrieron: plan-allowlist, baseline-intacto (por archivo), G3-compile, test-compile, G4-aceptación. Ninguno rechazó. Escalera (3 vueltas / reasoner×2): no se usó. Regresión por unidad: no se ejercitó (0 vueltas).

Lecturas honestas:
- El coder no produjo el bug de la excepción chequeada esta vez. No se puede decir si fue el gate o la varianza del modelo: con 0 rechazos, los gates nuevos quedaron sin ejercitar. Para medirlos hacen falta las 3 features del ticket 06.
- v3 cuesta el doble que C en dólares y tarda lo mismo, pero sin cupo de Claude. Contra B: 4x más barato y 3x más rápido, por el fix dirigido + gates tempranos.
- Mutación y tests por unidad (ticket 13) NO están en v3. El umbral de mutación no se midió acá.
- Artefactos: `research/04-driver-v3/driver_v3.py` (rearmado), `run-log-limpio.json`, `run-log-19-25-sesgado.json`.
