NOMBRE_PAQUETE = ""
SHA_PAQUETE = ""

# Guion del juez para el notebook de Colab. El orquestador corre cada celda_<n> en orden.
# Este módulo se importa sin google.colab. Las importaciones de Colab van dentro de las celdas.
# Las funciones de abajo reciben rutas por parámetro. Así se pueden testear.

import hashlib
import json
import math
import os
import shutil
import subprocess
import tarfile
import time

D = "/content/root-paquete"
PAQUETE_LOCAL = "/content/paquete.tar.gz"
CARPETA_DRIVE = "/content/drive/MyDrive/root-colab"
ARCHIVO_GASTO = "gasto.jsonl"
MANIFIESTO_VUELTA = "manifiesto-vuelta.json"
UMBRAL_PARADA = 0.85
TIPOS_DE_GASTO = ("llamada", "reserva")
PREFIJO_EVENTOS = "eventos.jsonl."

# La clave vive solo aquí, entre la celda 2 y la celda 3. Ningún print la muestra.
_CLAVE = None


def sha256_de(ruta):
    """Devuelve el SHA-256 de un archivo, leído por bloques."""
    h = hashlib.sha256()
    with open(ruta, "rb") as f:
        for bloque in iter(lambda: f.read(1 << 20), b""):
            h.update(bloque)
    return h.hexdigest()


def esperar_archivo(ruta, plazo_s=600, intervalo_s=5, dormir=time.sleep, ahora=time.monotonic):
    """Espera a que exista `ruta`. Falla si pasa `plazo_s` segundos."""
    inicio = ahora()
    while not os.path.exists(ruta):
        if ahora() - inicio >= plazo_s:
            raise FileNotFoundError(f"no apareció {ruta} tras {plazo_s} s")
        dormir(intervalo_s)


def copiar_y_verificar(origen, destino, sha_esperado):
    """Copia `origen` a `destino`. Si el SHA-256 no coincide, borra la copia y falla."""
    shutil.copyfile(origen, destino)
    obtenido = sha256_de(destino)
    if obtenido != sha_esperado:
        os.remove(destino)
        raise ValueError(f"SHA-256 no coincide: esperado {sha_esperado}, obtenido {obtenido}")


def abrir_paquete(tar, d):
    """Extrae en `d` un paquete ya verificado."""
    os.makedirs(d, exist_ok=True)
    with tarfile.open(tar, "r:gz") as paquete:
        paquete.extractall(d, filter="data")


def _leer_json(ruta):
    with open(ruta, encoding="utf-8") as f:
        return json.load(f)


def _escribir_json(ruta, valor):
    # Con sort_keys, el manifiesto sale igual que el del Rust.
    with open(ruta, "w", encoding="utf-8") as f:
        json.dump(valor, f, sort_keys=True)


def leer_filas(ruta):
    """Devuelve las filas de un calibracion.json. Lista vacía si el archivo no existe."""
    if not os.path.isfile(ruta):
        return []
    return _leer_json(ruta)["filas"]


def _lineas_de(ruta):
    if not os.path.isfile(ruta):
        return []
    with open(ruta, encoding="utf-8") as f:
        return f.readlines()


def _coincide(fila, trabajo):
    return all(fila.get(c) == trabajo.get(c) for c in ("tarea", "dimension", "n", "semilla"))


def fila_de_trabajo(trabajo, filas, ts_de_ida):
    """Devuelve la última fila nueva del trabajo. Una fila del paquete de ida no cuenta."""
    nuevas = [f for f in filas if _coincide(f, trabajo) and f.get("ts") not in ts_de_ida]
    return nuevas[-1] if nuevas else None


def ya_calibrado(trabajo, filas_runtime, ts_de_ida):
    """Dice si el trabajo ya tiene su fila nueva. Sirve para retomar una sesión cortada."""
    return fila_de_trabajo(trabajo, filas_runtime, ts_de_ida) is not None


def regla_de_parada(filas_de_etapa, umbral=UMBRAL_PARADA):
    """Dice True si la sensibilidad ponderada por defectos queda bajo el umbral."""
    usables = [
        f for f in filas_de_etapa if f.get("sensibilidad") is not None and f.get("defectos", 0) > 0
    ]
    total = sum(f["defectos"] for f in usables)
    if total == 0:
        return False
    ponderada = sum(f["sensibilidad"] * f["defectos"] for f in usables) / total
    return ponderada < umbral


