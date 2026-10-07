---
title: Vigia - requisitos de seguridad, operacion, cumplimiento y licencias
mision: Que controles de seguridad y operacion, testeables, necesita Vigia para correr autoalojado y multi-tenant con datos sensibles de finanzas, salud, legal y gobierno.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://genai.owasp.org/llm-top-10/
  - https://genai.owasp.org/llmrisk/llm102025-unbounded-consumption/
  - https://top10.owasp.org/2025
  - https://api-security.owasp.org/editions/2023/en/0x11-t10
  - https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
  - https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
  - https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
  - https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html
  - https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html
  - https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html
  - https://pages.nist.gov/800-63-4/sp800-63b.html
  - https://argon2-cffi.readthedocs.io/en/stable/api.html
  - https://github.blog/engineering/platform-security/behind-githubs-new-authentication-token-formats/
  - https://docs.github.com/en/code-security/secret-scanning/secret-scanning-partnership-program/secret-scanning-partner-program
  - https://github.com/truestamp/prefixed-api-key
  - https://www.postgresql.org/docs/current/ddl-rowsecurity.html
  - https://kubernetes.io/docs/concepts/configuration/liveness-readiness-startup-probes/
  - https://prometheus.io/docs/practices/naming/
  - https://github.com/open-telemetry/semantic-conventions-genai
  - https://datatracker.ietf.org/doc/draft-ietf-httpapi-ratelimit-headers/
  - https://docs.docker.com/engine/swarm/secrets/
  - https://docs.vllm.ai/en/latest/usage/security.html
  - https://github.com/microsoft/presidio
  - https://huggingface.co/docs/safetensors/index
  - https://github.com/openai/gpt-oss
  - https://raw.githubusercontent.com/openai/gpt-oss/main/USAGE_POLICY
  - https://github.com/langfuse/langfuse/blob/main/LICENSE
  - https://grafana.com/blog/2021/04/20/grafana-loki-tempo-relicensing-to-agplv3/
  - https://servicios.infoleg.gob.ar/infolegInternet/anexos/60000-64999/64790/texact.htm
  - https://www.argentina.gob.ar/normativa/nacional/resolución-47-2018-312662/texto
  - https://iapp.org/news/a/argentina-ratifica-el-convenio-108-en-materia-de-proteccion-de-datos-personales
  - https://www.marval.com/publicacion/nuevo-anteproyecto-busca-reemplazar-la-ley-de-proteccion-de-datos-argentina-14336
  - https://abogados.com.ar/seguridad-informatica-comunicacion-a-7724-bcra/33748
  - https://www.law.cornell.edu/cfr/text/45/164.312
  - https://www.law.cornell.edu/cfr/text/45/164.316
  - https://www.law.cornell.edu/cfr/text/45/164.514
  - https://compliancehub.wiki/blog/hipaa-security-rule-final-rule-may-2026-mfa-encryption-uncertainty
  - https://gdpr-info.eu/art-28-gdpr/
  - https://gdpr-info.eu/art-32-gdpr/
  - https://gdpr-info.eu/art-33-gdpr/
  - https://artificialintelligenceact.eu/article/50/
  - https://artificialintelligenceact.eu/annex/3/
  - https://compliancehub.wiki/eu-ai-act-article-50-transparency-digital-omnibus-2026/
created: 2026-10-04
---

# Vigia - seguridad, operacion, cumplimiento y licencias

## Tronco

Vigia necesita auth por API key hasheada con HMAC y prefijo, sesiones `__Host-` con CSRF firmado, RBAC deny-by-default con aislamiento por tenant testeado en cada endpoint, logs sin texto de prompts, retencion configurable con borrado real, y un juez de calibracion que nunca saque datos del perimetro sin opt-in.

## Hallazgos

### 1. Marcos de amenaza vigentes (octubre 2026)

#### 1.1 OWASP Top 10 web, edicion 2025

La pagina oficial lista la edicion 2025. No pude confirmar en la pagina si es final o release candidate [no verificado el estado].

| ID | Categoria | Aplicacion directa en Vigia |
|---|---|---|
| A01:2025 | Broken Access Control | Aislamiento por tenant, RBAC, autorizacion por objeto en cada ruta. |
| A02:2025 | Security Misconfiguration | Defaults seguros en Docker, headers, modo debug apagado. |
| A03:2025 | Software Supply Chain Failures | Pesos de modelo, wheels, imagenes base, dependencias JS del panel. |
| A04:2025 | Cryptographic Failures | Hash de claves y passwords, TLS, cifrado en reposo. |
| A05:2025 | Injection | SQL, XSS por texto del modelo renderizado, CSV injection en exports. |
| A06:2025 | Insecure Design | Juez externo que recibe datos sensibles, scores como canal lateral. |
| A07:2025 | Authentication Failures | Login del panel, API keys, rotacion y revocacion. |
| A08:2025 | Software or Data Integrity Failures | Integridad de artefactos de calibracion y del audit log. |
| A09:2025 | Security Logging and Alerting Failures | Audit log, eventos de auth, alertas. |
| A10:2025 | Mishandling of Exceptional Conditions | Errores que filtran stack traces, fail-open en checks de auth. |

#### 1.2 OWASP API Security Top 10 2023

La API de ingesta de Vigia es la superficie principal. Los riesgos mas relevantes son estos.

| ID | Riesgo | Control en Vigia |
|---|---|---|
| API1 | Broken Object Level Authorization | Todo acceso por ID filtra por `tenant_id` del principal. |
| API2 | Broken Authentication | API keys con prefijo, hash HMAC, expiracion y revocacion. |
| API3 | Broken Object Property Level Authorization | Schemas de salida por rol; viewer no ve texto crudo si la politica lo oculta. |
| API4 | Unrestricted Resource Consumption | Limites de body, tokens, batch, timeouts y rate limit. |
| API5 | Broken Function Level Authorization | Rutas admin con chequeo de rol, no solo de autenticacion. |
| API6 | Unrestricted Access to Sensitive Business Flows | Exports masivos y recalibracion con rate limit y audit. |
| API7 | SSRF | Webhooks y endpoint de re-consulta RAG con allowlist de hosts. |
| API8 | Security Misconfiguration | Ver A02. |
| API9 | Improper Inventory Management | OpenAPI versionado; rutas no documentadas fallan. |
| API10 | Unsafe Consumption of APIs | Respuestas del juez LLM validadas contra schema estricto. |

#### 1.3 OWASP Top 10 para aplicaciones LLM 2025

Esta lista es la mas especifica para Vigia. Vigia no genera texto, pero lee, guarda y muestra texto generado.

