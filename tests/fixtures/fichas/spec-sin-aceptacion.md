# Memora

Memora es una caché que habla RESP2.

## Ficha

```toml
[producto]
usuarios = ["operador que corre memora como caché"]
problema = "Las aplicaciones necesitan una caché con protocolo Redis."
fuera_de_alcance = ["clúster", "TLS"]
[[producto.requisito]]
id = "R1"
titulo = "Protocolo RESP2"
peso = 3
aceptacion = "un cliente redis-py 5.x completa SET/GET de 10 000 claves sin error"
[[producto.requisito]]
id = "R2"
titulo = "Comando PING"
peso = 1

[tecnica]
lenguaje = "rust"
version_minima = "1.85"
dependencias_permitidas = ["tokio", "bytes"]
plataformas = ["linux-x86_64"]
[[tecnica.limite]]
id = "T1"
metrica = "ops_s"
minimo = 5000.0
unidad = "operaciones por segundo"

[arquitectura]
capas = [["protocolo"], ["comandos"], ["almacen"]]
persistencia = "AOF en disco con fsync cada segundo"
concurrencia = "un hilo por conexión y un lock por fragmento del almacén"
[[arquitectura.interfaz]]
id = "A1"
modulo = "almacen"
publica = ["get", "set", "del", "expire"]

[diseno]
errores = "todo error de red responde -ERR con su mensaje"
logs = "a stderr, un evento por línea, nivel configurable"
configuracion = "archivo memora.conf; cada directiva documentada en memora.conf.example"
[[diseno.regla]]
id = "D1"
texto = "ningún unwrap() en código que procesa entrada de la red"
item = "diseno.robustez"
```
