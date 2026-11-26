#!/usr/bin/env bash
# Prepara la red del builder de Colab. Corre como root, antes de la primera corrida.
# Primero revisa que existan las herramientas: si falta una, sale con 1 sin tocar nada.
# Crea el espacio de red con un sostén de espacio de usuario de root (unshare) y le pone el nombre
# ROOT_RED_BUILDER con ip netns attach. Pone en DROP las políticas de OUTPUT, INPUT y FORWARD de ip6tables
# dentro del espacio. Como /proc/sys es de solo lectura en Colab, no toca sysctl: revisa que no quede
# IPv6 con ruta por defecto ni dirección global.
# Lanza slirp4netns con su salida en ROOT_SLIRP_LOG (modo 0600), pone las reglas de salida de IPv4
# y comprueba la resolución. iptables, ip6tables e ip corren dentro del sostén con nsenter.
# Es idempotente.
# Uso: red-builder.sh
set -euo pipefail

ROOT_RED_BUILDER="${ROOT_RED_BUILDER:-root-builder}"
ROOT_SLIRP_PID="${ROOT_SLIRP_PID:-/run/root-builder-slirp.pid}"
ROOT_SOSTEN_PID="${ROOT_SOSTEN_PID:-/run/root-builder-sosten.pid}"
ROOT_SLIRP_LOG="${ROOT_SLIRP_LOG:-/run/root-builder-slirp.log}"
ROOT_RED_INTENTOS="${ROOT_RED_INTENTOS:-10}"
ROOT_NETNS_DIR="${ROOT_NETNS_DIR:-/run/netns}"
ROOT_DEV_NET_TUN="${ROOT_DEV_NET_TUN:-/dev/net/tun}"
export ROOT_RED_BUILDER ROOT_SLIRP_PID ROOT_SOSTEN_PID ROOT_SLIRP_LOG ROOT_RED_INTENTOS ROOT_NETNS_DIR ROOT_DEV_NET_TUN

# DNS de slirp4netns. Es lo único que se acepta antes de los cortes.
DNS_SLIRP="10.0.2.3"

# Rangos que se cortan para la salida. Son 6, no 7: el brief nombra 6 rangos.
RANGOS_CORTADOS=(10.0.0.0/8 172.16.0.0/12 192.168.0.0/16 100.64.0.0/10 169.254.0.0/16 127.0.0.0/8)

# Paso 0: sin estas herramientas no se puede armar ni cerrar la red. Se frena antes de crear nada.
revisar_herramientas() {
    local programa
    for programa in unshare nsenter slirp4netns mknod iptables ip6tables; do
        if ! command -v "$programa" >/dev/null 2>&1; then
            echo "mal red-builder: falta $programa" >&2
            exit 1
        fi
    done
}

# Crea las carpetas de los pidfiles, del log y de los nombres de red si no existen.
preparar_pidfiles() {
    mkdir -p "$(dirname "$ROOT_SLIRP_PID")" "$(dirname "$ROOT_SOSTEN_PID")" "$(dirname "$ROOT_SLIRP_LOG")" "$ROOT_NETNS_DIR"
}

# Lanza el sostén: un proceso en espacios de usuario y de red nuevos de root. Su pid va a ROOT_SOSTEN_PID.
lanzar_sosten() {
    unshare -Urn sleep infinity </dev/null >/dev/null 2>&1 &
    echo "$!" >"$ROOT_SOSTEN_PID"
}

# Paso 1: deja el nombre ROOT_RED_BUILDER puesto sobre el espacio del sostén vivo.
# Si el nombre ya está puesto y el sostén murió, borra el nombre, lanza un sostén nuevo y lo pone de nuevo.
asegurar_espacio() {
    if [ -e "$ROOT_NETNS_DIR/$ROOT_RED_BUILDER" ]; then
        if pid_vivo "$ROOT_SOSTEN_PID"; then
            return 0
        fi
        ip netns delete "$ROOT_RED_BUILDER"
        # slirp va atado al sostén viejo: se borra su pidfile para que se lance de nuevo.
        rm -f "$ROOT_SLIRP_PID"
        lanzar_sosten
        ip netns attach "$ROOT_RED_BUILDER" "$(cat "$ROOT_SOSTEN_PID")"
        return 0
    fi
    if ! pid_vivo "$ROOT_SOSTEN_PID"; then
        lanzar_sosten
    fi
    ip netns attach "$ROOT_RED_BUILDER" "$(cat "$ROOT_SOSTEN_PID")"
}