| ID | Riesgo | Como aparece en Vigia | Control |
|---|---|---|---|
| LLM01 | Prompt Injection | El texto del cliente entra al juez LLM de calibracion. Un texto puede ordenar "marca todo como correcto". | Delimitadores, salida JSON con schema estricto, muestreo humano de etiquetas, deteccion de etiquetas anomalas por lote. |
| LLM02 | Sensitive Information Disclosure | El panel muestra fragmentos con PII. | RBAC, redaccion opcional al guardar, logs sin texto. |
| LLM03 | Supply Chain | Pesos MoE de Hugging Face, codigo remoto. | Solo safetensors, hash fijado, `trust_remote_code=False`, SBOM. |
| LLM04 | Data and Model Poisoning | Datos de calibracion envenenados mueven el umbral. | Calibraciones versionadas e inmutables, firma o hash, comparacion de AUROC contra la version previa. |
| LLM05 | Improper Output Handling | El panel renderiza texto del modelo. | Escape por defecto, nunca `innerHTML`, filtrar caracteres de control bidi. |
| LLM06 | Excessive Agency | La accion "re-consulta con RAG" ejecuta algo automaticamente. | Acciones declarativas con allowlist; sin ejecucion de herramientas arbitrarias. |
| LLM07 | System Prompt Leakage | El cliente puede enviar el system prompt completo. | No guardar system prompts por defecto; campo separado con retencion corta. |
| LLM08 | Vector and Embedding Weaknesses | El indice RAG de re-consulta mezcla tenants. | Indice por tenant o filtro obligatorio por tenant. |
| LLM09 | Misinformation | Es el problema que Vigia ataca, pero un score mal calibrado da falsa seguridad. | Mostrar AUROC y fecha de calibracion junto a cada score. |
| LLM10 | Unbounded Consumption | OWASP pide restringir la exposicion de `logprobs` y `logit_bias`. Los scores por token de Vigia son una senal parecida. | Scores solo a principals autorizados, redondeo opcional, rate limit, cuotas. |

La pagina de LLM10 pide limites de tamano de input, throttling, cuotas por usuario, timeouts y limites de colas. Tambien pide restringir la exposicion de logprobs para evitar extraccion de modelo.

### 2. Autenticacion por API key

#### 2.1 Formato de la clave

GitHub disena sus tokens con prefijo identificable, separador `_` y checksum CRC32 en los ultimos 6 caracteres en Base62. El objetivo es el escaneo de secretos con pocos falsos positivos y sin consultar la base. La entropia de sus tokens OAuth sube de 160 a 178 bits.

El paquete `prefixed-api-key` usa el formato `PREFIX_ID_SECRET`. El ID es un ULID de 26 caracteres. El secreto tiene 32 bytes aleatorios. El servidor guarda `HMAC-SHA256(hmacKey, ID || SECRET)` y nunca el secreto.

El programa de partners de secret scanning de GitHub pide prefijo unico, regex precisa, alta entropia y checksum. GitHub envia los hallazgos a un endpoint publico firmado con ECDSA P-256. Ese programa solo sirve si Vigia tiene un endpoint en internet. En air-gapped no aplica.

#### 2.2 Comparacion de esquemas de almacenamiento

| Esquema | Costo por request | Resistencia a fuerza bruta offline | Veredicto para Vigia |
|---|---|---|---|
| Texto plano | Nulo | Ninguna | Prohibido. |
| SHA-256 sin sal | Muy bajo | Alta si el secreto tiene 256 bits | Aceptable, pero sin defensa si roban la base y el secreto es corto. |
| HMAC-SHA256 con pepper en secreto aparte | Muy bajo | Alta, y requiere ademas el pepper | Recomendado. |
| Argon2id por request | Alto (64 MiB por verificacion con defaults de argon2-cffi) | Muy alta | No recomendado: el secreto ya tiene alta entropia y el costo habilita DoS. |

La recomendacion es HMAC-SHA256 con pepper. El pepper vive en el gestor de secretos, no en la base. El hash de passwords con Argon2 sirve para secretos de baja entropia. Una API key de 256 bits no lo necesita.

#### 2.3 Ciclo de vida

- La clave se muestra una sola vez al crearla.
- La base guarda `key_id`, `prefix` visible, `verifier`, `tenant_id`, `scopes`, `created_by`, `created_at`, `expires_at`, `last_used_at`, `revoked_at`.
- La rotacion crea una clave nueva y deja la vieja activa durante una ventana configurable.
- La revocacion es inmediata y no tiene cache mayor a unos segundos.
- La clave nunca viaja por query string, porque las URLs terminan en logs de proxy.

### 3. Sesiones del panel y passwords

#### 3.1 Password hashing

| Algoritmo | Parametros minimos OWASP | Cuando usar |
|---|---|---|
| Argon2id | m=19 MiB, t=2, p=1 (equivalentes: 46 MiB/t=1, 12 MiB/t=3, 9 MiB/t=4, 7 MiB/t=5) | Opcion por defecto. |
| scrypt | N=2^17, r=8, p=1 | Si no hay Argon2id. |
| bcrypt | factor 10 o mas, limite de 72 bytes | Solo legado. |
| PBKDF2-HMAC-SHA256 | 600.000 iteraciones | Si se exige FIPS-140. |

`argon2-cffi` usa por defecto el perfil RFC 9106 de baja memoria. Ese perfil usa t=3, m=64 MiB, p=4, hash de 32 bytes y sal de 16 bytes. Su metodo `check_needs_rehash()` permite rehashear al login cuando cambian los parametros.

La maquina de build tiene 8 GB de RAM. Diez logins simultaneos con 64 MiB consumen 640 MiB. Conviene parametrizar el costo y usar el minimo OWASP (19 MiB, t=2, p=1) en tests y dev.

OWASP sugiere un pepper opcional guardado fuera de la base de passwords.

#### 3.2 Reglas de password (NIST SP 800-63B rev 4)

- Minimo 15 caracteres si el password es el unico factor.
- Minimo 8 caracteres si hay MFA.
- Maximo soportado de al menos 64 caracteres.
- Sin reglas de composicion obligatorias.
- Chequeo contra una blocklist de passwords comunes o filtrados.
- Sin rotacion periodica forzada, salvo evidencia de compromiso.
- Maximo 100 intentos fallidos consecutivos antes de bloquear la cuenta.
- AAL2: reautenticacion cada 24 horas como maximo y tras 1 hora de inactividad como maximo.

#### 3.3 Cookies y sesion

