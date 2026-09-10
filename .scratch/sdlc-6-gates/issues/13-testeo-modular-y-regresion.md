# Testeo modular + regresión por unidad
Type: grilling
Status: open
Blocked by: 02
Map: ../map.md

## Question

Pregunta del usuario (2026-09-10): ¿el pipeline tiene testeo modular y total, para ver si una integración corrompe algo? Hoy no: en el A/B ningún brazo tuvo tests por módulo (B y C solo el test total de 35 asserts + compile por archivo; el engine de A tiene test_cmd por unidad pero el planner dejó las 7 unidades sin test y el gate de integración nunca corrió). Decidir tres gates: (1) tests por unidad SINTETIZADOS desde la spec por deepseek-reasoner y promovidos por `mutation_score` (existe en checkers.py, el engine no lo llama — ticket 01); (2) gate de regresión: la suite completa corre después de que cada unidad aterriza, y una rotura se atribuye a esa unidad (mvn test tarda 3 s); (3) `test-compile` antes del test. Preguntas: ¿un test sintetizado que no mata mutantes se descarta o se marca advisory? ¿qué cobertura mínima exige la promoción? ¿la regresión por unidad reemplaza al gate de integración final o lo complementa? ¿cómo se reporta "la unidad X rompió el test Y"?
