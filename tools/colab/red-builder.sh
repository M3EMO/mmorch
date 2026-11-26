#!/usr/bin/env bash
# Prepara la red del builder de Colab. Corre como root, antes de la primera corrida.
# Crea el espacio de red, lanza slirp4netns y pone reglas de salida. Es idempotente.
# Al final comprueba que api.anthropic.com resuelve dentro del espacio.
# Uso: red-builder.sh
set -euo pipefail

ROOT_RED_BUILDER="${ROOT_RED_BUILDER:-root-builder}"
ROOT_SLIRP_PID="${ROOT_SLIRP_PID:-/run/root-builder-slirp.pid}"
ROOT_SOSTEN_PID="${ROOT_SOSTEN_PID:-/run/root-builder-sosten.pid}"
ROOT_RED_INTENTOS="${ROOT_RED_INTENTOS:-10}"
export ROOT_RED_BUILDER ROOT_SLIRP_PID ROOT_SOSTEN_PID ROOT_RED_INTENTOS

# DNS de slirp4netns. Es lo único que se acepta antes de los cortes.
DNS_SLIRP="10.0.2.3"

# Rangos que se cortan para la salida. Son 6, no 7: el brief nombra 6 rangos.
RANGOS_CORTADOS=(10.0.0.0/8 172.16.0.0/12 192.168.0.0/16 100.64.0.0/10 169.254.0.0/16 127.0.0.0/8)

# Crea las carpetas de los pidfiles si no existen.
preparar_pidfiles() {
    mkdir -p "$(dirname "$ROOT_SLIRP_PID")" "$(dirname "$ROOT_SOSTEN_PID")"
}

# Paso 1: crea el espacio de red solo si no existe.
asegurar_espacio() {
    local listado
    listado="$(ip netns list | cut -d' ' -f1)"
    if grep -qx "$ROOT_RED_BUILDER" <<<"$listado"; then
        return 0
    fi
    ip netns add "$ROOT_RED_BUILDER"
}

# Dice si el pid guardado en el archivo dado sigue vivo.
pid_vivo() {
    local pid
    [ -f "$1" ] || return 1
    pid="$(cat "$1")"
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

# Paso 2: lanza slirp4netns sobre el espacio. Si su pid sigue vivo, no hace nada.
# Reusa el proceso de sostén si sigue vivo. slirp4netns necesita un pid dentro del espacio.
asegurar_slirp() {
    if pid_vivo "$ROOT_SLIRP_PID"; then
        return 0
    fi
    if ! pid_vivo "$ROOT_SOSTEN_PID"; then
        ip netns exec "$ROOT_RED_BUILDER" sleep infinity </dev/null >/dev/null 2>&1 &
        echo "$!" >"$ROOT_SOSTEN_PID"
    fi
    slirp4netns --configure --disable-host-loopback --mtu=65520 \
        "$(cat "$ROOT_SOSTEN_PID")" tap0 </dev/null >/dev/null 2>&1 &
    echo "$!" >"$ROOT_SLIRP_PID"
}

# Añade una regla de OUTPUT dentro del espacio solo si falta. Lo que sigue son los argumentos de la regla.
regla() {
    if ip netns exec "$ROOT_RED_BUILDER" iptables -C OUTPUT "$@" 2>/dev/null; then
        return 0
    fi
    ip netns exec "$ROOT_RED_BUILDER" iptables -A OUTPUT "$@"
}

# Paso 3: primero acepta el DNS de slirp, luego corta los rangos privados y especiales.
reglas_de_salida() {
    regla -d "$DNS_SLIRP" -p udp --dport 53 -j ACCEPT
    regla -d "$DNS_SLIRP" -p tcp --dport 53 -j ACCEPT
    local rango
    for rango in "${RANGOS_CORTADOS[@]}"; do
        regla ! -o lo -d "$rango" -j DROP
    done
}

# Paso 4: comprueba que api.anthropic.com resuelve dentro del espacio.
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
    preparar_pidfiles
    asegurar_espacio
    asegurar_slirp
    reglas_de_salida
    comprobar_red
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    principal "$@"
fi
