# Informe de la Tarea 165

Rama `laptop/tarea-165`, base 53fd716. Se construyó en la laptop, en el worktree `local121`, con `ROOT_SIN_PODMAN=1`.
Cada punto lo construyó un subagente Haiku con su test en rojo. El organizador revisó cada diff antes del commit.
No usé `codegraph`: busqué los llamadores con `grep -w`.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 53fd716 (dato del informe 164) | 4024 | 0 | 5 |
| Final | 4030 | 0 | 5 |

El hook de pre-commit pasó en los dos commits al primer intento. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Un rol con código distinto de 0 y un corte de instalación no dejaba marca. El aviso decía "1 vueltas" | `corre_instalacion_rol_con_codigo_1_pide_humano_con_el_motivo` (NoInstala y LockInvalido), `corre_instalacion_rol_con_codigo_1_no_relanza_aunque_haya_corte_de_instalacion` (valor nuevo), `corre_instalacion_segunda_evaluacion_con_error_deja_el_corte_y_el_aviso` (singular). Guardas: `..._y_producto_que_instala_no_lleva_vuelta`, `..._segunda_vuelta_con_codigo_1_dice_2_vueltas_corridas`, los `corre_instalacion_*` de la 164 | c23e491 |
| 2 | Un `Cargo.lock` ilegible daba un rechazo en el inventario y el corte "fuente no aceptada" | `descarga_inventario_lock_roto_no_da_rechazo_de_lock_invalido`, `oraculo_corte_lock_roto_con_manifiesto_da_lock_invalido_y_no_listo`. Guarda: `descarga_inventario_lock_roto_conserva_rechazo_de_fuente_y_aviso_sin_rev` | d256bbf |

El informe va en un tercer commit.

## 3. Qué falló o quedó incompleto

Nada quedó sin hacer. No corrí podman, `corre`, `calibrar` ni la pasada real. Quedan para el orquestador:
- La suite con podman de la cadena con la 165.
- La corrida real de `corre` sobre b8d0db4058 con su lock real.

Un detalle del punto 1: el aviso "después de N vueltas del builder" no puede salir con 1 vuelta. El bucle solo sale sin motivo con 2. Por eso solo hay test del singular en el aviso "la vuelta no terminó".

## 4. La decisión de un lock roto

| Camino | Antes | Ahora |
|---|---|---|
| Inventario, con `Cargo.toml` | Rechazo "Cargo.lock inválido" → "fuente no aceptada" → `SinDecision` | Sin rechazo. Las fuentes de `Cargo.toml` se revisan con lista de paquetes vacía |
| Evaluación completa, con `Cargo.toml` | `SinDecision` | Llega a `preparar_rust`: "lock inválido" con su arreglo, `NoListo`, rojo |
| Evaluación sin `Cargo.toml` | "lock inválido", `NoListo` | Igual |

## 5. Tests que cambiaron de valor

- `corre_instalacion_rol_con_codigo_1_no_relanza_aunque_haya_corte_de_instalacion` (`src/corre/corrida.rs`): la fila ahora trae `vuelta_instalacion` con 0 vueltas y `pide_humano`. Antes traía `None`. Sigue sin relanzar.
- `corre_instalacion_segunda_evaluacion_con_error_deja_el_corte_y_el_aviso` (`src/corre/corrida.rs`): el texto dice "1 vuelta corrida". Es el singular.
- `oraculo_corte_fuente_no_aceptada_deja_las_nueve_sin_veredicto` (`src/oracle/mod.rs`, test de otro archivo): usaba `LOCK_ROTO` para obtener "fuente no aceptada". Ahora usa `LOCK_SANO` y una dependencia git por `ssh://`. Sus aserciones no cambiaron.

## 6. Llamadores revisados

No cambió ninguna firma existente.
- `vueltas_de_instalacion`: un solo llamador, `corre` (`src/corre/corrida.rs`). Misma firma.
- `inventariar_cargo`: misma firma. Su llamador no cambia.
- `plural_vueltas`: función nueva. La usan solo los dos avisos de `vueltas_de_instalacion`.

## 7. Preguntas para el planner

1. **El brief dice que una dependencia git sin `rev` da rechazo. En el código da un aviso `sin_commit`** ("dependencia git sin commit fijo"). El rechazo de fuente es el del protocolo `ssh://`. Decisión: no cambié el código. La guarda del punto 2 cubre los dos casos: el rechazo ssh y el aviso sin `rev`. ¿Querés que una git sin `rev` sea rechazo?
2. **El motivo del punto 1 nombra el rol con `nombre_rol`** ("el builder salió con código 1 en el rol planifica"). El brief pide "con el rol" sin dar el formato. Decisión: usé ese texto.
3. **El chequeo del código distinto de 0 toma el primer lanzamiento fallido.** Con varios roles fallidos, el motivo nombra solo el primero. Decisión: no los junté.