OWASP recomienda IDs de sesion con al menos 64 bits de entropia desde un CSPRNG. Tambien recomienda este formato de cookie: `__Host-` con `Secure`, `HttpOnly`, `SameSite=Strict` y `Path=/`, sin `Domain`.

| Fuente | Timeout de inactividad | Timeout absoluto |
|---|---|---|
| OWASP, apps de alto valor | 2 a 5 minutos | 4 a 8 horas |
| OWASP, apps de bajo riesgo | 15 a 30 minutos | no indica |
| NIST AAL2 | 1 hora como maximo | 24 horas como maximo |
| HIPAA 164.312(a)(2)(iii) | "Automatic logoff" es addressable, sin numero | sin numero |

OWASP pide regenerar el ID al login y al cambiar privilegios. Tambien pide invalidar la sesion en el servidor al logout. Los logs deben guardar el hash del ID de sesion, nunca el valor.

#### 3.4 CSRF

| Defensa | Estado en OWASP |
|---|---|
| Synchronizer token por sesion | Defensa primaria. |
| Signed double-submit cookie atado a la sesion con HMAC | Recomendada. |
| Double-submit naive | Desaconsejada: un subdominio vulnerable puede escribir la cookie. |
| `Sec-Fetch-Site: cross-site` rechazado en metodos no seguros | Defensa primaria simple, requiere HTTPS. |
| Header custom (ej. `X-CSRF-Token`) | Valida para clientes API. |
| `SameSite` | Solo defensa en profundidad; `Lax` solo bloquea metodos inseguros. |
| Verificacion de `Origin`/`Referer` | Defensa en profundidad. |

Las requests con API key en header `Authorization` no usan cookies. Por eso no necesitan token CSRF.

### 4. RBAC

Vigia necesita tres roles del pedido mas un rol de dueno del tenant. La matriz propuesta es esta.

| Accion | owner | admin | reviewer | viewer | API key `ingest` | API key `read` |
|---|---|---|---|---|---|---|
| Enviar generaciones para puntuar | no | no | no | no | si | no |
| Ver scores agregados y metricas | si | si | si | si | no | si |
| Ver texto de fragmentos dudosos | si | si | si | segun politica | no | segun politica |
| Resolver revision humana (aprobar, corregir, abstener) | si | si | si | no | no | no |
| Configurar acciones por fragmento y umbrales | si | si | no | no | no | no |
| Lanzar calibracion con juez | si | si | no | no | no | no |
| Crear, rotar y revocar API keys | si | si | no | no | no | no |
| Gestionar usuarios y roles | si | si (no puede crear owners) | no | no | no | no |
| Configurar retencion y borrar datos | si | no | no | no | no | no |
| Leer audit log | si | si | no | no | no | no |
| Exportar datos | si | si | no | no | no | no |

La regla base es deny-by-default. Cada endpoint declara el permiso requerido. Un test recorre todas las rutas y falla si alguna ruta no declara permiso.

### 5. Aislamiento por tenant

PostgreSQL RLS aplica deny-by-default al habilitarse. Los superusers y roles con `BYPASSRLS` saltean RLS siempre. El dueno de la tabla tambien lo saltea, salvo con `FORCE ROW LEVEL SECURITY`. `USING` filtra lectura y `WITH CHECK` filtra escritura. Las politicas restrictivas se combinan con AND.

La documentacion advierte tres huecos. Los chequeos de PK, unique y FK saltean RLS y abren un canal encubierto. `TRUNCATE` no respeta RLS. Las subconsultas en politicas pueden tener condiciones de carrera.

SQLite no tiene RLS. La laptop de build usa CPU y 8 GB de RAM, y probablemente corre SQLite en tests. Entonces el aislamiento debe vivir en dos capas.

| Capa | Mecanismo | Testeable en laptop |
|---|---|---|
| Aplicacion | Un unico repositorio con `tenant_id` obligatorio en cada query; sin acceso ORM directo fuera de el. | Si, con SQLite. |
| Base (prod) | Postgres RLS con `FORCE`, rol de app sin ownership ni `BYPASSRLS`, `SET LOCAL app.tenant_id` por transaccion. | Solo con Postgres en Docker o CI. |

### 6. Logs, PII y audit

#### 6.1 Que loguear y que no

OWASP pide loguear exitos y fallos de autenticacion, fallos de autorizacion, fallos de validacion, acciones administrativas, uso de claves, imports, exports y cambios de configuracion. Cada evento lleva cuando, donde, quien y que.

OWASP prohibe loguear IDs de sesion, tokens, passwords, claves de cifrado, datos de tarjeta, PII sensible y connection strings. Tambien pide sanitizar CR, LF y delimitadores para evitar log injection. Pide deteccion de manipulacion y acceso restringido a los logs.

#### 6.2 Redaccion de PII

Presidio es un framework MIT de deteccion y anonimizacion. Combina NER, regex, reglas y checksums, y acepta recognizers propios. Su README dice que no garantiza encontrar toda la informacion sensible. El repo indica que el proyecto se mudo de organizacion. No confirme el soporte de espanol en la pagina leida [no verificado].

Vigia recibe datos argentinos. Los recognizers propios necesarios son estos [patrones a validar con tests, no con fuente oficial]:

| Dato | Patron | Validacion |
|---|---|---|
| DNI | 7 u 8 digitos, con o sin puntos | Contexto ("DNI") para bajar falsos positivos. |
| CUIT/CUIL | `XX-XXXXXXXX-X` | Digito verificador modulo 11. |
| CBU | 22 digitos | Dos digitos verificadores. |
| Tarjeta | 13 a 19 digitos | Luhn. |
| Email, telefono, IP | Regex estandar | Formato. |

#### 6.3 HIPAA: desidentificacion

45 CFR 164.514(b) ofrece dos caminos: determinacion por experto o Safe Harbor. Safe Harbor exige quitar 18 identificadores. Entre ellos estan nombres, fechas excepto el ano, telefonos, emails, numeros de historia clinica, cuentas, IPs, URLs y datos biometricos. Un redactor por regex no cubre nombres ni fechas con confiabilidad.

### 7. Rate limiting e input

- HTTP 429 con `Retry-After` es el patron estandar.
- El draft IETF `draft-ietf-httpapi-ratelimit-headers-11` define `RateLimit-Policy` y `RateLimit`. Es un Internet-Draft activo, actualizado el 23 de mayo de 2026. No es RFC.
- Ejemplo del draft: `RateLimit-Policy: "burst";q=100;w=60` y `RateLimit: "default";r=50;t=30`.
- Vigia necesita limites por API key, por tenant y por IP en el login.
- Vigia necesita limites de tamano de body, de tokens por request, de items por batch y de tiempo de procesamiento.

