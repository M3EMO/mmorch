# Informe de la Tarea 136

## 1. Resultado de la suite

Corrida con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes (lib) | Fallas | Ignorados |
|---|---|---|---|
| Base (eed1225) | 3036 | 0 | 1 |
| Final (8014d83) | 3041 | 0 | 1 |

La base es el final menos los 5 tests nuevos. No la corrí aparte. Los demás binarios quedaron verdes.
El hook de commit no falló por tiempo.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El lastre caía en una función vieja que ningún test viejo llama | `lastre_de_memoria_con_dos_viejas_va_a_la_que_llama_el_test_viejo`, `lastre_de_memoria_test_nuevo_no_hace_candidata_a_su_llamada`, `lastre_de_memoria_sin_test_viejo_que_llame_a_una_funcion_no_aplica`, `llamados_de_tests_viejos_ignora_macros_y_palabras_clave` | 8014d83 |

Guarda sin rojo real: `lastre_de_memoria_repo_chico_con_test_viejo_compila`. Pasaba con el código viejo.

## 3. Qué falló o quedó incompleto

- La calibración de Eficiencia de t1 a t3 queda para el orquestador. Necesita podman.
- `llamadas_en` no lee genéricos como `foo::<T>(`.
- Solo se detecta `#[test]` exacto. No se detectan `#[tokio::test]` ni otros atributos.
- Un nombre común puede coincidir con otra función. La llamada puede estar en una rama que el test no recorre.
- Un nombre local, como `helper(` dentro de un test, también cuenta como llamado.

## 4. El lugar del lastre en el repo de prueba

- Archivo: `src/a.rs`. Función: `vieja`. Línea del lastre: la de debajo de `pub fn vieja(x: u32) -> u32 {`.
- Test viejo que la llama: `llama_a_vieja`, con `assert_eq!(vieja(1), 2)`.
- `nueva` es vieja también, pero ningún test viejo la llama. Nunca recibe el lastre, con las semillas 1 a 20.

## 5. Tests que cambiaron de valor

Los dos tests están en `inyectar.rs`. Antes bastaba con que la función existiera en `bootstrap/`.
Ahora hace falta un test viejo que la llame, así que agregué ese test al estado inicial y al repo.

- `lastre_de_memoria_va_a_la_funcion_vieja_y_no_a_la_nueva`: suma el test viejo `llama_a_vieja`.
- `lastre_de_memoria_con_el_cambio_solo_en_nuevas_va_a_una_vieja_de_otro_archivo`: `src/b.rs` suma el test `llama_a_tres`.

No edité tests de otros archivos.

## 6. Preguntas para el planner

1. ¿Se aceptan atributos como `#[tokio::test]` como tests viejos? Decisión: no. Solo `#[test]`.
2. ¿Un nombre llamado solo dentro de un test cuenta como candidato? Decisión: sí. Es la regla del brief, sin filtro extra.
3. La base de 3036 es una resta, no una corrida. Decisión: la anoto así, sin correr el commit anterior.
