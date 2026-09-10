# Escalacion por niveles
Type: grilling
Status: open
Blocked by: 
Map: ../map.md

## Question

El usuario eligió escalación automática hasta agotar y después avisar, y preguntó si se puede hacer mejor. Diseñar los niveles: (1) fix loop dirigido con presupuesto de vueltas y USD; (2) diagnosticador con razonamiento (reasoner) que lee log + oráculo + diff y produce UNA instrucción precisa por archivo — es lo que Claude hizo en C (I2) y un modelo puede hacerlo; (3) Claude en sesión, con el diagnóstico adjunto; (4) humano. Decidir: qué dispara cada nivel (N fallos, USD, tipo de fallo), qué recibe, qué deja (corrección + chequeo nuevo), y cómo se registra (supervision.md, job en estado gate). ¿El nivel 2 elimina la mayoría de las escalaciones a Claude? Se mide en el ticket 04.