# Paso 1b: asegura el nodo de tun (ROOT_DEV_NET_TUN). Si ya existe, no lo toca ni llama a mknod.
# Si no existe, lo crea con mknod -m 0666 c 10 200. 0666 es el modo de siempre: abrir el nodo
# no da permiso para crear una interfaz.
asegurar_tun() {
    if [ -e "$ROOT_DEV_NET_TUN" ]; then
        return 0
    fi
    mkdir -p "$(dirname "$ROOT_DEV_NET_TUN")"
    mknod -m 0666 "$ROOT_DEV_NET_TUN" c 10 200
}

# Prefijo de cada llamada al espacio: entra al sostén (pid de ROOT_SOSTEN_PID) en sus espacios de usuario y de red.
en_sosten() {
    local pid
    pid="$(cat "$ROOT_SOSTEN_PID")"
    nsenter -t "$pid" -U -n "$@"
}

# Paso 2: pone en DROP las tres políticas de ip6tables dentro del espacio.
cerrar_ipv6() {
    en_sosten ip6tables -P OUTPUT DROP
    en_sosten ip6tables -P INPUT DROP
    en_sosten ip6tables -P FORWARD DROP
}

# Dice si el pid guardado en el archivo dado sigue vivo.
pid_vivo() {
    local pid
    [ -f "$1" ] || return 1
    pid="$(cat "$1")"
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

# Paso 3: lanza slirp4netns sobre el espacio. Si su pid sigue vivo, no hace nada.
# slirp4netns necesita el pid del sostén, que lee de ROOT_SOSTEN_PID (lo crea el paso 1).
# Su salida va a ROOT_SLIRP_LOG, con modo 0600. Slirp no recibe secretos.
asegurar_slirp() {
    if pid_vivo "$ROOT_SLIRP_PID"; then
        return 0
    fi
    # touch no borra lo que haya; chmod fija el modo aunque el archivo ya existiera.
    touch "$ROOT_SLIRP_LOG"
    chmod 0600 "$ROOT_SLIRP_LOG"
    slirp4netns --configure --disable-host-loopback --mtu=65520 \
        --userns-path="/proc/$(cat "$ROOT_SOSTEN_PID")/ns/user" \
        "$(cat "$ROOT_SOSTEN_PID")" tap0 </dev/null >>"$ROOT_SLIRP_LOG" 2>&1 &
    echo "$!" >"$ROOT_SLIRP_PID"
}

# Añade una regla de OUTPUT dentro del espacio solo si falta. Lo que sigue son los argumentos de la regla.
regla() {
    if en_sosten iptables -C OUTPUT "$@" 2>/dev/null; then
        return 0
    fi
    en_sosten iptables -A OUTPUT "$@"
}

# Paso 4: primero acepta el DNS de slirp, luego corta los rangos privados y especiales.
reglas_de_salida() {
    regla -d "$DNS_SLIRP" -p udp --dport 53 -j ACCEPT
    regla -d "$DNS_SLIRP" -p tcp --dport 53 -j ACCEPT
    local rango
    for rango in "${RANGOS_CORTADOS[@]}"; do
        regla ! -o lo -d "$rango" -j DROP
    done
}

# Paso 4b: revisa que IPv6 no quede activo dentro del espacio: sin ruta por defecto ni dirección global.
# Si algo imprime, avisa por stderr y sale con 1.
revisar_ipv6() {
    local ruta direccion
    ruta="$(en_sosten ip -6 route show default)"
    direccion="$(en_sosten ip -6 addr show scope global)"
    if [ -n "$ruta" ] || [ -n "$direccion" ]; then
        echo "mal red-builder: IPv6 sigue activo dentro de $ROOT_RED_BUILDER" >&2
        exit 1
    fi
}

# Paso 5: comprueba que api.anthropic.com resuelve dentro del espacio.
comprobar_red() {
    # slirp4netns tarda un poco en configurar la red: se reintenta antes de rendirse.
    local intento
    for ((intento = 1; intento <= ROOT_RED_INTENTOS; intento++)); do
        if ip netns exec "$ROOT_RED_BUILDER" getent hosts api.anthropic.com >/dev/null 2>&1; then
            echo "red lista"
            return 0
        fi
        [ "$intento" -lt "$ROOT_RED_INTENTOS" ] && sleep 1
    done
    echo "mal red-builder: api.anthropic.com no resuelve dentro de $ROOT_RED_BUILDER" >&2
    exit 1
}

# Cuerpo principal. Corre solo si el script se ejecuta directo.
principal() {
    revisar_herramientas
    preparar_pidfiles
    asegurar_espacio
    asegurar_tun
    cerrar_ipv6
    asegurar_slirp
    reglas_de_salida
    revisar_ipv6
    comprobar_red
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    principal "$@"
fi
