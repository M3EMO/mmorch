# Memora

Memora es una caché que habla RESP2. La prosa explica el producto.
La ficha fija los datos que Root mide.

## Ficha

```toml
[producto]
usuarios = ["operador que corre memora como caché"]
problema = "Las aplicaciones necesitan una caché con protocolo Redis sin depender de Redis."
fuera_de_alcance = ["clúster", "TLS"]
[[producto.requisito]]
id = "R1"
titulo = "Protocolo RESP2"
peso = 3
aceptacion = "un cliente redis-py 5.x completa SET/GET de 10 000 claves sin error"
pruebas = ["test_set_get"]
[[producto.requisito]]
id = "R2"
titulo = "Comando PING"
peso = 1
aceptacion = "`cargo test --test resp` termina con código 0 y la salida es \"PONG\""

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
errores = "todo error de red responde -ERR con su mensaje y no corta otras conexiones"
logs = "a stderr, un evento por línea, nivel configurable"
configuracion = "archivo memora.conf; cada directiva documentada en memora.conf.example"
[[diseno.regla]]
id = "D1"
texto = "ningún unwrap() en código que procesa entrada de la red"
item = "diseno.robustez"
```

Fin del spec.
