# Informe de la Tarea 156 (segunda vuelta)

El caso de Eficiencia guarda su mayor carga ajena como número. El campo es `CasoGuardado.carga_ajena`. Este informe reemplaza el de la primera vuelta.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (8570e9e) | 3917 | 0 | 5 |
| Final | 3926 | 0 | 5 |

Los 9 verdes nuevos son los tests de este punto. El hook del commit corrió la suite y pasó sin reintento.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1, la carga ajena como número | `CasoGuardado` no guardaba la carga ajena. El valor moría en `CasoGuardado::de`. | `caso_de_eficiencia_guarda_la_carga_ajena_en_el_renglon`, `la_carga_ajena_llega_a_cada_resultado_de_dos_lados`, `la_carga_ajena_llega_a_cada_resultado_contra_referencia` | eb0f2ce |

Guardas que nacieron verdes:
- `sin_fuente_de_carga_no_hay_carga_ajena`
- `sin_root_cpu_us_no_hay_carga_ajena`
- `caso_de_eficiencia_sin_carga_en_la_base_guarda_none`
- `caso_de_otra_dimension_guarda_carga_ajena_none`
- `renglon_viejo_sin_carga_ajena_se_lee_como_none`
- `veredicto_viejo_sin_carga_ajena_lee_none`

## 3. Qué falló o quedó incompleto

- **Sin puntos incompletos.**
- **Tercer archivo tocado: `src/oracle/mod.rs`.** Solo lleva el número de `ResultadoMedida` al `Veredicto`.
- **Un cuarto archivo: `docs/interfaces.md`.** El brief pide regenerarlo. Lo regeneró `ROOT_ACTUALIZAR_SNAPSHOT=1 tools/test-seguro.sh --test interfaces`. El diff agrega solo `carga_ajena`.
- **El test de 0,30 núcleos no es exacto en `eficiencia.rs`.** El valor sale del reloj real. Esos tests acotan la carga a (0,1; 1,0] y exigen el mismo valor en las cuatro medidas. El test de `calibra.rs` sí usa 0,30 exacto, puesto en la base.
- **Podman.** Ningún test nuevo lo necesita. La suite con podman queda para el orquestador.

## 4. El camino del número

1. `Vigilancia.mayor` (`Option<f64>`) ya guardaba la mayor carga ajena. Es `None` sin fuente o sin `ROOT_CPU_US`.
2. `medir_los_dos_lados` y `medir_contra_referencia` llaman a `con_carga_ajena`. Esa función copia `mayor` a cada resultado.
3. **Campo nuevo:** `ResultadoMedida.carga_ajena: Option<f64>`.
4. `medir` de `oracle/mod.rs` toma el máximo de `ResultadoMedida.carga_ajena` y lo pone en `Medicion`.
5. **Campo nuevo:** `Medicion.carga_ajena` (struct interno).
6. **Campo nuevo:** `Veredicto.carga_ajena: Option<f64>`, con `#[serde(default)]`.
7. **Campo nuevo:** `CasoGuardado.carga_ajena: Option<f64>`, con `#[serde(default)]`. `CasoGuardado::de` lo copia del veredicto. `de_no_aplicable` lo deja vacío.

El aviso de texto en `Veredicto.avisos` no cambió.

## 5. Tests que cambiaron de valor

- Ningún test cambió de valor.
- No edité tests de otros archivos.

## 6. Llamadores revisados

La única firma cambiada es el struct `ResultadoMedida`, con un campo nuevo. Sus literales viven solo en `eficiencia.rs`.
- `medir`, `medir_con` y `medir_lados` no cambian de firma.
- `codegraph callers medida_de_eficiencia`: `medir` en `oracle/mod.rs`.
- `grep -w` de `medir_lados`, `medir_contra_referencia` y `medir_los_dos_lados` fuera de `eficiencia.rs`: cero resultados.
- Un cero de una herramienta no prueba nada. El compilador lo confirma: la suite compila con los literales nuevos.

## 7. Preguntas para el planner

1. **`CasoGuardado::de` no filtra por dimensión.** Copia la carga del veredicto. Hoy solo Eficiencia la llena. Decisión del organizador: sin filtro, igual que `eficiencia`. ¿Se agrega un filtro explícito?
2. **El valor es el de la medición propia del caso.** No incluye la carga de la base que hereda `referencia.carga`. Decisión del organizador: solo la propia, como dice el brief.
