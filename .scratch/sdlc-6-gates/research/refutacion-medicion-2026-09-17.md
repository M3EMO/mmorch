# Medicion del refutador de tests (ticket 17, brazo mmorch)

Banco del ticket 16 (3 casos, oraculo por ejecucion). Runtime: `deepseek-reasoner` llamado desde mmorch, temperatura
0.7, hasta 3 propuestas por corrida, 3 corridas por caso (criterio D6 del ticket 14). Crudos (con el texto de cada
propuesta) en `logs/refutacion/medicion-mmorch-v*.json`, fuera del repo.

| Version de la skill | Corridas con acierto | Falsas alarmas | Neutros | Invalidos | Corridas caidas | USD | Minutos de modelo |
|---|---|---|---|---|---|---|---|
| v1 (contrato: archivo completo) | 0 de 9 | 0 | 22 | 0 | 0 | 0.049 | 11.5 |
| v2 (+ metodo "implementacion perezosa") | 0 de 7 | 5 | 16 | 0 | 2 | 0.044 | 10.3 |
| v3 (v1 con hasta 8 propuestas) | 0 de 9 | 1 | 35 | 0 | 0 | 0.051 | 11.5 |

- Ningun caso llega a "2 de 3": la skill NO aprueba el criterio D6 en este runtime. Se dice, como pedia el ticket.
- Casi todo sale NEUTRO: las propuestas pasan con la version con defecto y con la corregida, o sea no fijan nada nuevo.
- La v2 empeoro (5 falsas alarmas en export-mastery) y ademas agoto el presupuesto de razonamiento en 2 corridas
  (finish_reason=length con max_tokens 32768). Por el gate del ticket 14 (D7) la skill volvio a la v1.
- Hallazgo: los dos defectos reales de la semana se encontraron CON el codigo delante (revision del diff en ChatBot;
  verificacion independiente con sondas en leadlag). Refutar un test SIN ver ninguna implementacion apunta a un blanco
  muy angosto: un defecto concreto por caso. El neutro masivo es la evidencia de eso.
- v3 (8 propuestas en vez de 3, mas presupuesto de razonamiento) tampoco atrapo ninguno: 25 corridas en total
  entre las 3 versiones, 0 aciertos. Mas tiros no alcanzan: el blanco es un defecto concreto por caso.
- Conclusion del ticket 17: NINGUN runtime aprueba el criterio D6. El refutador previo a la aprobacion vuelve a
  la niebla; el banco queda armado y cualquier intento futuro se mide en minutos (US$0.05, ~12 min por pasada).
- Hermes queda fuera por decision del usuario (2026-09-17): su proveedor de DeepSeek no quedo activo, toma una clave de
  Google del entorno y carga 24 herramientas por default.
