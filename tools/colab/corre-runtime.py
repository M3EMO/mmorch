#!/usr/bin/env python3
"""Corre las tareas de un paquete de `corre` en Colab, una por una, con el r00t del paquete.

Se lanza aparte del exec, como lanzar.py. Usa solo la biblioteca estándar.
Uso: corre-runtime.py <carpeta_del_paquete>

La clave entra por DEEPSEEK_API_KEY (entorno) y se pasa solo al entorno del r00t hijo.
El token entra por archivo: su ruta sale de ROOT_TOKEN_ARCHIVO (defecto /content/root-token).
Se mueve a ROOT_CORRE_TOKEN (defecto /run/root-token), con modo 0600, y el original se borra.
El token de /run se borra al terminar. ROOT_TOKEN_CUPO ya no existe: si está en el entorno,
el guion sale con 2 sin correr nada.
Variables para testear, todas con defecto: ROOT_CORRE_R00T, ROOT_CORRE_HOME, ROOT_CORRE_TOKEN,
ROOT_CORRE_PLAZO_S, ROOT_TOKEN_ARCHIVO, ROOT_CORRE_HOME_USUARIO.
El r00t hijo corre con HOME=/srv/root-usuario (ROOT_CORRE_HOME_USUARIO lo cambia para tests).
El r00t hijo recibe ROOT_PROC_BUILDER=/proc: el procfs del runtime, que el builder ve en solo lectura.
"""

import hashlib
import io
import json
import os
import signal
import subprocess
import sys
import tarfile
import time
from datetime import datetime, timezone

R00T_DEFECTO = "/usr/local/bin/r00t"
HOME_DEFECTO = "/root/root-home"
# HOME del r00t hijo. Es una carpeta que el builder puede buscar pero no listar (preparar-corre.sh).
HOME_USUARIO_DEFECTO = "/srv/root-usuario"
TOKEN_DEFECTO = "/run/root-token"
TOKEN_ARCHIVO_DEFECTO = "/content/root-token"
PLAZO_DEFECTO = "7200"
# Código que se anota cuando el plazo corta la vuelta. Es el de `timeout`.
CODIGO_PLAZO = 124
OCULTO = b"[oculto]"


class ErrorCorrida(Exception):
    """Error de la corrida. Su mensaje nunca lleva claves ni tokens."""


def ahora():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def escribir_atomico(ruta, datos):
    """Escribe en un temporal al lado y lo renombra. Nadie lee un archivo a medias."""
    temporal = ruta + ".tmp"
    with open(temporal, "wb") as f:
        f.write(datos)
    os.replace(temporal, ruta)


def ocultar(datos, secretos):
    """Cambia cada secreto por [oculto]."""
    for secreto in secretos:
        datos = datos.replace(secreto, OCULTO)
    return datos


def leer_bytes(ruta):
    try:
        with open(ruta, "rb") as f:
            return f.read()
    except FileNotFoundError:
        return b""


def leer_filas(ruta):
    return leer_bytes(ruta).splitlines(keepends=True)


def transcripts_de(home):
    carpeta = os.path.join(home, "transcripts")
    if os.path.isdir(carpeta):
        return set(os.listdir(carpeta))
    return set()


def hecho_de(carpeta, tarea):
    return os.path.join(carpeta, "hechos", tarea + ".json")


def escribir_estado(carpeta, tareas, corre=None, pid=None, final=None):
    """Escribe estado.json. Las tareas con hechos/<t>.json están hechas; las demás faltan."""
    hechas = [t for t in tareas if os.path.exists(hecho_de(carpeta, t))]
    dato = {
        "hechas": hechas,
        "faltan": [t for t in tareas if t not in hechas],
        "corre": corre,
        "pid": pid,
    }
    if final:
        dato.update(final)
    texto = json.dumps(dato, ensure_ascii=False, indent=2) + "\n"
    escribir_atomico(os.path.join(carpeta, "estado.json"), texto.encode())


def leer_token(ruta):
    """Lee el token del archivo tal cual. Falla si no existe o está vacío. El mensaje nunca lleva
    el contenido."""
    try:
        with open(ruta, "rb") as f:
            datos = f.read()
    except OSError:
        raise ErrorCorrida(f"no se pudo leer el token en {ruta}")
    if not datos.strip():
        raise ErrorCorrida(f"el archivo del token está vacío: {ruta}")
    return datos


