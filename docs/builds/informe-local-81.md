# Informe de la Tarea 81

Rama `laptop/tarea-81`, base 89fe8f2. Todo corrió con `ROOT_SIN_PODMAN=1`.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 89fe8f2 | 2808 | 0 | 4 |
| Final d67ab37 | 2841 | 0 | 4 |

Ningún test de tiempo falló. No hubo reintentos de commit por tiempo.
El primer commit del punto 1 y el del punto 4 fallaron por otras causas, ya corregidas (ver sección 3).

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El juez no ve el cambio | Ventana 860 a 940, archivo nuevo primero, ventanas unidas a 50 líneas, secreto omitido, total de 40.000, rangos de `cambios.rs` | b00b12a |
| 2 | El resultado no guarda lo que el juez leyó | Juicio con ventana de cambio y archivo grande guarda ambos | 46a035e |
| 3 | Un hallazgo puede citar una línea que el juez no leyó | Línea 500 con extracto 1 a 150 queda descartada | ba7d2f6 |
| 4 | `Fila` no guarda huella del juez | Huella vieja, sin huella y huella actual en Diseño; solo sondas con huella distinta; motivo; `calibrar_con` escribe la huella | d67ab37 |

Guardas: sin `bootstrap/` los extractos son idénticos a los de hoy. La huella del prompt cambia.
Un `ResultadoJuez` sin `extractos` se lee vacío. Un hallazgo en la línea 100 sigue válido.
Una fila solo de sondas, sin huella, sigue calibrando. Un JSON de `Fila` sin `huella` se lee `None`.

## 3. Qué falló o quedó incompleto

- **Punto 4, motivo visible: incompleto.** `Puntaje` no tiene campo de motivo. Cablear "calibración de otro juez" exige `src/tipos.rs`, `src/oracle/mod.rs` e `informe.rs`, fuera de la lista. Quedó `Calibraciones::motivo_sin_calibrar`, con test, sin uso en el veredicto.
- **Archivos fuera de la lista, tocados solo para compilar.** Cada cambio es una línea de campo nuevo.
  - `tests/juez_tope.rs`: `inicial: None`.
  - `src/oracle/mod.rs`: dos literales de test con `extractos`, uno con `huella: None`, y el orden alfabético de `mod cambios;` que pide el hook de formato.
  - `tests/calibra.rs`: tres literales `Fila` con `huella: None`.
- **Commits rechazados por hooks.** El primero del punto 1 falló por el orden de `mod cambios;`. El del punto 4 falló porque `docs/interfaces.md` estaba viejo. Se corrigieron y no hubo `--no-verify`.
- **Verificación de bootstrap.** `armar` en `src/corre/mod.rs` usa `home/oraculos/<tarea>/bootstrap`. `juzgar_diseno` usa `tarea.dir/bootstrap`, que es la misma carpeta.
- **Defecto de revisión corregido en el punto 1.** La ventana de cambio guardaba el `hasta` pedido, no la última línea escrita. El punto 3 habría aceptado líneas recortadas. Quedó la última línea escrita.
- **No probado:** la vuelta 73 (calibrar Arquitectura y Diseño en t1). Necesita API y la corre el orquestador.
- **Limpieza pendiente.** Quedan los worktrees `~/.cache/root-wt/local81-p4` y `~/.cache/root-wt/local81-base` y la rama `laptop/tarea-81-p4`. Un hook frenó el borrado. El orquestador puede quitarlos.
- **Codegraph.** El comando `codegraph.cmd` no existe en esta máquina. La versión instalada es `codegraph` 1.6.2 y no se corrió. Se usó grep.

## 4. Tests que cambiaron de valor

Ninguno cambió de aserción.
Los literales de tests existentes solo ganaron el campo nuevo.
Ningún test viejo de `aplicar` esperaba calibrada una fila de Diseño, Arquitectura o Estructura.

## 5. Llamadores revisados

Se revisó con `grep -w`. Codegraph no se corrió.

| Cambio | Llamadores |
|---|---|
| `EntradaJuez.inicial` (campo nuevo) | `mod.rs` `juzgar_diseno`; `tests/juez_tope.rs`; tests de `juez.rs` |
| `ResultadoJuez.extractos` (campo nuevo) | `juez.rs`; dos literales de test en `mod.rs` |
| `Fila.huella` (campo nuevo) | `calibra.rs` (`fila_de`, `fila_no_aplica`, tests); `tests/calibra.rs`; un test de `mod.rs` |
| `extractos`, `construir_prompt_con`, `reintentar`, `sin_veredicto`, `Contexto` | solo dentro de `juez.rs` |
| `huella_del_prompt` | `compartidos.rs` (clave de limpios) y `calibra.rs`. La huella nueva invalida los limpios guardados. |

## 6. Preguntas para el planner

1. **Huella en dimensiones solo de sondas.** Cualquier cambio del prompt del juez cambia la huella de todas las dimensiones. Una fila de Seguridad con huella vieja deja de calibrar. Decisión: se siguió el brief tal cual. Una fila solo de sondas vale sin huella, pero no con huella distinta.
2. **Cambio mayor que `extracto_max`.** Una ventana de más de 6000 caracteres se corta al final, y la parte cambiada puede quedar fuera. Decisión: se respetó el tope y no se partió la ventana.
3. **Archivo con cambio que no entra por el tope.** Puede entrar después desde su comienzo, como archivo grande o de sonda. Decisión: se dejó así.
4. **Hallazgo sin extractos.** Con la lista de extractos vacía, la validación de línea no descarta nada. Decisión: no hay contra qué validar, y el gate de evidencia sigue activo.
5. **Orden de motivos de descarte.** Un hallazgo con requisito inválido reporta ese motivo aunque su línea también quede fuera. Decisión: la línea se valida justo antes del gate de evidencia.
6. **Motivo visible.** ¿Se autoriza tocar `tipos.rs`, `mod.rs` e `informe.rs` para mostrar "calibración de otro juez"? Decisión: no se tocaron.
7. **Efecto conocido.** Al mezclar, las filas guardadas de memora y brasa en dimensiones de juez dejan de calibrar. No se recalibraron.