def etapas_del_manifiesto(manifiesto):
    """Agrupa los trabajos por dimensión consecutiva. Cada etapa guarda (índice, trabajo)."""
    etapas = []
    for indice, trabajo in enumerate(manifiesto["trabajos"]):
        if etapas and etapas[-1]["dimension"] == trabajo["dimension"]:
            etapas[-1]["trabajos"].append((indice, trabajo))
        else:
            etapas.append({"dimension": trabajo["dimension"], "trabajos": [(indice, trabajo)]})
    return etapas


def lanzar_con_clave(args, entorno, clave_deepseek, cwd, log):
    """Corre `args` con la clave por una tubería. Devuelve el código de salida.

    `args` es el argv completo. La clave no va a argumentos, a archivos ni a impresiones.
    """
    limpio = {k: v for k, v in entorno.items() if not k.endswith(("_KEY", "_TOKEN"))}
    lector, escritor = os.pipe()
    try:
        limpio["ROOT_CLAVES_FD"] = str(lector)
        with open(log, "w", encoding="utf-8") as salida:
            proceso = subprocess.Popen(
                args,
                cwd=cwd,
                env=limpio,
                stdout=salida,
                stderr=subprocess.STDOUT,
                pass_fds=(lector,),
            )
    except BaseException:
        os.close(lector)
        os.close(escritor)
        raise
    os.close(lector)
    try:
        # La clave es corta y el hijo lee en paralelo. Cerrar la escritura da fin de lectura.
        with os.fdopen(escritor, "w", encoding="utf-8") as tuberia:
            tuberia.write(f"DEEPSEEK_API_KEY={clave_deepseek}\n")
    except BrokenPipeError:
        pass
    return proceso.wait()


def _lineas_de_eventos(d):
    """Lee los eventos rotados en orden y después el archivo actual."""
    rotados = sorted(
        (int(n[len(PREFIJO_EVENTOS):]), n)
        for n in os.listdir(d)
        if n.startswith(PREFIJO_EVENTOS) and n[len(PREFIJO_EVENTOS):].isdigit()
    )
    nombres = [n for _, n in rotados]
    if os.path.isfile(os.path.join(d, "eventos.jsonl")):
        nombres.append("eventos.jsonl")
    lineas = []
    for nombre in nombres:
        with open(os.path.join(d, nombre), encoding="utf-8") as entrada:
            lineas.extend(entrada)
    return lineas


def _es_gasto(linea):
    # Una línea que no es JSON pasa tal cual. La laptop la rechaza con su número.
    try:
        evento = json.loads(linea)
    except ValueError:
        return True
    return not isinstance(evento, dict) or evento.get("tipo") in TIPOS_DE_GASTO


def _sha_de_partes(d, partes):
    archivos = {}
    for parte in partes:
        ruta = os.path.join(d, parte)
        if os.path.isdir(ruta):
            for raiz, _, nombres in os.walk(ruta):
                for nombre in nombres:
                    completa = os.path.join(raiz, nombre)
                    relativa = os.path.relpath(completa, d).replace(os.sep, "/")
                    archivos[relativa] = sha256_de(completa)
        else:
            archivos[parte] = sha256_de(ruta)
    return archivos


def gasto_de(lineas_antes, lineas_despues):
    """Suma `cost_usd` de los eventos `llamada` que aparecen entre dos lecturas."""
    total = 0.0
    for linea in lineas_despues[len(lineas_antes):]:
        try:
            evento = json.loads(linea)
        except ValueError:
            continue
        if isinstance(evento, dict) and evento.get("tipo") == "llamada":
            total += float(evento.get("cost_usd") or 0)
    return total


def gastado_del_paquete(d):
    """Suma `cost_usd` de las llamadas de todos los eventos de `d`: rotados y actual.

    Es la misma regla que `gasto_de`, así cuenta también una sesión anterior retomada.
    """
    return gasto_de([], _lineas_de_eventos(d))


def resto_del_tope(tope_paquete, gastado):
    """Lo que queda del tope del paquete, en USD, redondeado hacia abajo a centavos. Nunca negativo."""
    return max(0.0, math.floor(round((tope_paquete - gastado) * 100, 6)) / 100)


