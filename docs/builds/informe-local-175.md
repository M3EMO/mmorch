# Informe de la Tarea 175: la red del builder en Colab

Base: `laptop/cadena-174` (f7fd89b). Rama: `laptop/tarea-175`. No usé `codegraph`: usé `grep`.
Solo toqué `tools/colab/red-builder.sh`, `tests/red_builder.rs` y este informe.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (f7fd89b) | 4243 (*) | 0 | 5 |
| Final | 4258 | 0 | 5 |

(*) La base se deduce: `tests/red_builder.rs` pasó de 8 a 23 tests, y el resto no cambió. La suma de la base solo se vio parcial (el binario de la librería: 3617 verdes, 1 ignorado).
Ningún test de tiempo falló en el hook. No hubo reintentos de commit por tiempo.
El punto 1 falló una vez en el hook por `cargo fmt --check`. Corrí `cargo fmt` y reintenté.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El espacio nacía con `ip netns add`, sin espacio de usuario de root | `unshare_y_attach`, `con_sosten_vivo_y_nombre_puesto_no_crea_nada`, `con_nombre_puesto_y_sosten_muerto_recrea`, `sin_nsenter_frena_y_lo_nombra`, `segunda_corrida_no_repite_nada` | 9eef837 |
| 2 | Sin `/dev/net/tun` ni `--userns-path` | `sin_tun_lo_crea_con_c_10_200`, `slirp_recibe_userns_path`, `segunda_corrida_no_repite_nada`. Guarda: `con_tun_no_llama_a_mknod` | 8d04577 |
| 3 | Reglas con `ip netns exec` y `sysctl` | `ninguna_llamada_a_sysctl`, `toda_regla_va_detras_de_nsenter`, `politicas_ip6tables_en_drop`, `ruta_ipv6_frena_el_guion`. Guarda: `reglas_de_dns_van_antes_que_los_cortes` | 53b8c1e |
| 4 | El builder no tenía `resolv.conf` propio | `escribe_resolv_del_builder`. Guarda: `no_toca_el_resolv_del_runtime` | 66f7cc9 |
| 5 | Sin defecto: `comprobar_red` no cambia | Ninguno. Guardas: `comprobacion_usa_ip_netns_exec_getent`, `getent_reintenta_antes_de_rendirse`, `la_comprobacion_corre_al_final` | af92159 |

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- **Nada se probó en Colab.** Todos los tests usan programas falsos. Lo real lo prueba el orquestador (sonda y prueba de aislamiento).
- El guion nuevo usa `mknod -m 0666`. En Colab, `mknod` con modo no se probó aquí.
- `/etc/netns/<red>/resolv.conf`: el guion no usa `chown`. Corre como root, y el dueño ya es root.

## 4. La red, paso a paso

Orden de `principal`. "Host" es el espacio del contenedor de Colab, como root.

| # | Comando | Espacio |
|---|---|---|
| 1 | `command -v` de unshare, nsenter, slirp4netns, mknod, iptables, ip6tables (sale con 1 si falta una) | host |
| 2 | `mkdir -p` de las carpetas de pidfiles, log y `/run/netns` | host |
| 3 | `unshare -Urn sleep infinity &`, pid a `ROOT_SOSTEN_PID` (solo si el sostén no vive) | espacio nuevo, dueño: espacio de usuario de root |
| 4 | Con el nombre puesto y el sostén muerto: `ip netns delete <red>`, se borra el pidfile de slirp y se vuelve a 3 | host |
| 5 | `ip netns attach <red> <pid>` | host |
| 6 | Escribe `/etc/netns/<red>/resolv.conf` (`nameserver 10.0.2.3`, 0644) | host |
| 7 | `mkdir -p /dev/net` y `mknod -m 0666 /dev/net/tun c 10 200` (solo si falta) | host |
| 8 | `nsenter -t <pid> -U -n ip6tables -P OUTPUT/INPUT/FORWARD DROP` | espacio de red y de usuario del sostén |
| 9 | `slirp4netns --configure --disable-host-loopback --mtu=65520 --userns-path=/proc/<pid>/ns/user <pid> tap0`, log en `ROOT_SLIRP_LOG` (0600) | host, sobre el espacio del sostén |
| 10 | `nsenter -t <pid> -U -n iptables -C/-A OUTPUT …`: 2 ACCEPT al DNS 10.0.2.3 (udp y tcp 53), luego 6 DROP con `! -o lo` | espacio del sostén |
| 11 | `nsenter -t <pid> -U -n ip -6 route show default` y `ip -6 addr show scope global`: deben salir vacíos, o el guion sale con 1 | espacio del sostén |
| 12 | `ip netns exec <red> getent hosts api.anthropic.com`, con reintentos | espacio de red solamente (el que usa el builder) |
| 13 | Imprime `red lista` | — |

Variables nuevas, solo para los tests: `ROOT_NETNS_DIR` (`/run/netns`), `ROOT_DEV_NET_TUN` (`/dev/net/tun`), `ROOT_ETC_DIR` (`/etc`).
El builder sigue entrando con `ip netns exec` (solo la red). Ningún paso usa `nsenter -U` para el builder.

## 5. Tests que cambiaron de valor

- `segunda_corrida_no_repite_nada`: cuenta `ip netns attach` en vez de `ip netns add`, y espera `--userns-path=` en slirp.
- `apaga_ipv6_y_pone_drop_en_ip6tables`: se quitó. Esperaba `sysctl` y `ip netns exec`. Lo reemplaza `politicas_ip6tables_en_drop`.
- `sin_ip6tables_frena_antes_de_crear_el_espacio`: ahora comprueba que no hay ninguna llamada `ip netns`, no solo `ip netns add`. Es la misma regla.
- Soporte: `ip` falso con `attach`/`delete`, fakes de `unshare`, `nsenter`, `mknod`, y el escenario pasa las tres variables nuevas.

## 6. Preguntas para el planner

1. **Sostén vivo y nombre ausente.** Decisión: el guion hace `attach` con el sostén existente, sin crear otro. Alternativa: matar el sostén y crear uno nuevo.
2. **Sostén muerto con slirp vivo.** Decisión: se borra el pidfile de slirp y se lanza otro, porque slirp va atado al sostén viejo. El slirp viejo puede quedar huérfano; el guion no lo mata.
3. **Orden de la revisión de IPv6.** Decisión: va después de las reglas IPv4 y antes de `comprobar_red`, no después de `red lista`.
4. **Modo del nodo tun.** Decisión: `mknod -m 0666` en una sola llamada, no `mknod` y luego `chmod`.