### 8. Headers de seguridad

| Header | Valor recomendado por OWASP |
|---|---|
| Content-Security-Policy | `script-src 'nonce-{RANDOM}' 'strict-dynamic'; object-src 'none'; base-uri 'none'` mas `frame-ancestors 'none'` |
| Strict-Transport-Security | `max-age=63072000; includeSubDomains; preload` |
| X-Content-Type-Options | `nosniff` |
| X-Frame-Options | `DENY` (CSP `frame-ancestors` es la alternativa moderna) |
| Referrer-Policy | `strict-origin-when-cross-origin` |
| Permissions-Policy | `geolocation=(), camera=(), microphone=()` |
| Cross-Origin-Opener-Policy | `same-origin` |
| Cross-Origin-Resource-Policy | `same-site` |
| Cache-Control | `no-store` en respuestas con datos sensibles |
| X-XSS-Protection | `0` u omitir |
| Server | Quitar o dejar no informativo |

OWASP advierte no crear un middleware que agregue nonce a todos los `<script>`. Ese middleware tambien le da nonce a scripts inyectados. La politica basica alternativa es `default-src 'self'; frame-ancestors 'self'; form-action 'self'`. OWASP recomienda `report-to` y deja `report-uri` como fallback deprecado.

### 9. Health checks

Kubernetes distingue tres probes.

| Probe | Pregunta | Accion si falla | Que debe chequear Vigia |
|---|---|---|---|
| Startup | Termino de arrancar | Reinicia | Modelo, detector y calibracion cargados. |
| Liveness | Sigue vivo el proceso | Reinicia | Solo el proceso; nunca la base ni el modelo remoto. |
| Readiness | Puede recibir trafico | Saca del balanceo | Base accesible, calibracion activa, cola bajo umbral, disco con espacio. |

La documentacion advierte que una liveness mal hecha causa fallas en cascada bajo carga. Tambien recomienda startup probe para contenedores lentos, en lugar de subir el delay de liveness. Cargar un modelo MoE es lento, asi que Vigia necesita startup probe.

### 10. Metricas

Prometheus pide sufijos de unidad en plural, `_total` en counters y unidades base (segundos, bytes). Cada combinacion de labels crea una serie nueva. Prometheus desaconseja labels con IDs de usuario, emails u otros conjuntos no acotados.

Las convenciones GenAI de OpenTelemetry se mudaron al repo `semantic-conventions-genai`. No pude leer la lista de metricas ni su estado de estabilidad [no verificado]. Los nombres `gen_ai.client.token.usage` y `gen_ai.client.operation.duration` aparecen como ejemplos en la pagina vieja.

### 11. Despliegue air-gapped y secretos

Docker monta los secretos en `/run/secrets/<nombre>` sobre tmpfs. Docker no los pasa como variables de entorno a proposito, porque las variables se filtran entre contenedores. Muchas imagenes oficiales aceptan variantes `_FILE` (ej. `WORDPRESS_DB_PASSWORD_FILE`). Los secretos cifrados completos son exclusivos de swarm. Con `docker compose` standalone los secretos vienen de archivos sin la gestion cifrada de swarm.

Safetensors es un formato para guardar tensores de forma segura, a diferencia de pickle. Los repos de gpt-oss y transformers lo usan.

vLLM documenta riesgos que afectan a Vigia si se integra como sidecar:

- `--api-key` protege solo rutas bajo `/v1`, `/v2`, `/inference` y `/cohere`.
- `/invocations` da inferencia sin credenciales.
- `/pause`, `/resume`, `/abort_requests` y `/update_weights` no tienen auth.
- La comunicacion entre nodos no esta cifrada por defecto.
- `VLLM_SERVER_DEV_MODE=1` expone RPC arbitrario por `/collective_rpc`.
- La interfaz gRPC no tiene autenticacion ni cifrado.
- vLLM recomienda un reverse proxy con allowlist de endpoints y red aislada.

### 12. Cumplimiento

#### 12.1 Argentina

| Norma | Contenido relevante | Implicancia |
|---|---|---|
| Ley 25.326, art. 2 y 7 | Define datos sensibles (origen racial, opiniones politicas, religion, salud, entre otros) y restringe su recoleccion. | Fragmentos de salud y legal pueden ser datos sensibles. |
| Ley 25.326, art. 4 | Los datos se destruyen cuando dejan de ser necesarios. | Retencion con borrado automatico. |
| Ley 25.326, art. 9 | Medidas tecnicas y organizativas de seguridad y confidencialidad. | Base legal del checklist. |
| Ley 25.326, art. 10 | Secreto profesional, aun despues de terminada la relacion. | Acceso de soporte controlado. |
| Ley 25.326, art. 12 | Transferencia internacional prohibida a paises sin proteccion adecuada. | Un juez LLM en una API extranjera es una transferencia. |
| Ley 25.326, art. 14 | Acceso: respuesta en 10 dias corridos. | Export por titular o por request. |
| Ley 25.326, art. 16 | Rectificacion y supresion: 5 dias habiles. | Borrado por request y por identificador. |
| AAIP Res. 47/2018, Anexo I | Medidas recomendadas: recoleccion, control de acceso, control de cambios, respaldo, vulnerabilidades, destruccion, incidentes, entornos de desarrollo. Derogo las Disposiciones 11/06 y 9/08. | Mapear el checklist a estas 8 categorias. |
| Ley 27.483 | Aprobo el Convenio 108 y su protocolo adicional (fuente IAPP). | Marco de transferencias. |
| Ley 27.699 | Adhesion al Convenio 108+ (fuente secundaria). | Minimizacion y privacidad por diseno. |
| Expediente 3397-D-2026 | Proyecto del 16 de julio de 2026 para derogar la Ley 25.326, basado en GDPR (fuentes secundarias). | Disenar ya con estandar GDPR. |
| BCRA Com. "A" 7724 (10 de marzo de 2023) | Riesgo tecnologico de entidades financieras; incluye gestion de terceras partes (fuente secundaria). | Clientes bancarios auditaran a Vigia como tercero. |

La decision de adecuacion de la Comision Europea para Argentina (2003/490/CE) aparece en fuentes secundarias [no verificado en fuente primaria]. No encontre la Disposicion 60-E/2016 sobre paises adecuados [no verificado].

#### 12.2 HIPAA

| Control 45 CFR 164.312 | Tipo |
|---|---|
| Identificacion unica de usuario | Required |
| Procedimiento de acceso de emergencia | Required |
| Logoff automatico | Addressable |
| Cifrado y descifrado | Addressable |
| Controles de auditoria | Estandar sin etiqueta |
| Mecanismo de integridad | Addressable |
| Autenticacion de persona o entidad | Estandar sin etiqueta |
| Integridad en transmision | Addressable |
| Cifrado en transmision | Addressable |

