# Informe de la Tarea 156

La tarea no cambia código. El punto 1 quedó fuera por la Prueba 0f. El punto 2 se detuvo: ningún campo existente sirve.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (0f8342f) | 3917 | 0 | 5 |
| Final | 3917 | 0 | 5 |

El código no cambió, así que los dos números coinciden. La biblioteca sola da 3314 verdes y 1 ignorado.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1, las CPU que Root no puede usar | No se hizo. Ver la sección 3. | ninguno | ninguno |
| 2, el aviso en el caso | Se detuvo. Ver la sección 3. | ninguno | ninguno |

El único commit es este informe.

## 3. Qué falló o quedó incompleto

**Punto 1: fuera por la Prueba 0f.**
- En dos VMs de CPU de Colab, `nproc` y `nproc --all` dan 2.
- `/proc/stat` trae 2 líneas `cpuN`, y `Cpus_allowed_list` es 0-1. Entonces `v = n = 2`.
- En reposo, la carga ocupada fue 0,18 y 0,29 núcleos. `top` no mostró ningún proceso que gire.
- La VM no tiene CPU extra, así que la resta `v − n` valdría 0. El punto 1 no cambiaría nada.

**Punto 2: detenido, sin cambio de código.** El brief pide parar si ningún campo existente sirve.

El camino del aviso hoy es este:
- `Vigilancia::aviso` arma la línea "eficiencia: mayor carga ajena de una corrida: X núcleos de N".
- `medir_contra_referencia` la suma a `avisos`.
- `oracle/mod.rs` la pasa a `Veredicto.avisos`.
- `CasoGuardado::de`, en `src/oracle/calibra.rs`, no copia `veredicto.avisos`. El aviso se pierde ahí.

Revisé los campos de texto de `CasoGuardado` y de `MedidaEficiencia`:

| Campo | Por qué no sirve |
|---|---|
| `CasoGuardado.motivo` | Dice por qué `detectado` es true. Un aviso lo confundiría con una detección. |
| `CasoGuardado.sin_veredicto` | Dice por qué la dimensión quedó sin veredicto. El caso de Eficiencia sí tiene veredicto. |
| `CasoGuardado.filtro` | Es el `-F` de cargo-mutants. |
| `MedidaEficiencia.motivo` | Dice "por qué la medida no tiene valor". Ver abajo. |

`MedidaEficiencia.motivo` es el único candidato técnico. Tiene dos problemas:
- Cambia su significado: hoy `None` equivale a "la medida tiene valor".
- `puntaje_de_eficiencia`, en `src/oracle/mod.rs`, junta esos motivos en el texto de "sin veredicto" cuando ninguna medida tiene valor. Un aviso con valor se mezclaría con los motivos reales.

**Opciones para el planner:**
- **A.** Campo nuevo `carga_ajena: Option<f64>` en `CasoGuardado`, con `#[serde(default)]`. Es el más claro y pide cambio de esquema. Se guardaría el valor numérico, no el texto.
- **B.** Campo nuevo `avisos: Vec<String>` en `CasoGuardado`. Lleva también los demás avisos del veredicto. Pide cambio de esquema.
- **C.** Escribir el aviso en `MedidaEficiencia.motivo` de `tiempo_tests` y `tiempo_compilacion`. No cambia el esquema. Obliga a cambiar la regla de `puntaje_de_eficiencia`.

**Recomendación del organizador:** la opción A. El umbral se revisa con un número, y un número no necesita parseo. Si el planner quiere el texto del aviso, la opción B lo cubre.

Si el planner elige A o B, el archivo de producción es `src/oracle/calibra.rs`, además de `src/oracle/eficiencia.rs`. El esquema de `CasoGuardado` es JSON Schema, así que el cambio llega a quien lo lea.

## 4. Dónde quedó el aviso

No quedó en ningún campo del renglón. Los casos siguen sin llevar la mayor carga ajena. Ejemplo de la línea que se pierde:

`eficiencia: mayor carga ajena de una corrida: 0.30 núcleos de 2`

## 5. Tests que cambiaron de valor

Ninguno. No se editó ningún test de otro archivo.

## 6. Llamadores revisados

No se cambió ninguna firma. No hizo falta `codegraph`.

## 7. Preguntas para el planner

1. ¿Qué opción toma el aviso: A (campo numérico nuevo), B (lista de avisos nueva) o C (reusar `MedidaEficiencia.motivo`)?
   - Decisión del organizador: no se tomó. Se recomienda A, y la tarea queda detenida hasta tu respuesta.
2. ¿El punto 1 se reabre? Con `v = n = 2` en Colab, la carga ajena de 1,00 núcleos no viene de CPU invisibles.
   - Decisión del organizador: no se hizo. La explicación B (un proceso real ocupa un núcleo) sigue abierta. La Prueba 0f no vio un proceso que gire, y la carga en reposo fue menor de 0,3 núcleos.
