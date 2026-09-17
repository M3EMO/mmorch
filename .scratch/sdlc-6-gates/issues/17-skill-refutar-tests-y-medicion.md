# Skill común "refutar tests" y medición mmorch contra Hermes
Type: task
Status: resolved (2026-09-17)
Blocked by: 16
Map: ../map.md

## Question

Escribir la skill comun "refutar tests" (SKILL.md, formato compartido por Claude Code y Hermes) y medirla en los dos
runtimes del ticket 14 (D2/D10): deepseek-reasoner llamado desde mmorch, y Hermes Agent 0.21.1
(`Documents/Hermes/hermes-agent`, `hermes -z` one-shot) con deepseek-reasoner. La skill recibe tarea, ficha y test
aprobado, y devuelve hasta 3 tests propuestos que nombran R<n>.

Criterio (D6): 3 corridas por caso del banco del ticket 16; un defecto queda atrapado si 2 de 3 corridas proponen un test
que separa versiones; la skill aprueba con los 2 defectos atrapados y como maximo 1 falsa alarma en export-mastery.

Hecho cuando: la tabla de resultados por runtime (aciertos, falsas alarmas, invalidos, USD, segundos) queda en
`research/` y en el vault (tag mmorch), y el usuario elige el runtime con ese numero. Si ningun runtime aprueba, se dice.
HITL solo para cargar la API key de DeepSeek en la config de Hermes (el agente no ingresa claves).

## Answer (2026-09-17): el refutador previo a la aprobacion NO aprueba

- Skill escrita: `mmorch/sdlc_templates/refutar-tests.md` (formato SKILL.md, comun a Claude y Hermes). Runner:
  `refutar(caso, runtime="mmorch"|"hermes", max_n=...)` en `mmorch/refutacion.py`.
- Medicion (tabla y crudos en `research/refutacion-medicion-2026-09-17.md` y `logs/refutacion/`): 3 versiones de la
  skill, 25 corridas, 0 aciertos. v1 (3 propuestas) 0/9 con 0 falsas alarmas; v2 (metodo "implementacion perezosa")
  0/7, 5 falsas alarmas y 2 corridas sin presupuesto de razonamiento -> revertida por el gate D7; v3 (8 propuestas)
  0/9 con 1 falsa alarma. Costo total ~US$0.14 y ~35 minutos de modelo.
- Casi todo sale NEUTRO: las propuestas pasan con la version con defecto y con la corregida.
- Hallazgo que explica el resultado: los dos defectos reales de la semana se encontraron CON el codigo delante
  (revision del diff en ChatBot; verificacion independiente con sondas en leadlag). Refutar un test sin ver ninguna
  implementacion apunta a un blanco muy angosto, y mas tiros no lo corrigen.
- Hermes queda FUERA por decision del usuario: su proveedor DeepSeek no quedo activo, tomaba una clave de Google del
  entorno y carga 24 herramientas por default.
- Consecuencia: el ticket 18 (conectar la refutacion a la etapa 1) se cierra sin construir; la idea vuelve a la niebla
  con su numero, y el banco del ticket 16 queda listo para medir cualquier refutador futuro (US$0.05, ~12 min).