def con_tope(args, resto):
    """Copia de `args` con el valor de `--tope-usd` cambiado por `resto`. Sin `--tope-usd`, no cambia."""
    if "--tope-usd" not in args:
        return list(args)
    nuevos = list(args)
    nuevos[nuevos.index("--tope-usd") + 1] = f"{resto:.2f}"
    return nuevos


def decidir_tope(trabajo, tope_paquete, gastado):
    """Devuelve (trabajo a correr o None, resto). None: el tope del paquete se agotó y no corre.

    Sin tope en el manifiesto, o sin `--tope-usd` en el trabajo (oráculo), el trabajo sigue igual.
    """
    if tope_paquete is None or "--tope-usd" not in trabajo["args"]:
        return trabajo, None
    resto = resto_del_tope(tope_paquete, gastado)
    if resto < 0.01:
        return None, resto
    return dict(trabajo, args=con_tope(trabajo["args"], resto)), resto


def linea_de_resultado(tarea, dimension, sens, esp, gasto):
    return (
        f"tarea={tarea} dimension={dimension} sensibilidad={sens} "
        f"especificidad={esp} gasto_usd={gasto:.4f}"
    )


def armar_vuelta(d, destino):
    """Arma el paquete de vuelta en `destino`. Devuelve su SHA-256."""
    script = os.path.join(d, "tools", "colab", "verificar-versiones.sh")
    # Si el script falta, bash escribe el error aquí. La vuelta sale igual.
    with open(os.path.join(d, "verificacion.txt"), "w", encoding="utf-8") as verificacion:
        subprocess.run(
            ["bash", script, d], stdout=verificacion, stderr=subprocess.STDOUT, check=False
        )
    with open(os.path.join(d, ARCHIVO_GASTO), "w", encoding="utf-8") as gasto:
        for linea in _lineas_de_eventos(d):
            if linea.strip() and _es_gasto(linea):
                gasto.write(linea.rstrip("\n") + "\n")
    partes = ["verificacion.txt", ARCHIVO_GASTO]
    if os.path.isfile(os.path.join(d, "calibracion.json")):
        partes.append("calibracion.json")
    if os.path.isdir(os.path.join(d, "calibracion")):
        partes.append("calibracion")
    archivos = _sha_de_partes(d, partes)
    _escribir_json(os.path.join(d, MANIFIESTO_VUELTA), {"v": 1, "archivos": archivos})
    partes.append(MANIFIESTO_VUELTA)
    subprocess.run(["tar", "-czf", destino, "-C", d] + partes, check=True)
    return sha256_de(destino)


def _correr_trabajo(indice, trabajo, clave):
    args = [os.path.join(D, "r00t")] + trabajo["args"]
    entorno = dict(os.environ)
    entorno.update(trabajo["entorno"])
    entorno["ROOT_HOME"] = D
    log = os.path.join(D, f"trabajo-{indice}.log")
    return lanzar_con_clave(args, entorno, clave, D, log)


def _correr_y_medir(indice, trabajo, clave):
    """Corre un trabajo. Devuelve (código, gasto). Una excepción cuenta como fallo."""
    ruta_eventos = os.path.join(D, "eventos.jsonl")
    antes = _lineas_de(ruta_eventos)
    try:
        codigo = _correr_trabajo(indice, trabajo, clave)
    except Exception as error:
        codigo = f"excepción {type(error).__name__}"
    return codigo, gasto_de(antes, _lineas_de(ruta_eventos))


