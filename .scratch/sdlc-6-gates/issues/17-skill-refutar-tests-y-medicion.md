# Skill común "refutar tests" y medición mmorch contra Hermes
Type: task
Status: open
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