164.316(b)(2) exige retener la documentacion de politicas 6 anos desde su creacion o ultima vigencia. Esa regla aplica a documentacion de politicas y procedimientos. No es una regla directa sobre logs de aplicacion.

La NPRM del 6 de enero de 2025 propone volver obligatorios el cifrado y el MFA. Segun fuentes secundarias, no hay regla final a octubre de 2026 y la agenda de OMB apunta a julio de 2027 [no verificado en Federal Register].

Vigia autoalojado sin acceso del proveedor a la PHI probablemente no convierte al proveedor en business associate. El soporte con acceso a datos si podria hacerlo [no verificado: requiere opinion legal].

#### 12.3 GDPR

| Articulo | Contenido | Implicancia |
|---|---|---|
| Art. 28(3) | El encargado procesa solo bajo instrucciones, garantiza confidencialidad, aplica art. 32, controla subencargados, asiste en derechos, borra o devuelve al terminar, permite auditorias. | El plan pago con soporte necesita DPA y offboarding con export y borrado. |
| Art. 32 | Seudonimizacion, cifrado, CIA y resiliencia, restauracion, pruebas periodicas. | Backups probados y tests de seguridad recurrentes. |
| Art. 33 | Responsable notifica en 72 horas; encargado avisa al responsable sin demora indebida. | Runbook de incidentes y export del audit log. |

#### 12.4 EU AI Act

El art. 50 rige desde el 2 de agosto de 2026. Exige informar a las personas que interactuan con IA y marcar contenido sintetico en formato legible por maquina. Tambien exige revelar texto generado publicado para informar al publico, salvo revision editorial humana.

Segun fuentes secundarias, el Digital Omnibus (acuerdo provisorio del 7 de mayo de 2026) no posterga el art. 50. Da un plazo hasta el 2 de diciembre de 2026 para el marcado del art. 50(2) en sistemas lanzados antes del 2 de agosto de 2026. Posterga obligaciones de alto riesgo del Anexo III al 2 de diciembre de 2027. Esas fechas siguen pendientes de adopcion formal [no verificado en el Diario Oficial].

El Anexo III incluye scoring crediticio, precio de seguros de vida y salud, elegibilidad para beneficios publicos y apoyo a decisiones judiciales. Vigia no es un sistema de alto riesgo por si mismo. Sus clientes de finanzas, salud, legal y gobierno pueden integrarlo en sistemas de alto riesgo. Esos clientes van a pedir documentacion tecnica, logs y metricas de precision.

### 13. Licencias

gpt-oss usa Apache 2.0. Su README dice que permite construir sin restricciones copyleft ni riesgo de patentes. Su `USAGE_POLICY` es corta y pide cumplir la ley aplicable. gpt-oss-120b tiene 117B parametros y 5.1B activos por token. gpt-oss-20b tiene 21B y 3.6B activos.

| Modelo de licencia | Ejemplo | Ventaja | Desventaja |
|---|---|---|---|
| Apache 2.0 en todo el core | gpt-oss | Concesion de patentes, aceptada por legales corporativos, compatible con el modelo objetivo. | Un tercero puede ofrecerlo como servicio sin aportar. |
| AGPLv3 en el core | Grafana, Loki, Tempo desde 2021 | Obliga a compartir modificaciones de servicios en red. Grafana mantuvo plugins, agentes y librerias en Apache 2.0. | Muchos legales corporativos rechazan AGPL [no verificado como dato]. |
| Core permisivo mas directorio `ee/` con licencia comercial | Langfuse: MIT fuera de `ee/`, licencia propia dentro de `ee/`, `web/src/ee/` y `worker/src/ee/` | Un solo repo y frontera clara. | Requiere disciplina para que el core no importe `ee/`. |

Las licencias de Mixtral, Qwen-MoE, DeepSeek, OLMoE y Granite no fueron verificadas en esta nota [no verificado].

## Implicancias para Vigia

### Decisiones de diseno recomendadas

1. **D-01 Licencia.** La libreria `vigia` (extractor, detector, evaluacion, CLI) usa Apache 2.0. El panel, la calibracion gestionada y el multi-tenant van en `ee/` con licencia comercial, al estilo Langfuse. Un test de CI falla si un modulo del core importa `ee/`.
2. **D-02 Juez local por defecto.** La calibracion usa por defecto un juez autoalojado (por ejemplo gpt-oss-120b). Un juez por API externa requiere opt-in explicito del owner, queda en el audit log y pasa por redaccion previa. Esto evita una transferencia internacional (Ley 25.326 art. 12, GDPR cap. V).
3. **D-03 Modo "solo scores".** Cada tenant elige `store_text: none | redacted | full`. El default es `redacted`. En `none` Vigia guarda scores, offsets y hash del texto, y el panel pide el texto al sistema del cliente.
4. **D-04 API keys con HMAC.** El formato es `vg_<env>_<key_id>_<secret><checksum>`. El servidor guarda `HMAC-SHA256(pepper[kid], key_id || secret)` y compara en tiempo constante.
5. **D-05 Aislamiento en dos capas.** Un `TenantScopedRepository` obligatorio en la app, mas Postgres RLS con `FORCE` en prod.
6. **D-06 Respuesta 404 entre tenants.** Un recurso de otro tenant devuelve 404, no 403, para no confirmar su existencia.
7. **D-07 Proxy obligatorio delante de vLLM.** Vigia nunca expone vLLM directo. El reverse proxy hace allowlist de `/v1/chat/completions` y `/v1/completions`.
8. **D-08 Metricas en puerto interno.** `/metrics` escucha en un puerto separado, no publicado fuera de la red de monitoreo.
9. **D-09 Sin dependencias de red en runtime.** El panel sirve sus assets propios, sin CDNs. La imagen no hace telemetria hacia afuera.
10. **D-10 Costo de hash configurable.** Argon2id usa el minimo OWASP (19 MiB, t=2, p=1) como default para entornos chicos, configurable hacia arriba.

### Checklist de seguridad accionable y testeable

Cada item tiene un ID, el control y el test automatico que lo prueba.

#### Autenticacion API