# CELDA 1: monta Drive, verifica el paquete y prepara el runtime.
def celda_1():
    from google.colab import drive

    drive.mount("/content/drive")
    origen = os.path.join(CARPETA_DRIVE, NOMBRE_PAQUETE)
    esperar_archivo(origen)
    copiar_y_verificar(origen, PAQUETE_LOCAL, SHA_PAQUETE)
    if shutil.which("bwrap") is None:
        subprocess.run(["apt-get", "update", "-qq"], check=True)
        subprocess.run(["apt-get", "install", "-y", "bubblewrap"], check=True)
    version = subprocess.run(["bwrap", "--version"], capture_output=True, text=True, check=False)
    print(version.stdout.strip())
    ruta_ida = os.path.join(D, "filas-de-ida.json")
    # Si ya existe, la sesión ya abrió el paquete. No se extrae de nuevo: borraría las filas nuevas.
    if os.path.exists(ruta_ida):
        print("el paquete ya está abierto: no se extrae de nuevo")
    else:
        abrir_paquete(PAQUETE_LOCAL, D)
        ida = [f.get("ts") for f in leer_filas(os.path.join(D, "calibracion.json"))]
        _escribir_json(ruta_ida, ida)
    prep = subprocess.run(
        ["bash", os.path.join(D, "tools", "colab", "preparar-runtime.sh"), D],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )
    print("\n".join(prep.stdout.splitlines()[-20:]))
    if prep.returncode != 0:
        raise RuntimeError(f"preparar-runtime.sh salió con código {prep.returncode}")


# CELDA 2: pide la clave a Colab y la guarda solo en _CLAVE.
def celda_2():
    global _CLAVE
    from google.colab import userdata

    _CLAVE = userdata.get("DEEPSEEK_API_KEY")
    print(f"clave presente: {bool(_CLAVE)}")


# CELDA 3: corre las etapas del manifiesto en orden, con regla de parada y retomar.
def celda_3():
    global _CLAVE
    try:
        if not _CLAVE:
            raise RuntimeError("falta la clave: corre la celda 2 primero")
        manifiesto = _leer_json(os.path.join(D, "manifiesto.json"))
        ruta_ida = os.path.join(D, "filas-de-ida.json")
        if not os.path.isfile(ruta_ida):
            raise RuntimeError("falta filas-de-ida.json: corre la celda 1 primero")
        ts_de_ida = _leer_json(ruta_ida)
        tope_paquete = manifiesto.get("tope_s2_usd")
        ruta_calibracion = os.path.join(D, "calibracion.json")
        filas_previas = {}
        fallos = []
        saltos = []
        for etapa in etapas_del_manifiesto(manifiesto):
            dimension = etapa["dimension"]
            if regla_de_parada(filas_previas.get(dimension, [])):
                print(f"salta la etapa de {dimension}: sensibilidad bajo el umbral")
                saltos.append(dimension)
                continue
            filas_etapa = []
            for indice, trabajo in etapa["trabajos"]:
                nombre = f"{trabajo['tarea']} {dimension} n={trabajo['n']} semilla={trabajo['semilla']}"
                gasto = 0.0
                if ya_calibrado(trabajo, leer_filas(ruta_calibracion), ts_de_ida):
                    print(f"ya calibrado, se salta: {nombre}")
                else:
                    a_correr, _ = decidir_tope(trabajo, tope_paquete, gastado_del_paquete(D))
                    if a_correr is None:
                        print(f"tope del paquete agotado: {nombre}")
                        saltos.append(nombre)
                    else:
                        codigo, gasto = _correr_y_medir(indice, a_correr, _CLAVE)
                        if codigo != 0:
                            fallos.append(nombre)
                            print(f"fallo: {nombre}: {codigo}")
                fila = fila_de_trabajo(trabajo, leer_filas(ruta_calibracion), ts_de_ida)
                if fila is None:
                    print(f"sin fila nueva: {nombre}")
                    continue
                filas_etapa.append(fila)
                print(
                    linea_de_resultado(
                        trabajo["tarea"],
                        dimension,
                        fila.get("sensibilidad"),
                        fila.get("especificidad"),
                        gasto,
                    )
                )
            filas_previas[dimension] = filas_etapa
        print("fallos: " + (", ".join(fallos) or "ninguno"))
        print("saltos: " + (", ".join(saltos) or "ninguno"))
    finally:
        _CLAVE = None


# CELDA 4: arma el paquete de vuelta y lo copia a Drive.
def celda_4():
    local = "/content/vuelta.tar.gz"
    sha = armar_vuelta(D, local)
    marca = time.strftime("%Y%m%d-%H%M%S", time.gmtime())
    destino = os.path.join(CARPETA_DRIVE, f"vuelta-{marca}.tar.gz")
    copiar_y_verificar(local, destino, sha)
    print(destino)
    print(os.path.getsize(destino))
    print(f"ROOT_SHA_VUELTA={sha}")
