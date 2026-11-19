# Informe de la Tarea 164

Rama `laptop/tarea-164`, base a88e0fb. Se construyó en la laptop, en el worktree `local121`, con `ROOT_SIN_PODMAN=1`.
Cada parte la construyó un subagente Haiku con su test en rojo. El organizador revisó cada diff antes del commit.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base a88e0fb (dato del plan) | 3989 | 0 | 5 |
| Final | 4024 | 0 | 5 |

El hook de pre-commit pasó en los cinco commits al primer intento. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 0 | `corre` dejaba vivos a los nietos de un proceso vencido | `corre_proceso_vencido_mata_al_nieto`, `corre_proceso_vencido_con_nieto_termina_rapido`, `corre_proceso_vence_el_plazo_y_no_espera_el_fin` | 0e5ac3a |
| 2 | El hallazgo no traía el arreglo. Los cortes de Python caían en `Otro` | `descarga_hallazgo_lock_no_cuadra_dice_como_arreglar`, `..._sin_paquete_...`, `..._version_sin_seleccion_...`, `..._generico_...`, `..._lock_invalido_...`, `descarga_python_corte_no_instala_da_tipo_no_instala`, `descarga_python_git_no_seguro_da_fuente_no_aceptada`, `descarga_python_hallazgo_no_instala_dice_como_arreglar` | b3485f7 |
| 3 | `rompe_la_compilacion` contaba un corte de la descarga | `rompe_la_compilacion_corte_de_descarga_no_cuenta`, `corte_de_motivo_ida_y_vuelta_para_cada_variante`, `corte_de_motivo_motivo_ajeno_da_none`, `corte_del_veredicto_sin_cortes_da_none_y_con_corte_da_some` | fe3ba71 |
| 1 | Un producto que no instala no volvía al builder | 8 tests `corre_instalacion_*` (relanza, instala, 2 vueltas con aviso, prompt sin ids, sin cupo, eficiencia después, evaluación rota, rol con código 1) | ea86780 |
| 4 | `apt-get` sin reintentos en el runtime | `bubblewrap_reintenta_el_par_entero_hasta_andar`, `apt_que_siempre_falla_sale_con_su_codigo_tras_3_intentos`, `bwrap_presente_no_llama_a_apt_get` | 96c0c51 |

El orden fue 0, 2, 3, 1, 4. El punto 1 necesita el lector de cortes del punto 3 (ver pregunta 1).
Guardas verdes sin tocar: `corre_proceso_vencido_conserva_lo_que_alcanzo_a_dar`, los tests de `contenedor.rs`, los de `diagnosticar`, los `rompe_la_compilacion_*`, los `corre_eficiencia_*` y `regresiones_de_vuelta_*`, y los demás de `colab_preparar.rs`.
La fila vieja sin `vuelta_instalacion` se lee con `None`.

## 3. Qué falló o quedó incompleto

- **Sin podman:** el test `descarga_dependencia_inexistente_sin_lock_da_no_instala` no se corrió. Solo compila. Queda para el orquestador.
- **Sin corrida real:** `corre` sobre b8d0db4058 queda para el orquestador. Tampoco corrí la validez, `calibrar` ni la pasada.
- **Ctrl-C real:** no lo probé a mano. Los tests de señal pasan.
- **Mensaje del aviso:** con 1 vuelta dice "1 vueltas corridas". Lo dejé como pide el brief.
- **`inventario.rs:1013`** arma su propio "Cargo.lock inválido: …" sin el arreglo. Está fuera de mi lista. No lo toqué.
- **Codegraph:** no lo usé. Los llamadores salen de `grep -w`.

## 4. El prompt de la vuelta de instalación

Con un hallazgo falso y un prompt de rol "Construí la tarea y hacé commit.":

```
El producto no instala. Arreglá lo siguiente y dejá el repo listo. Root vuelve a evaluar.
- el producto no instala: error falso. Arreglo: corré `cargo update --workspace`
Construí la tarea y hacé commit.
```

Solo trae el `detalle` de los hallazgos de la fuente `descarga`. No trae ids, citas ni tests.

## 5. Cortes de Python, apt-get y señales

| Motivo en `python.rs` | Tipo ahora | Antes |
|---|---|---|
| `no-instala` | no instala | otro |
| `git-no-seguro` | fuente no aceptada | otro |
| `url-no-soportada` | fuente no aceptada | otro |
| `indice-propio-sdist` | fuente no aceptada | otro |
| `sin-version-fijada` | otro | otro |
| `solo-sdist` | otro | otro |
| `dependencias-dinamicas` | otro | otro |

