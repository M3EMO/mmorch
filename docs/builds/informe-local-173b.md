# Informe 173b: `sin_ip6tables_frena_antes_de_crear_el_espacio` en Fedora

**Falla.** En Fedora, `bash` e `ip6tables` viven en `/usr/bin`. `path_del_test(e, true)` quitaba esa
carpeta del PATH y `Command::new("bash")` no encontraba `bash` (`NotFound`, `tests/red_builder.rs:204`).

**Arreglo (solo helpers de `tests/red_builder.rs`; ni el guion ni las aserciones cambian).**
- `correr_con_path` lanza `bash` por ruta absoluta, resuelta con el PATH real del test antes del filtro.
- Con `sin_ip6tables`, el PATH del guion es `falsos` + una carpeta propia `reales` con enlaces a
  `cat cut grep dirname mkdir touch chmod`. Ya no depende de dónde viva `ip6tables`.

**Rojo.** Carpeta temporal con enlaces a todo `/usr/bin` más un `ip6tables` falso, como único PATH
(más `~/.cargo/bin`): `PATH="$F:/root/.cargo/bin" tools/test-seguro.sh --test red_builder`.
Antes: 7 verdes, 1 falla, con el mismo panic de Fedora. Después: 8 verdes.

**Suite completa** (`ROOT_SIN_PODMAN=1 tools/test-seguro.sh`): 4219 verdes, 0 fallas, 5 ignorados.
