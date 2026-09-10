# Prototipo driver v3
Type: prototype
Status: open
Blocked by: 02, 03
Map: ../map.md

## Question

Driver v3 = B v2 + los 4 gates nuevos + escalación por niveles (ticket 03), corriendo sobre la MISMA feature del A/B (port de demo.py, baseline d6853c9) desde cero. Pregunta que responde: ¿el pipeline llega a aceptación verde con 0 intervenciones de Claude y a qué costo? Comparar contra B ($1.13, 22 min) y C ($0.16, 9 min). Registrar por gate: pasó/falló, vueltas, USD. Artefacto: `research/04-driver-v3/` con el run-log. Es el único ticket que ejecuta; lo hace para decidir el 05.