def escribir_token(ruta, datos):
    """Crea el archivo del token con modo 0600. Si ya existe, lo reemplaza."""
    try:
        os.unlink(ruta)
    except FileNotFoundError:
        pass
    fd = os.open(ruta, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as f:
        f.write(datos)


def quitar_token(ruta):
    try:
        os.unlink(ruta)
    except FileNotFoundError:
        pass


def leer_manifiesto(carpeta):
    ruta = os.path.join(carpeta, "manifiesto.json")
    try:
        with open(ruta, "rb") as f:
            manifiesto = json.load(f)
    except (OSError, ValueError):
        raise ErrorCorrida(f"no se pudo leer {ruta}")
    tareas = manifiesto.get("tareas")
    config = manifiesto.get("config")
    imagen_id = manifiesto.get("imagen_id")
    if not isinstance(tareas, list) or not all(isinstance(t, str) for t in tareas):
        raise ErrorCorrida("manifiesto.json sin lista de tareas")
    if not isinstance(config, str) or not config:
        raise ErrorCorrida("manifiesto.json sin config")
    if not isinstance(imagen_id, str) or not imagen_id:
        raise ErrorCorrida("manifiesto.json sin imagen_id")
    for tarea in tareas:
        if not tarea or "/" in tarea or tarea in (".", ".."):
            raise ErrorCorrida(f"nombre de tarea inválido: {tarea!r}")
    return tareas, config, imagen_id


def leer_plazo():
    texto = os.environ.get("ROOT_CORRE_PLAZO_S", PLAZO_DEFECTO)
    try:
        plazo = float(texto)
    except ValueError:
        raise ErrorCorrida("ROOT_CORRE_PLAZO_S no es un número")
    if plazo <= 0:
        raise ErrorCorrida("ROOT_CORRE_PLAZO_S debe ser mayor que 0")
    return plazo


def agregar(tar, nombre, datos):
    info = tarfile.TarInfo(nombre)
    info.size = len(datos)
    info.mtime = int(time.time())
    tar.addfile(info, io.BytesIO(datos))


def armar_vuelta(meta, runs, eventos, transcripts_dir, nuevos, salida, secretos):
    """Arma el tar.gz de la vuelta en memoria. Todo pasa por ocultar()."""
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as tar:
        texto_meta = json.dumps(meta, ensure_ascii=False, indent=2) + "\n"
        agregar(tar, "vuelta.json", texto_meta.encode())
        agregar(tar, "runs.jsonl", ocultar(b"".join(runs), secretos))
        agregar(tar, "eventos.jsonl", ocultar(b"".join(eventos), secretos))
        for nombre in nuevos:
            base = os.path.join(transcripts_dir, nombre)
            for raiz, _, archivos in os.walk(base):
                for archivo in sorted(archivos):
                    ruta = os.path.join(raiz, archivo)
                    rel = os.path.relpath(ruta, transcripts_dir)
                    agregar(
                        tar,
                        "transcripts/" + rel,
                        ocultar(leer_bytes(ruta), secretos),
                    )
        agregar(tar, "salida.txt", ocultar(salida, secretos))
    return buffer.getvalue()


def matar_grupo(proceso):
    """Mata el grupo de procesos del r00t. Se creó con start_new_session, así que pgid == pid."""
    try:
        os.killpg(proceso.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


class Corrida:
    """Corre las tareas pendientes en orden. Guarda el proceso vivo para limpiarlo si hay error."""

    def __init__(self, carpeta, tareas, config, entorno):
        self.carpeta = carpeta
        self.tareas = tareas
        self.config = config
        self.imagen_id = entorno["imagen_id"]
        self.r00t = entorno["r00t"]
        self.home = entorno["home"]
        self.home_usuario = entorno["home_usuario"]
        self.token = entorno["token"]
        self.plazo = entorno["plazo"]
        self.clave = entorno["clave"]
        self.cupo = entorno["cupo"]
        self.secretos = [s for s in (self.clave.encode(), self.cupo) if s]
        self.proceso = None

    def correr_todas(self):
        escribir_estado(self.carpeta, self.tareas)
        for tarea in self.tareas:
            if os.path.exists(hecho_de(self.carpeta, tarea)):
                print(f"tarea {tarea} ya hecha", flush=True)
                continue
            self.correr_una(tarea)

    def entorno_del_hijo(self):
        entorno = dict(os.environ)
        entorno.update(
            {
                "ROOT_HOME": self.home,
                "HOME": self.home_usuario,
                "ROOT_CONTENEDOR_MODO": "bwrap",
                "ROOT_RED_BUILDER": "root-builder",
                "ROOT_USUARIO_BUILDER": "builder",
                "ROOT_PROC_BUILDER": "/proc",
                "ROOT_TOKEN_BUILDER": self.token,
                "DEEPSEEK_API_KEY": self.clave,
                "ROOT_BWRAP_ENTORNO": os.path.join(self.carpeta, "entorno.env"),
                "ROOT_BWRAP_IMAGEN_ID": self.imagen_id,
            }
        )
        return entorno

    def correr_una(self, tarea):
        inicio = ahora()
        runs_ruta = os.path.join(self.home, "runs.jsonl")
        eventos_ruta = os.path.join(self.home, "eventos.jsonl")
        runs_antes = len(leer_filas(runs_ruta))
        eventos_antes = len(leer_filas(eventos_ruta))
        transcripts_antes = transcripts_de(self.home)

        cmd = [self.r00t, "corre", "--tarea", tarea, "--config", self.config]
        self.proceso = subprocess.Popen(
            cmd,
            cwd=self.carpeta,
            env=self.entorno_del_hijo(),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        escribir_estado(self.carpeta, self.tareas, corre=tarea, pid=self.proceso.pid)

        cortada = None
        try:
            salida, _ = self.proceso.communicate(timeout=self.plazo)
            codigo = self.proceso.returncode
        except subprocess.TimeoutExpired:
            matar_grupo(self.proceso)
            salida, _ = self.proceso.communicate()
            codigo = CODIGO_PLAZO
            cortada = "plazo"
        self.proceso = None
        fin = ahora()

        print(f"tarea {tarea} código {codigo}", flush=True)

        runs = leer_filas(runs_ruta)[runs_antes:]
        eventos = leer_filas(eventos_ruta)[eventos_antes:]
        nuevos = sorted(transcripts_de(self.home) - transcripts_antes)
        meta = {
            "tarea": tarea,
            "codigo": codigo,
            "cortada": cortada,
            "inicio": inicio,
            "fin": fin,
        }
        vuelta = armar_vuelta(
            meta,
            runs,
            eventos,
            os.path.join(self.home, "transcripts"),
            nuevos,
            salida or b"",
            self.secretos,
        )
        escribir_atomico(os.path.join(self.carpeta, f"vuelta-{tarea}.tar.gz"), vuelta)

        os.makedirs(os.path.join(self.carpeta, "hechos"), exist_ok=True)
        hecho = {
            "codigo": codigo,
            "sha": hashlib.sha256(vuelta).hexdigest(),
            "inicio": inicio,
            "fin": fin,
        }
        texto_hecho = json.dumps(hecho, indent=2) + "\n"
        escribir_atomico(hecho_de(self.carpeta, tarea), texto_hecho.encode())
        escribir_estado(self.carpeta, self.tareas)


def _interrumpir(signum, frame):
    raise SystemExit(128 + signum)


def principal(argv):
    if len(argv) != 2 or not argv[1]:
        print("mal corre-runtime.py: falta la carpeta del paquete", file=sys.stderr)
        return 2

    carpeta = os.path.abspath(argv[1])
    # Se sacan del entorno antes de imprimir nada: el hijo no debe heredarlas, y el valor del
    # token viejo no debe salir en ninguna salida.
    tenia_cupo_viejo = os.environ.pop("ROOT_TOKEN_CUPO", None) is not None
    clave = os.environ.pop("DEEPSEEK_API_KEY", "")
    if tenia_cupo_viejo:
        print(
            "mal corre-runtime.py: ROOT_TOKEN_CUPO ya no se usa; el token va en el archivo "
            "de ROOT_TOKEN_ARCHIVO",
            file=sys.stderr,
        )
        return 2
    if not clave:
        print("mal corre-runtime.py: falta DEEPSEEK_API_KEY", file=sys.stderr)
        return 2

    archivo_token = os.environ.get("ROOT_TOKEN_ARCHIVO", TOKEN_ARCHIVO_DEFECTO)
    token = os.environ.get("ROOT_CORRE_TOKEN", TOKEN_DEFECTO)
    if os.path.abspath(archivo_token) == os.path.abspath(token):
        print(
            "mal corre-runtime.py: ROOT_TOKEN_ARCHIVO y ROOT_CORRE_TOKEN no pueden ser la misma ruta",
            file=sys.stderr,
        )
        return 2
    try:
        crudo = leer_token(archivo_token)
    except ErrorCorrida as e:
        print("mal corre-runtime.py: " + str(e), file=sys.stderr)
        return 2
    # El valor que se oculta va sin espacios de sobra; el archivo se mueve tal cual.
    cupo = crudo.strip()
    secretos = [s for s in (clave.encode(), cupo) if s]
    tareas = []
    corrida = None
    error = None
    signal.signal(signal.SIGTERM, _interrumpir)
    signal.signal(signal.SIGINT, _interrumpir)
    try:
        tareas, config, imagen_id = leer_manifiesto(carpeta)
        plazo = leer_plazo()
        corrida = Corrida(
            carpeta,
            tareas,
            config,
            {
                "imagen_id": imagen_id,
                "r00t": os.environ.get("ROOT_CORRE_R00T", R00T_DEFECTO),
                "home": os.environ.get("ROOT_CORRE_HOME", HOME_DEFECTO),
                "home_usuario": os.environ.get(
                    "ROOT_CORRE_HOME_USUARIO", HOME_USUARIO_DEFECTO
                ),
                "token": token,
                "plazo": plazo,
                "clave": clave,
                "cupo": cupo,
            },
        )
        escribir_token(token, crudo)
        quitar_token(archivo_token)
        corrida.correr_todas()
    except SystemExit:
        error = "interrumpido"
        raise
    except Exception as e:
        # El mensaje puede traer una ruta, pero nunca un valor de los secretos.
        error = ocultar(str(e).encode(), secretos).decode(errors="replace")
        print("mal corre-runtime.py: " + error, file=sys.stderr)
    finally:
        if corrida is not None and corrida.proceso is not None:
            matar_grupo(corrida.proceso)
        quitar_token(token)
        final = {"error": error} if error else {"fin": True}
        escribir_estado(carpeta, tareas, final=final)
    return 1 if error else 0


if __name__ == "__main__":
    sys.exit(principal(sys.argv))
