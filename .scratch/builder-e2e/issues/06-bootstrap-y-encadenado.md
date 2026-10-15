# Bootstrap y encadenado automáticos
Type: task
Status: open
Blocked by: 
Map: ../map.md

## Question

¿Cómo arma el builder, sin manos, el commit inicial de un repo vacío y la cadena de módulos?

- Bootstrap: `src/tipos.ts` o su equivalente (contratos), `sdlc.toml`, configuración del lenguaje, `Dockerfile.sdlc` e imagen construida.
- Encadenado: cada módulo parte de la rama del módulo anterior, o de `main` tras un merge automático y verificado.
- Hoy lo hace a mano `research/04-modular/correr_modulos.py`; la aceptación es rearmar el prototipo de F2 desde una carpeta vacía con un solo comando.
