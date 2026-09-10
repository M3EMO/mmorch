# synth por tipo, seed 47, 6 modelos, 57 tipos x 17 items de test (~969/modelo)

Costo total: $0.75 (deepseek $0.32 + zhipu $0.43). 6 pares (modelo,tipo) caidos por API.

| modelo | promovidos | fallados | bugs perdidos | t_med | p95 | costo |
|---|---|---|---|---|---|---|
| deepseek-chat | 55/57 | 2 | 0 | 0.36s | 0.48s | $0.003 |
| deepseek-v4-pro | 56/56 | 0 | 0 | **0.23s** | **0.32s** | $0.30 |
| deepseek-reasoner | 57/57 | 0 | 0 | 0.29s | 0.44s | $0.02 |
| glm-4.5-air | 52/52 | 0 | 0 | 0.16s | 0.37s | $0.17 |
| glm-5.2 | 56/56 | 0 | 0 | 0.16s | 0.21s | $0.24 |
| glm-5.2-nothink | 55/57 | 2 | 0 | 0.16s | 0.24s | $0.02 |

Cero bugs perdidos en 6 modelos. Solo fallan los dos SIN razonamiento, uno por familia.

## Primer co-fallo cross-familia de la semana: cadenas_sin_ab

deepseek-chat y glm-5.2-nothink escribieron LA MISMA funcion mal:

    a, b, c = 1, 1, 1
    for _ in range(n - 1):
        a, b, c = a + b + c, a + c, a + b + c      # b' deberia ser b + c, no a + c
    return a + b + c

Mismo estado, misma inicializacion, mismo slip (b' = a + c). Familias distintas, mismo
bug, byte a byte salvo el estilo. phi(chat, 5.2-nothink) = +0.48 con 1 co-fallo en 57.

Lectura: para este tipo, cross-family NO decorrelaciono. El error no viene de la familia;
viene de un prior compartido (la solucion "canonica" mal recordada) que el razonamiento
corrige y el no-razonamiento reproduce. Es n=1, pero es el unico dato directo sobre la
hipotesis OneFlow en codigo, y va en contra. Los modelos CON razonamiento de las dos
familias no fallaron el tipo.

Los otros fallos no se comparten: deepseek-chat tiling_2xn (recurrencia con terminos
cambiados), glm-5.2-nothink digitos_crecientes.

## Eficiencia del codigo escrito (tiempo de sandbox, incluye ~0.15s de arranque)

glm-5.2 y glm-4.5-air escriben el codigo mas rapido (0.16s mediana). Entre deepseek,
v4-pro (0.23s) le gana a reasoner (0.29s) y a chat (0.36s). Cero timeouts en 969 items
por modelo, incluido libres_cuadrados_grande (n hasta 2e6).
