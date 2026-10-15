# Errores del entorno contra errores del código
Type: task
Status: open
Blocked by: 
Map: ../map.md

## Question

¿Cómo distingue la etapa 4 un error del entorno de un error del código, para no gastar vueltas del coder ni escaladas a Claude?

- Caso real del ticket 04: `TS2307 Cannot find module 'node:test'` (faltaban tipos en la imagen); el coder hizo 3 vueltas y Claude 2 escaladas en vano.
- `_toolchain_missing` (`mmorch/sdlc.py`) reconoce comandos ausentes, pero no módulos ni tipos ausentes del entorno.
- Medir con los casos guardados en `logs/sdlc/diag-cases/`: cuántas escaladas eran errores del entorno.
