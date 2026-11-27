# Brief de la Tarea 176c para el builder local

Este brief vale solo. No necesita otra conversación. La Tarea 176c arregla un defecto de la Tarea 172 en `colab empaquetar --corre`.

## El defecto

La prueba de aislamiento en Colab cortó antes de lanzar el builder. `r00t corre` salió con código 3 en 0 segundos.

- El mensaje, literal: "error: GOAL no autorizado: falta GOAL.hash".
- `src/colab_corre.rs` exige `GOAL.md` en la raíz de Root y lo copia al paquete. No exige ni copia `GOAL.hash`.
- `r00t corre` llama a `verificar_goal` sobre su carpeta, y pide los dos archivos.
- El orquestador probó en la laptop el paquete con `GOAL.hash` agregado a mano, sin red y con un `claude` falso. `r00t corre` pasó el GOAL, la tarea, la config, el oráculo y el ruteo, y llegó a lanzar el builder.

## Qué hacer

1. Escribí primero un test en rojo: el paquete de `empaquetar --corre` trae `GOAL.hash`, igual al de la raíz de Root.
2. Escribí un segundo test en rojo: sin `GOAL.hash` en la raíz, `empaquetar --corre` sale con error de uso que lo nombra, como hoy hace con `GOAL.md`.
3. Copiá `GOAL.hash` junto a `GOAL.md`, y exigilo junto al chequeo de `GOAL.md`.
4. Revisá si algún test de `src/colab_corre.rs` arma una raíz con `GOAL.md` y sin `GOAL.hash`. Si alguno, agregale `GOAL.hash`.

No verifiques que el hash coincida al empaquetar. Esa verificación ya la hace `r00t corre` en el runtime.

## Límites

- Tocá solo `src/colab_corre.rs` y el informe `docs/builds/informe-local-176c.md`.
- No cambies `src/goal.rs` ni `src/corre/`.
- No corras nada con podman. Exportá `ROOT_SIN_PODMAN=1`.

## El informe

El informe trae la tabla de commits, el rojo visto de cada test, la suite completa con `tools/test-seguro.sh` y las "Preguntas para el planner".
