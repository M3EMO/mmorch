# Informe de la Tarea 141

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (c038b0e) | 3627 | 0 | 5 |
| Final | 3671 | 0 | 5 |

El hook de commit no falló por tiempo en ningún punto. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Falta el paquete de ida. | Los 12 de `colab_calibra::tests`, con `todo!()`. | 853eed3 |
| 2 | Falta la importación con control de la base. | Los 11 de `colab_calibra::tests_importar`, con `todo!()`. | c7a64e8 |
| 3 | `activo.json` guarda una sola sesión y no hay ciclo en paralelo. | 6 de 7 nuevos de `colab.rs` y los 14 de `tests_calibrar`. | b1c3741 |

Guardas que ya pasaban: lectura de `activo.json` v1, `es_nombre_secreto`, `valores_distintos_agrupa_con_tolerancia`.

## 3. Qué falló o quedó incompleto

- Nada se probó contra Colab real ni contra podman real. Eso queda para el orquestador.
- `tools/colab/` no existe en esta base: lo trae la Tarea 140. `empaquetar` falla con un mensaje claro sin él. Los tests lo crean falso.
- `upload` deja el archivo en `/content/paquete.tar.gz` y `download` usa el orden `-s sesión remoto destino`. Ambos supuestos no se verificaron con la CLI real.
- El runtime de CPU se pide con `new -s <sesión>`, sin `--gpu`. No se verificó que la CLI lo acepte.
- Si `stop` falla después de bajar la vuelta, la vuelta se pierde y la sesión queda en `activo.json`.
- Los `trabajo-<i>.log` quedan en el runtime. Solo se informan los códigos de salida.
- El paquete de vuelta lo arma la laptop con lo que baja. El SHA-256 protege solo desde ese armado, no desde el runtime.
- Los archivos de un repo con nombre de clave o token, por ejemplo `token.rs`, no entran al paquete. Puede romper su compilación en Colab.

## 4. Los comandos nuevos

Los flags cambiaron de nombre solo en `--rustup-sha256` y `--raiz-root`, que son nuevos.

```
r00t colab empaquetar [OPTIONS] --binario <BINARIO> --trabajos <TRABAJOS> --repo <REPO> --rustup-sha256 <RUSTUP_SHA256> --salida <SALIDA>
      --binario <BINARIO>              Binario `r00t` para x86-64 que entra al paquete
      --trabajos <TRABAJOS>            Trabajos `tarea:dimension`, separados por coma
      --repo <REPO>                    Repo bueno de una tarea, como `tarea=ruta`. Se repite por cada tarea
      --rustup-sha256 <RUSTUP_SHA256>  SHA-256 en hexadecimal (64 caracteres) del instalador de rustup
      --salida <SALIDA>                Archivo `.tar.gz` de salida
      --raiz-root <RAIZ_ROOT>          Repo de Root: de ahí sale `tools/colab/`. Por defecto, el directorio actual

r00t colab importar [OPTIONS] <ARCHIVO>
      --raiz-root <RAIZ_ROOT>          Repo de Root: de ahí sale el commit de la laptop. Por defecto, el directorio actual

r00t colab calibrar [OPTIONS] --paquete <PAQUETE>
      --paquete <PAQUETE>              Paquete `.tar.gz` de `colab empaquetar`. Se repite: cada paquete va a su runtime
      --raiz-root <RAIZ_ROOT>          Repo de Root: de ahí sale el commit de la laptop. Por defecto, el directorio actual
```

Ejemplo de manifiesto de ida (`manifiesto.json`):

```json
{
  "v": 1,
  "commit_root": "b1c3741...",
  "binario_sha256": "<sha256 de r00t>",
  "archivos": { "versiones.env": "<sha256>", "oraculos/s2-t1/tarea.toml": "<sha256>" },
  "trabajos": [{
    "tarea": "s2-t1", "dimension": "seguridad", "huella": "<huella_de>", "tarea_hash": "<hash>",
    "imagen_id": "sha256:<id>", "comando": "r00t",
    "args": ["oraculo", "calibrar", "--tarea", "s2-t1", "--dimension", "seguridad", "--repo", "repos/s2-t1", "--json"],
    "entorno": { "ROOT_CONTENEDOR_MODO": "bwrap", "ROOT_BWRAP_ENTORNO": "entorno.env", "ROOT_BWRAP_IMAGEN_ID": "sha256:<id>" }
  }],
  "imagenes": { "s2-t1": "sha256:<id>" }
}
```

