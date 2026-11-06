# Método de las fichas del dataset (para aprobar, P81)

**Qué aprobás.** Este método, y las fichas de las 3 primeras tareas. Las demás siguen el método, y te muestro su lista. Las fichas todavía no existen: salen de la minería.

## De un commit a una ficha

1. Un commit público de Rust arregla algo y trae su test.
2. Un chequeo confirma que el test falla antes del commit y pasa después, 3 veces, sin red.
3. Un modelo escribe la ficha desde la descripción del commit y su diff. La ficha pide la conducta, no el arreglo.
4. `r00t ficha revisar` aplica sus reglas deterministas: partes, aceptación medible, vaguedad y pruebas.
5. El chequeo de fuga rechaza la ficha si cita un identificador nuevo del diff o copia una línea agregada.
6. El estado anterior al commit es el repo inicial. Los tests del commit se reparten entre contrato y oculto.

## Qué decide un modelo y qué no

| Paso | Quién decide |
|---|---|
| Si el defecto existía | El test: rojo antes, verde después |
| Si Root lo encontró | Archivo y línea, a 3 líneas o menos |
| El texto de la ficha | Un modelo escribe, y las reglas lo filtran |
| Si una ficha entra | Reglas y chequeo de fuga. En las 3 primeras, vos |

## Qué puede salir mal

- **Una ficha pide de más o de menos.** Por eso revisás las 3 primeras.
- **El chequeo de fuga no ve una pista indirecta.** Una regla no ve todo. La lista queda a tu vista, y una ficha se puede sacar.
- **Las otras 17 no las leés.** Tu decisión 12 pedía aprobar los requisitos. Con P81 aprobás el método en su lugar.

## Lo que recibís con las 3 primeras

Para cada una: el enlace al commit, la ficha, el test que queda visible, la cantidad de tests ocultos sin sus nombres, y el resultado del chequeo de fuga. Respondés "sí", o qué cambiar en el método.