| ID | Control | Test |
|---|---|---|
| SEC-01 | La base nunca guarda el secreto de la API key. | Crear clave; leer la fila; afirmar que el secreto no aparece en ninguna columna. |
| SEC-02 | La verificacion usa HMAC con pepper y comparacion en tiempo constante. | Unit test con `hmac.compare_digest`; test que cambia el pepper y espera 401. |
| SEC-03 | La clave lleva prefijo y checksum. | Clave con checksum alterado recibe 401 sin consultar la base (mock de repo sin llamadas). |
| SEC-04 | La clave revocada falla de inmediato. | Revocar y llamar: 401 en la siguiente request. |
| SEC-05 | La clave expirada falla. | Clave con `expires_at` pasado: 401. |
| SEC-06 | La rotacion mantiene la vieja durante la ventana configurada. | Rotar con ventana de 60 s; vieja y nueva funcionan; tras la ventana, vieja 401. |
| SEC-07 | La API rechaza claves en query string. | `?api_key=...` devuelve 400 y el valor no aparece en logs. |
| SEC-08 | Los scopes se aplican. | Clave `ingest` en endpoint de lectura: 403. |
| SEC-09 | `last_used_at` se actualiza. | Llamar y afirmar timestamp nuevo. |

#### Panel: passwords, sesiones y CSRF

| ID | Control | Test |
|---|---|---|
| SEC-10 | Passwords con Argon2id y rehash al login si cambian parametros. | Hash empieza con `$argon2id$`; cambiar parametros y verificar rehash tras login. |
| SEC-11 | Politica NIST: minimo 15 sin MFA, maximo de al menos 64, sin composicion, blocklist. | Rechazar `password123456789` por blocklist; aceptar frase de 64 caracteres. |
| SEC-12 | Throttling de login por cuenta y por IP. | 11 intentos fallidos en un minuto: 429 [umbral a definir en Plan]. |
| SEC-13 | Cookie `__Host-` con `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`, sin `Domain`. | Parsear `Set-Cookie` y afirmar cada atributo. |
| SEC-14 | ID de sesion con al menos 128 bits desde CSPRNG. | Longitud y origen `secrets.token_urlsafe(32)`. |
| SEC-15 | Regeneracion de sesion al login y al cambiar rol. | ID antes y despues del login difieren; sesion vieja invalida. |
| SEC-16 | Timeout de inactividad y absoluto en servidor. | Reloj simulado: inactividad mayor al limite devuelve 401. |
| SEC-17 | Logout invalida en servidor. | Reusar cookie tras logout: 401. |
| SEC-18 | CSRF firmado atado a sesion en todo metodo no seguro. | POST sin token: 403; token de otra sesion: 403. |
| SEC-19 | Rechazo de `Sec-Fetch-Site: cross-site` en metodos no seguros. | POST con ese header: 403. |
| SEC-20 | MFA TOTP opcional para owner y admin [requisito propuesto, no exigido por fuente]. | Login de admin con MFA activo sin codigo: 401. |

#### Autorizacion y aislamiento

| ID | Control | Test |
|---|---|---|
| SEC-21 | Toda ruta declara permiso; deny-by-default. | Test que introspecciona el router y falla si una ruta no tiene permiso declarado. |
| SEC-22 | Matriz RBAC aplicada. | Test parametrizado: cada rol por cada ruta contra la matriz esperada. |
| SEC-23 | Aislamiento por objeto entre tenants. | Dos tenants; para cada ruta con ID, el tenant B pide IDs de A: siempre 404. |
| SEC-24 | Aislamiento en listados y agregados. | Metricas y listados de B no cuentan filas de A. |
| SEC-25 | Aislamiento en exports. | Export de B no contiene `tenant_id` de A. |
| SEC-26 | RLS en Postgres con `FORCE` y rol sin `BYPASSRLS`. | Test de integracion: query sin `app.tenant_id` devuelve 0 filas. |
| SEC-27 | Escalada de rol bloqueada. | Admin intenta crear owner: 403. Reviewer intenta cambiar umbral: 403. |

#### Datos, retencion y borrado

| ID | Control | Test |
|---|---|---|
| SEC-28 | Retencion configurable por tenant con purga automatica. | Insertar con fecha vieja; correr job; filas borradas. |
| SEC-29 | Borrado por request y por tenant completo. | Borrar; no queda texto ni score en ninguna tabla. |
| SEC-30 | Export de datos del tenant (art. 14 Ley 25.326, art. 28 GDPR). | Export devuelve todos los registros del tenant en JSON. |
| SEC-31 | Modo `store_text=none` no persiste texto. | Ingestar con ese modo; buscar el texto en la base: ausente. |
| SEC-32 | Redaccion de DNI, CUIT/CUIL, CBU, tarjeta, email y telefono en modo `redacted`. | Corpus de prueba en espanol; cero hallazgos tras redaccion; CUIT con digito invalido no se marca. |

#### Logs y audit

| ID | Control | Test |
|---|---|---|
| SEC-33 | Logs sin texto de prompts ni completions. | Ingestar texto con marcador unico; buscar el marcador en logs: ausente. |
| SEC-34 | Logs sin secretos (keys, cookies, passwords). | Igual que SEC-33 con una API key y una cookie. |
| SEC-35 | Sanitizacion de CR/LF en campos logueados. | Usuario con `\n` en el nombre produce una sola linea de log. |
| SEC-36 | Audit log append-only con cadena de hashes. | Modificar una fila; el verificador detecta el corte de la cadena. |
| SEC-37 | Audit de eventos clave: login ok y fallido, cambios de rol, keys, umbrales, calibracion, exports, borrados, cambio de retencion. | Cada accion genera exactamente un evento con actor, tenant, accion, objeto y resultado. |

#### Abuso, input y salida

| ID | Control | Test |
|---|---|---|
| SEC-38 | Rate limit por API key y por tenant con 429 y `Retry-After`. | Rafaga por encima del limite: 429 con header. |
| SEC-39 | Limite de tamano de body. | Body sobre el limite: 413. |
| SEC-40 | Limite de tokens por request y de items por batch. | Exceder: 422 con mensaje claro. |
| SEC-41 | Timeout de procesamiento. | Detector simulado lento: 504 y sin bloqueo del worker. |
| SEC-42 | Escape del texto del modelo en el panel. | Fragmento `<img src=x onerror=alert(1)>` se renderiza como texto. |
| SEC-43 | Filtrado de caracteres bidi y de control al renderizar. | Fragmento con U+202E se muestra neutralizado. |
| SEC-44 | Export CSV sin formula injection. | Celda que empieza con `=`, `+`, `-` o `@` sale prefijada con `'`. |
| SEC-45 | Respuesta del juez validada contra schema estricto. | Respuesta con campos extra o texto libre se descarta y se cuenta. |
| SEC-46 | Allowlist de hosts para webhooks y re-consulta RAG. | URL hacia `169.254.169.254` o `localhost`: rechazada. |