- **Mensaje de `apt-get`:** "mal bubblewrap: apt-get falló después de 3 intentos (código N)". El paso sale con ese código.
- **Camino de señales del punto 0:** `terminar_por_senal`, `esperar_registrado` y `esperar_o_matar` ya pasaban por `matar_y_recoger`. Ahora esa función llama a `matar_grupo`. Una señal que corta a Root mata el grupo entero. No hizo falta otro manejador.
  Con grupo propio, el Ctrl-C de la terminal no llega al hijo. Solo llega por la limpieza de Root.
- `KILLS_DE_PRUEBA` sigue contando un kill por corte.
- `matar_y_recoger` ahora devuelve el resultado de `wait()`, no el del `kill`.

## 6. Tests que cambiaron de valor

- `tests/descarga.rs`, `descarga_dependencia_inexistente_sin_lock_da_no_instala`: espera `Estado::Rojo`, antes `SinVeredicto` (P121 a). El resto del test sigue valiendo: contrato vacío y hallazgo "el producto no instala". No lo corrí.
- `descarga_paquete_inexistente_y_version_siguen_sobre_cargo_toml` (`descarga.rs`): el detalle ya no es exacto. Ahora usa `starts_with` y `contains("corregí `Cargo.toml`")`.
- `preparar_termina_llamando_a_verificar` (`colab_preparar.rs`): busca "install -y bubblewrap" y las dos opciones de `Acquire`.
- `terminaron_bien` reemplaza el chequeo de `regresiones_de_vuelta`. Compara roles cubiertos, no la cantidad de lanzamientos. Las guardas siguen verdes.
- Archivos de otro punto tocados: `docs/interfaces.md` se regeneró. Solo cambian `Fila`, `VueltaInstalacion` y una frase de `r00t corre`.
- `preparar-runtime.sh`: el cuerpo principal va en `principal`, bajo una guarda de ejecución directa. Así los tests cargan el paso con `source`.

## 7. Llamadores revisados (grep -w, sin codegraph)

- `matar_grupo` (ahora `pub(crate)`): `contenedor.rs:512,516`, `ejecutor.rs:356`.
- `corte_de_motivo` y `corte_del_veredicto` (nuevas, `oracle/mod.rs`): `calibra.rs` (`rompe_la_compilacion`), `corrida.rs` (`corte_de_instalacion`).
- `hallazgo_no_instala`: `descarga.rs` (dos sitios). `hallazgo_lock_invalido`: `descarga.rs:759`. Misma firma.
- `hallazgo_no_instala_python`: cuatro sitios en `python.rs`. Misma firma.
- `corte` (libre en `descarga.rs`): se eliminó. Solo la usaba `python.rs`, que ahora usa `corte_de`.
- `Fila.vuelta_instalacion`: todos los literales de `Fila` usan `..Default` salvo `tipos.rs`, ya ajustado.
- `regresiones_de_vuelta`: misma firma, cambia su chequeo interno.

El orquestador debe correr `codegraph callers` en la laptop sobre estas firmas.

## 8. Preguntas para el planner

1. **Dónde vive el lector del tipo de corte.** `tipo_de_corte` es privado en `informe.rs`, que no se toca. Lo puse en `oracle/mod.rs`, sobre `motivo_de_corte`. Decisión: así el formato queda en un solo lugar.
2. **Cortes de Python `sin-version-fijada`, `solo-sdist` y `dependencias-dinamicas`.** Son límites de Root, no fallos del producto. Quedan en `Otro`, sin vuelta. ¿Se prefiere otro tipo?
3. **`instalo` verdadero si la nueva evaluación da otro corte.** Si tras la vuelta sale "fuente no aceptada", `instalo` queda verdadero y `pide_humano` falso. Decisión: `instalo` solo dice que ya no es un corte de instalación.
4. **Pide humano si la vuelta no corre.** Sin cupo, o con un rol en error, `pide_humano` es verdadero y el aviso dice "la vuelta del builder no terminó".
5. **Un rol con código distinto de 0 y un producto que no instala.** No hay vuelta, `vuelta_instalacion` queda `None` y no sale aviso. Es lo que pide el brief. El caso queda sin marca.
6. **Vueltas agotadas.** El aviso va en `avisos` y `motivo` queda `None`. No escribe `Gate`. Los fallos sí escriben `Gate` de fase `instalacion`.
7. **`ROOT_APT_ESPERA` no se valida.** Un valor no numérico haría fallar el `sleep`.
8. **`inventario.rs:1013`** arma su propio motivo de lock inválido sin el arreglo. ¿Va en otra tarea?
