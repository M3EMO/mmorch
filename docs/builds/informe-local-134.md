# Informe de la Tarea 134

## 1. Resultado de la suite

Corrida con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes (lib) | Fallas | Ignorados |
|---|---|---|---|
| Base (56fd3eb) | 3011 | 0 | 1 |
| Final | 3017 | 0 | 1 |

La base es el final menos los 6 tests nuevos. Los demás binarios de test quedaron verdes y sin cambios.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El lastre caía en funciones nuevas que los tests comunes no corren | `lastre_de_memoria_va_a_la_funcion_vieja_y_no_a_la_nueva`, `lastre_de_memoria_con_el_cambio_solo_en_nuevas_va_a_una_vieja_de_otro_archivo`, `lastre_de_memoria_sin_ninguna_funcion_vieja_no_aplica`, `es_funcion_vieja_solo_si_el_texto_inicial_la_declara` | c922cbd |

Guardas verdes desde el inicio: `lastre_de_memoria_con_inicial_la_copia_compila`, `es_funcion_vieja_sin_estado_inicial_es_siempre_vieja` y los tests de la Tarea 131.

## 3. Qué falló o quedó incompleto

- **El motivo "ninguna función existía en el estado inicial" no está.** El motivo sale de `motivo_sin_inyeccion` en `src/oracle/calibra.rs`. La 133 toca ese archivo y el brief lo prohíbe. Hoy el caso sale como no aplicable con el motivo común `MOTIVO_SIN_CANDIDATO`. El inyector ya devuelve `None` sin tocar la copia. Falta una rama `(Defecto::LastreDeMemoria, _) if con_rangos` en `calibra.rs`.
- La calibración de t1 a t3 no se corrió. La corre el orquestador.
- El test con podman no se corrió, por la regla de `ROOT_SIN_PODMAN=1`. Esta tarea no agregó tests con podman.

## 4. El lugar del lastre en el repo de prueba

- Archivo `src/a.rs`, línea 2, función `vieja`. La línea 1 es `pub fn vieja(x: u32) -> u32 {`.
- Cuenta como vieja porque `bootstrap/src/a.rs` declara `fn vieja(`.
- `nueva` no entra: `bootstrap/` no la declara.
- Con el cambio solo en funciones nuevas de `src/a.rs`, el lastre va a `tres` en `src/b.rs`, que `bootstrap/` sí declara.

## 5. Tests que cambiaron de valor

- Ninguno cambió de valor. Los tests de la Tarea 131 siguen verdes sin edición, porque no pasan `inicial`.
- No edité tests de otros archivos.

## 6. Preguntas para el planner

1. ¿Qué hace `en_cambio` cuando el lastre cae por el segundo criterio, en un archivo que el cambio no toca? Decisión: queda en `false`, como pide la regla 2.
2. ¿El motivo nuevo va en `calibra.rs` después de la 133? Decisión: no lo toqué. Lo dejo para el orquestador.
3. Una función es vieja si `bootstrap/` declara el mismo nombre en el mismo archivo. No distingue sobrecargas en `impl` distintos. Decisión: sigo `declara_fn`, igual que `CuerpoVaciado`.
