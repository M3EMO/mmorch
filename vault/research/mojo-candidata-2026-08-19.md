---
title: Mojo — candidata, no accionable hoy (sin carga de entrenamiento que medir)
status: seed
tags: [research, mmorch, mojo, entrenamiento, hardware]
created: 2026-08-19
---

Mojo (Modular) pasó a Apache 2.0 completo el 18/08/2026 (compilador entero,
antes solo la stdlib), tras la adquisición de Modular por Qualcomm. Pregunta
del dueño: ¿sirve para el entrenamiento propio?

## Lo medido (investigación con fuentes, no marketing)

- **CPU numérico**: benchmark independiente (ORNL, arXiv:2509.21039) — Mojo
  llega a 90-95% del ancho de banda teórico en BabelStream, empata o supera
  C++ en miniBUDE. Real, creíble.
- **vs Rust/C++ compilado**: la diferencia real es chica (few %), no
  órdenes de magnitud — el 35.000x/68.000x que promociona Modular compara
  contra Python interpretado sin optimizar, la comparación más fácil de
  ganar posible.
- **GPU para entrenamiento — el hueco real**: CERO autodiff nativo (GitHub
  Discussion #188, el propio equipo de Modular dice que no está en
  desarrollo). El propio roadmap "Path to Mojo 1.0" habla de "escribir
  kernels de alto rendimiento" — nunca de loops de entrenamiento.
- **Adopción real para entrenamiento hoy: ninguna evidenciada.** Un paper
  (arXiv:2606.16059) lo recomienda explícitamente solo para el camino de
  INFERENCIA: "Python y PyTorch siguen siendo naturales para research y
  entrenamiento". El único framework nativo en Mojo (Basalt) se describe a
  sí mismo como "todavía en su infancia" y parece parado desde marzo 2025.

## Por qué no es accionable ahora

mmorch no entrena nada localmente hoy — la memoria de hardware ya documenta
que el entrenamiento local espera la ExpertBook 64GB. No hay carga de
entrenamiento corriendo contra la cual medir si Mojo ayuda; sería adoptar
sin medir, justo lo que la regla propia de selección de lenguaje prohíbe
("nunca Rust por adivinanza" — misma lógica aplica acá).

## Cuándo reconsiderar

Cuando el entrenamiento local arranque de verdad: la interfaz correcta (por
la propia regla de selección de lenguaje) sería un módulo DATA-only vía FFI
para el kernel puntual que resulte ser el cuello de botella medido — nunca
una reescritura completa. En ese punto, además, Mojo va a tener meses más
de maduración desde este release (1.0 recién el 12/08/2026).
