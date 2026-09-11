# Diferencial contra el oráculo Python — 300 frases, 25 sesiones (2026-09-11)

`gen_diff.py` genera 300 frases de usuario (menú, nombres de producto, colores, talles, saludos,
basura, follow-ups) con sesiones, y guarda la respuesta del prototipo `demo.py` como oráculo en
`diff_cases.json`. `DifferentialTest.java` las reproduce contra cada Bot Java y cuenta coincidencias
exactas de texto, imagen, pausa y orden. Los 35 asserts del test de aceptación NO detectan esto.

| brazo | texto | imagen | paused | order | todo igual |
|---|---|---|---|---|---|
| B script (v2) — `ab/b-sdlc` | 203/300 | 299/300 | 300 | 300 | **203/300** |
| C híbrido — `ab/c-hybrid` | 300/300 | 300 | 300 | 300 | **300/300** |
| v3 limpio (0 Claude) — `ab/v3-clean` | 300/300 | 300 | 300 | 300 | **300/300** |

B falla en el formato de precio: escribe `$ 25.000,00` (con espacio duro y decimales) donde el
oráculo escribe `$ 25.000`. El test de aceptación solo exige que contenga `$`. Los 97 casos que
fallan son todos precios. **Ese brazo B es el que la otra sesión mergeó a `main` del ChatBot**
(`dfb3184`): main tiene el bug.

Lectura: v3 sin Claude y C con Claude producen código de la misma fidelidad sobre 300 casos.
Lo que separa a B de los otros dos no es Claude: es la spec. B v2 y C/v3 partieron de la misma
spec de reasoner, pero B v2 reconstruyó con un coder que ignoró la sección de formato ARS
(§8.4 del plan) y ningún gate lo midió. Un gate diferencial contra el oráculo, cuando hay
oráculo, es determinista, cuesta cero, y es sintetizable (ticket 08/13).