#### Headers, errores y supply chain

| ID | Control | Test |
|---|---|---|
| SEC-47 | Headers de la tabla de la seccion 8 en toda respuesta HTML. | Test que pide cada pagina y compara headers. |
| SEC-48 | CSP con nonce por respuesta, sin `unsafe-inline`. | Dos requests tienen nonces distintos; la politica no contiene `unsafe-inline`. |
| SEC-49 | Errores sin stack trace ni versiones. | Forzar excepcion: respuesta JSON generica con `request_id`. |
| SEC-50 | Fail-closed en auth ante error interno. | Repo de keys que lanza excepcion: 503, nunca 200. |
| SEC-51 | Solo safetensors y `trust_remote_code=False`. | Cargar un `.bin` pickle: error explicito. |
| SEC-52 | Hash fijado de pesos y de calibraciones. | Alterar un byte: la carga falla. |
| SEC-53 | Dependencias fijadas con hashes y escaneo de vulnerabilidades. | `pip install --require-hashes` en CI; `pip-audit` sin hallazgos altos. |
| SEC-54 | Licencias de dependencias compatibles. | Escaneo de licencias falla ante GPL/AGPL en el core. |

#### Operacion

| ID | Control | Test |
|---|---|---|
| OPS-01 | `/livez` solo chequea el proceso. | Base caida: `/livez` 200, `/readyz` 503. |
| OPS-02 | `/readyz` chequea base, calibracion activa y cola. | Sin calibracion cargada: 503 con causa en JSON interno. |
| OPS-03 | `/startupz` pasa cuando terminan de cargar detector y calibracion. | Antes de la carga: 503; despues: 200. |
| OPS-04 | Probes sin datos sensibles ni versiones exactas. | Respuesta de probes sin nombres de tenant. |
| OPS-05 | Metricas con nombres Prometheus y sin labels no acotados. | Test que parsea `/metrics` y falla si un label contiene `request_id`, `user` o `email`. |
| OPS-06 | Metrica de latencia agregada por Vigia. | `vigia_overhead_seconds` (histogram) y `vigia_overhead_ratio` existen y suben tras una request. |
| OPS-07 | Contenedor no-root, filesystem de solo lectura, sin capabilities extra. | Test sobre `docker inspect` o sobre el Dockerfile (`USER` distinto de root). |
| OPS-08 | Secretos por archivo (`/run/secrets` o `*_FILE`), no por variable plana. | Arrancar sin el archivo: falla con mensaje claro; el secreto no aparece en `/proc/self/environ`. |
| OPS-09 | Arranque offline. | Correr el contenedor con `--network none`: arranca y pasa readiness con pesos locales. |
| OPS-10 | Backup y restore probados. | Script de restore sobre una base vacia reproduce conteos. |
| OPS-11 | Rotacion del pepper con `kid`. | Claves viejas validan con `kid` viejo; nuevas usan `kid` nuevo. |
| OPS-12 | Runbook de incidente con export del audit log por rango. | Endpoint de export por rango devuelve eventos ordenados y verificables. |

### Metricas propuestas

| Nombre | Tipo | Labels |
|---|---|---|
| `vigia_requests_total` | counter | `route`, `method`, `status` |
| `vigia_request_duration_seconds` | histogram | `route` |
| `vigia_overhead_seconds` | histogram | `model` |
| `vigia_tokens_scored_total` | counter | `model` |
| `vigia_low_confidence_tokens_total` | counter | `model`, `action` |
| `vigia_calibration_info` | gauge (valor 1) | `version`, `model` |
| `vigia_auth_failures_total` | counter | `kind` (`api_key`, `password`, `csrf`) |
| `vigia_rate_limited_total` | counter | `scope` |
| `vigia_review_queue_depth` | gauge | ninguno |

El label `tenant` solo es aceptable si la cantidad de tenants esta acotada. Si no, va en logs y no en metricas.

### Documentacion de cumplimiento que el producto debe entregar

- Una matriz que mapea el checklist a Ley 25.326 art. 9, AAIP Res. 47/2018, HIPAA 164.312 y GDPR art. 32.
- Un modelo de DPA para el plan con soporte.
- Una ficha tecnica del detector con AUROC, latencia y limites, util para clientes en Anexo III.
- Un aviso de texto para que el cliente informe uso de IA (art. 50) cuando muestre fragmentos marcados a usuarios finales.

## Edge cases y riesgos

1. **PII partida en tokens.** Un DNI puede quedar repartido en varios tokens. Una redaccion por token no lo detecta. La redaccion debe correr sobre el texto completo y luego mapear offsets a tokens.
2. **Scores como canal lateral.** Los scores por token se parecen a logprobs. OWASP LLM10 pide restringir logprobs para evitar extraccion. Exponer scores sin rate limit facilita destilar el detector o el modelo.
3. **Juez envenenado por el propio texto.** Un texto del cliente con instrucciones puede sesgar las etiquetas de calibracion. Un lote con distribucion de etiquetas anomala debe frenar la calibracion.
4. **Juez externo como transferencia internacional.** Enviar fragmentos a una API fuera del pais puede violar Ley 25.326 art. 12. La configuracion por defecto debe ser local.
5. **Argon2 y RAM.** Con defaults de argon2-cffi (64 MiB, p=4) y logins concurrentes, una laptop de 8 GB puede quedarse sin memoria. Tambien es un vector de DoS en prod.
6. **SQLite sin RLS.** Los tests en laptop no prueban RLS. Hace falta un job de CI con Postgres, o el aislamiento real queda sin probar.
7. **FK y unique saltean RLS.** Un intento de insertar un ID duplicado de otro tenant revela que ese ID existe. Los IDs deben ser aleatorios (UUIDv4 o ULID), no secuenciales.
8. **Borrado y backups.** Un borrado por pedido del titular no borra backups. La politica debe declarar la ventana de backups y re-aplicar borrados tras un restore.
9. **Logs del reverse proxy.** Nginx loguea URLs completas. Si algun cliente manda la clave en query string, queda escrita. Por eso SEC-07 rechaza ese caso y lo documenta.
10. **Endpoints de vLLM sin auth.** `/invocations`, `/pause` y `/update_weights` quedan abiertos aunque se use `--api-key`. Un despliegue que publica vLLM entero queda expuesto.
11. **CSV injection.** Revisores abren exports en Excel. Un fragmento que empieza con `=` puede ejecutar formulas.
12. **Caracteres bidi.** Un fragmento con U+202E puede invertir el texto mostrado y enganar al revisor.
13. **Revocacion con cache.** Una cache de keys de varios minutos deja una clave revocada activa. El TTL debe ser de segundos.
14. **Reloj desfasado.** Expiraciones y timeouts dependen del reloj. Los tests deben usar un reloj inyectable.
15. **Tenant borrado con trabajos en curso.** Una calibracion en curso puede escribir filas despues del borrado. El job debe chequear el estado del tenant antes de escribir.
16. **Falsa seguridad.** Un AUROC de 0.76 por token deja muchos errores sin marcar. El panel debe decir que la ausencia de marca no prueba correccion (LLM09).
17. **Labels de metricas con PII.** Un label con nombre de modelo custom o de tenant puede contener datos del cliente. Los valores de labels deben venir de un conjunto cerrado.
18. **Timeouts de sesion vs uso real.** OWASP sugiere 2 a 5 minutos de inactividad para alto valor. Eso molesta a un revisor que lee un fragmento largo. Conviene default de 30 minutos, configurable, dentro del tope NIST de 1 hora.
19. **Air-gapped sin secret scanning.** El partner program de GitHub requiere un endpoint publico. En air-gapped la deteccion de fugas depende del prefijo y de escaneos internos.

