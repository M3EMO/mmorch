# Informe de la Tarea 138

## 1. Resultado de la suite

Corrí `tools/test-seguro.sh` con `ROOT_SIN_PODMAN=1`.

| Momento | Verdes (lib) | Fallas | Ignorados |
|---|---|---|---|
| Base (c038b0e) | 3052 | 0 | 1 |
| Final | 3063 | 0 | 1 |

La base es el final menos los 11 tests nuevos. No la corrí aparte. Los demás binarios quedaron verdes.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La comparación usaba el mínimo y el rango. Una corrida suelta subía la razón y el ruido. | Los 11 tests nuevos. Fallaron con `not yet implemented` sobre esqueletos. Ver sección 3. | ver `git log` |

Guardas: compilación y tamaño con mínimos, y las funciones `comparar` y `comparar_con_ancla` sin cambio.

## 3. Qué falló o quedó incompleto

- El rojo muestra pánicos de esqueleto, no valores. El valor viejo (razón 1,099, ruido 0,1635) quedó como aserción de guarda en el test de t1.
- No corrí la calibración de Eficiencia de t1 a t3. Le toca al orquestador, con podman, junto con la 137.
- No revisé si `informe.rs` dice "mínimo" en algún texto. Ningún test de otros módulos falló.

## 4. Los números del test de t1

| Dato | Hoy (mínimos y rango) | Nuevo (medianas y cuartiles) |
|---|---|---|
| Referencia | 0,263 | 0,2945 |
| Repo | 0,289 | 0,296 |
| Razón | 0,289 / 0,263 = 1,099 | 0,296 / 0,2945 = 1,0051 |
| Ruido | (0,306 − 0,263) / 0,263 = 0,1635 | 2 × (0,2965 − 0,29175) / 0,2945 = 0,0323 |

## 5. Tests que cambiaron de valor

Ninguno. Todos los tests viejos pasaron sin tocar sus expectativas.

## 6. Preguntas para el planner

1. El brief dice ruido de hoy 0,162. La cuenta da 0,1635. Decisión: el test usa 0,1635.
2. Un lado tiene 4 corridas o más y el otro 3. Decisión: el factor 2 y los cuartiles aplican solo al lado con 4 o más. El lado de 3 usa (máx − mín) / mín sin factor.
3. Con ancla, la referencia tiene 11 valores. Decisión: entra completa a la mediana y a los cuartiles.
4. Mediana de referencia no positiva. Decisión: la comparación da `None`. Una mediana de repo no positiva da dispersión 0.
5. El brief dice que los cuartiles de 1 a 5 corridas "dan 2 y 4". Decisión: ese valor vale para la serie 1 a 5. Las otras series dan sus valores interpolados.
6. El hook de commit puede fallar un test de tiempo. Si pasó, el commit se reintentó. Ver el mensaje del commit.