Paquete de vuelta: `manifiesto.json` (commit, SHA-256 de cada archivo, filas con sus casos, `donde: {lugar: "colab", runtime: "cpu"}`), `verificacion.txt`, `filas.json` y `calibracion/*.casos.jsonl`.

Secuencia por runtime: `new -s S`, `upload`, `exec` de preparar, un `exec` por trabajo, `exec` de cierre, `download`, `stop -s S`. El tope es `TOPE_RUNTIME` de 4 horas.

## 5. El control de la base

- Se compara el conjunto de valores distintos de `valor_base` de los renglones de casos de las dos filas, con tolerancia de 1e-6.
- Los valores salen de `CasoGuardado.valor_base`, en `calibracion/<tarea>-<dimensión>-<ts>.casos.jsonl`.
- La fila de la laptop es la de mayor `ts` con misma tarea, dimensión y huella.
- En Pruebas no se compara el estado de los mutantes de la base. Ese dato no se guarda: `CasoGuardado.mutantes` es por caso. La salida lo avisa siempre.
- Falta guardar, para comparar más: el estado de cada mutante de la muestra de la base, en `Fila` o en un archivo aparte.

## 6. Tests que cambiaron de valor

Ambos en `src/oracle/colab.rs`, por el formato v2 de `activo.json`:
- `colab_activo_se_escribe_antes_de_new`: `v` pasa de 1 a 2 y lee `sesiones[0]`.
- `colab_activo_viejo_sin_campos_nuevos_se_lee`: `leer_activo` devuelve una lista y el test toma `[0]`.

Ningún test de otro archivo cambió. Se agregó la fila de cada comando en `docs/interfaces.md`.

## 7. Llamadores revisados

Funciones de `colab.rs` abiertas a `pub(crate)`: `Limites`, `Fin`, `Parada`, `paso`, `liberar`, `confirmar_parada`, `Confirmacion`, `registrar_pedir`, `registrar`, `validar_sesion`, `args`, `ruta`, `marcador`, `parece_sin_sesion`, `epoch_segundos`, `escribir`, `EntradaActivo`. Solo cambia su visibilidad: no hay llamadores nuevos que revisar fuera de `colab_calibra.rs`.

Funciones cuyo comportamiento cambió, con firma pública igual:
- `liberar_anotado`: ahora libera todas las sesiones. Codegraph y grep: `oraculo_liberar` (`src/lib.rs`), `colab_calibra.rs` y los tests de `colab.rs`. `correr_en_colab` la llama antes de anotar: con una sola corrida da lo mismo que antes.
- `vigilar`: ahora recorre la lista y agrega el resultado. Llamadores: `src/lib.rs` (`oraculo_vigilar`), `tests/colab.rs` y los tests de `colab.rs`.
- `leer_activo`: ahora devuelve una lista. Solo la usan `leer_sesiones` y un test.
- `borrar_activo` y `escribir_activo` se reemplazaron por `quitar_sesion` y `anotar_sesion`. Llamadores: `liberar`, `parar_vigilado`, `correr_en_colab` y `colab_calibra.rs`.

Riesgo: `correr_en_colab` y `colab calibrar` a la vez se estorban, porque `liberar_anotado` para todas las sesiones anotadas.

## 8. Preguntas para el planner

1. **Constantes de la 139.** Las tres constantes guardan el nombre de la variable, y `MODO_BWRAP` guarda el valor `bwrap`. Decisión: así. El orquestador las unifica al encadenar.
2. **Huella o `tarea_hash` distintos.** El brief los lista en la revisión previa, pero también dice que una fila que no pasa no entra. Decisión: la fila no entra, las demás sí.
3. **`--rustup-sha256`.** El empaquetado no baja nada de red, así que el usuario pasa el SHA-256 de `rustup-init`. Decisión: flag obligatorio.
4. **Una imagen por paquete.** Todas las tareas de un paquete deben usar la misma imagen. Decisión: error de uso si no.
5. **Nombres secretos en archivos.** Se omiten del paquete, también en los repos. ¿Querés una lista de excepciones?
6. **Sin `valor_base` en ningún lado.** La fila no entra. Dos conjuntos vacíos serían iguales sin verificar nada.
7. **Stop fallido tras bajar la vuelta.** Decisión: no se importa. ¿Preferís importarla igual y dejar la sesión anotada?
8. **Paquete de vuelta armado en la laptop.** La alternativa es armarlo en el runtime con un script que calcule los SHA-256 allí.
9. **CPU.** `new` va sin `--gpu`. Si la CLI pide un valor explícito, se cambia la constante `ACELERADOR_CPU`.