## Objeciones

- No confirme si OWASP Top 10:2025 es edicion final o release candidate.
- No pude leer la lista de metricas GenAI de OpenTelemetry ni su estabilidad.
- El estado de la regla final de HIPAA viene de fuentes secundarias.
- Las fechas del Digital Omnibus son provisorias y vienen de fuentes secundarias.
- No lei el texto primario de la BCRA Com. "A" 7724 ni de la Ley 27.699.
- No encontre la Disposicion 60-E/2016 sobre paises adecuados.
- El resumen de AAIP Res. 47/2018 cubre las 8 categorias, no cada medida.
- No verifique el soporte de espanol de Presidio.
- No verifique las licencias de Mixtral, Qwen-MoE, DeepSeek, OLMoE y Granite.
- No verifique si la Ley 26.529 de derechos del paciente agrega requisitos sobre historias clinicas.
- La condicion de business associate o de encargado del proveedor depende del modelo de soporte. Requiere opinion legal.
- Falta un dato de mercado sobre cuantos legales corporativos rechazan AGPL.
- Los umbrales concretos de rate limit y de throttling de login se definen en el Plan.

## Fuentes

Fuentes primarias leidas:

- OWASP Top 10 LLM 2025: https://genai.owasp.org/llm-top-10/
- OWASP LLM10 Unbounded Consumption: https://genai.owasp.org/llmrisk/llm102025-unbounded-consumption/
- OWASP Top 10:2025: https://top10.owasp.org/2025
- OWASP API Security Top 10 2023: https://api-security.owasp.org/editions/2023/en/0x11-t10
- OWASP Password Storage: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- OWASP CSRF Prevention: https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
- OWASP Session Management: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
- OWASP CSP: https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html
- OWASP HTTP Headers: https://cheatsheetseries.owasp.org/cheatsheets/HTTP_Headers_Cheat_Sheet.html
- OWASP Logging: https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html
- NIST SP 800-63B rev 4: https://pages.nist.gov/800-63-4/sp800-63b.html
- argon2-cffi API: https://argon2-cffi.readthedocs.io/en/stable/api.html
- GitHub, formato de tokens: https://github.blog/engineering/platform-security/behind-githubs-new-authentication-token-formats/
- GitHub secret scanning partner program: https://docs.github.com/en/code-security/secret-scanning/secret-scanning-partnership-program/secret-scanning-partner-program
- prefixed-api-key: https://github.com/truestamp/prefixed-api-key
- PostgreSQL RLS: https://www.postgresql.org/docs/current/ddl-rowsecurity.html
- Kubernetes probes: https://kubernetes.io/docs/concepts/configuration/liveness-readiness-startup-probes/
- Prometheus naming: https://prometheus.io/docs/practices/naming/
- OpenTelemetry GenAI semconv (repo nuevo): https://github.com/open-telemetry/semantic-conventions-genai
- IETF RateLimit headers draft-11: https://datatracker.ietf.org/doc/draft-ietf-httpapi-ratelimit-headers/
- Docker secrets: https://docs.docker.com/engine/swarm/secrets/
- vLLM security: https://docs.vllm.ai/en/latest/usage/security.html
- Presidio: https://github.com/microsoft/presidio
- safetensors: https://huggingface.co/docs/safetensors/index
- gpt-oss: https://github.com/openai/gpt-oss
- gpt-oss USAGE_POLICY: https://raw.githubusercontent.com/openai/gpt-oss/main/USAGE_POLICY
- Langfuse LICENSE: https://github.com/langfuse/langfuse/blob/main/LICENSE
- Grafana, relicencia AGPLv3: https://grafana.com/blog/2021/04/20/grafana-loki-tempo-relicensing-to-agplv3/
- Ley 25.326 (texto actualizado): https://servicios.infoleg.gob.ar/infolegInternet/anexos/60000-64999/64790/texact.htm
- AAIP Res. 47/2018: https://www.argentina.gob.ar/normativa/nacional/resolución-47-2018-312662/texto
- 45 CFR 164.312: https://www.law.cornell.edu/cfr/text/45/164.312
- 45 CFR 164.316: https://www.law.cornell.edu/cfr/text/45/164.316
- 45 CFR 164.514: https://www.law.cornell.edu/cfr/text/45/164.514
- GDPR art. 28: https://gdpr-info.eu/art-28-gdpr/
- GDPR art. 32: https://gdpr-info.eu/art-32-gdpr/
- GDPR art. 33: https://gdpr-info.eu/art-33-gdpr/
- EU AI Act art. 50: https://artificialintelligenceact.eu/article/50/
- EU AI Act Anexo III: https://artificialintelligenceact.eu/annex/3/

Fuentes secundarias (resultados de busqueda o articulos, menor confianza):

- Convenio 108 en Argentina (IAPP): https://iapp.org/news/a/argentina-ratifica-el-convenio-108-en-materia-de-proteccion-de-datos-personales
- Proyecto de reemplazo de la Ley 25.326 (Marval): https://www.marval.com/publicacion/nuevo-anteproyecto-busca-reemplazar-la-ley-de-proteccion-de-datos-argentina-14336
- BCRA Com. "A" 7724: https://abogados.com.ar/seguridad-informatica-comunicacion-a-7724-bcra/33748
- Estado de la regla HIPAA: https://compliancehub.wiki/blog/hipaa-security-rule-final-rule-may-2026-mfa-encryption-uncertainty
- Digital Omnibus y art. 50: https://compliancehub.wiki/eu-ai-act-article-50-transparency-digital-omnibus-2026/
