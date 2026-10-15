# Haiku 5.5 como coder por sorteo
Type: task
Status: open
Blocked by: 
Map: ../map.md

## Question

¿Cómo entra Haiku 5.5 como coder sin romper el invariante del REGISTRY (la familia anthropic no es un nodo)?

- Ruta: la CLI de Claude, como el planner `claude:<modelo>` de `mmorch/sdlc.py`.
- Sorteo por módulo, mitad y mitad, con semilla fija, entre Haiku 5.5 y DeepSeek v4 pro.
- El run-log guarda el coder sorteado, la fricción, el éxito, los dólares y el cupo.
