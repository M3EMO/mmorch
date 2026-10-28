//! Biblioteca de Root: evalúa, rutea y observa corridas.

pub mod aprende;
pub mod borrado;
pub mod cli;
pub mod config;
pub mod entorno;
pub mod error;
pub mod esquema;
pub mod ficha;
pub mod goal;
pub mod importar;
pub mod mcp;
pub mod observe;
pub mod oracle;
pub mod proveedores;
pub mod route;
pub mod tipos;
pub mod track;

pub use cli::Cli;
pub use error::Error;

use std::io;
use std::path::Path;

use cli::{
    Comando, FichaAccion, GoalAccion, ObservaAccion, OraculoAccion, ProveedorJuez, TrackAccion,
    ViaRegistro,
};
use ficha::{ContextoRevision, ErrorFicha, JuezProveedor, Revision};
use oracle::tarea::Tarea;
use tipos::{Eleccion, Estado, Fila, Proveedor, Via};

/// Ejecuta un subcomando y devuelve su código de salida. Los errores salen por `Err`.
/// Por ahora `route`, `llama`, `oraculo evaluar`, `ficha`, `esquema`, `goal`, `observa`, `track` e la importación externa tienen lógica.
pub fn ejecutar(cli: &Cli) -> Result<u8, Error> {
    match &cli.comando {
        Comando::Oraculo {
            accion: OraculoAccion::Evaluar { tarea, repo },
        } => oraculo_evaluar(cli.json, tarea, repo),
        Comando::Ficha { accion } => ficha_accion(cli.json, accion),
        Comando::Route {
            rol,
            tipo,
            checkeable,
            tokens,
        } => route::comando::route(
            cli.json,
            (*rol).into(),
            tipo,
            *checkeable,
            tokens.unwrap_or(0),
        )
        .map(|()| 0),
        Comando::Llama {
            rol,
            tipo,
            prompt_file,
            corrida,
            max_tokens,
        } => route::comando::llama((*rol).into(), tipo, prompt_file, corrida, *max_tokens),
        Comando::Mcp => {
            let entrada = io::stdin();
            let salida = io::stdout();
            mcp::servir(entrada.lock(), salida.lock())
                .map(|()| 0)
                .map_err(Error::Mcp)
        }
        _ => ejecutar_sin_codigo(cli).map(|()| 0),
    }
}

/// `oraculo evaluar`: imprime el veredicto. El código de salida sale de su estado.
fn oraculo_evaluar(json: bool, nombre: &str, repo: &Path) -> Result<u8, Error> {
    let veredicto = oracle::evaluar_por_nombre(nombre, repo)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&veredicto)?);
    } else {
        println!(
            "{}: contrato {}/{}, oculto {}/{}, estado {}, repeticiones {}, inestables {}",
            repo.display(),
            veredicto.contrato.pasa,
            veredicto.contrato.total,
            veredicto.oculto.pasa,
            veredicto.oculto.total,
            etiqueta_estado(veredicto.estado),
            veredicto.repeticiones,
            veredicto.inestables.len()
        );
    }
    Ok(oracle::codigo_de_estado(veredicto.estado))
}

/// `oraculo pendientes --colab`: corre el paquete en Colab, importa el junit y reporta.
/// Sin tests pendientes no toca Colab.
fn oraculo_colab(
    json: bool,
    tarea: &Tarea,
    home: &Path,
    paquete: Option<oracle::pendientes::Paquete>,
) -> Result<(), Error> {
    let Some(paquete) = paquete else {
        if json {
            println!("{}", serde_json::json!({"dir": null, "tests": []}));
        } else {
            println!("tests: []");
        }
        return Ok(());
    };
    let cfg = config::Config::cargar(&home.join("root.toml"))?;
    let cli = oracle::colab::ColabProceso::desde_entorno()?;
    let senales = oracle::colab::Senales::instalar().map_err(|e| {
        oracle::ErrorOraculo::Infra(format!("no se pudieron registrar las señales: {e}"))
    })?;
    let tope = std::time::Duration::from_secs(u64::from(cfg.colab.minutos_max) * 60);
    let sesion = oracle::colab::nombre_sesion(&tarea.nombre);
    let resultado =
        oracle::colab::correr_en_colab(&cli, &paquete.dir, tarea, home, &sesion, &senales, tope)?;
    let resumen = oracle::pendientes::importar(home, tarea, &resultado.junit)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "importados": resumen.importados,
                "ignorados": resumen.ignorados,
                "gpu": resultado.gpu,
                "sesion": resultado.sesion,
            })
        );
    } else {
        println!(
            "gpu: {}, sesion: {}, importados: {}, ignorados: {}",
            resultado.gpu, resultado.sesion, resumen.importados, resumen.ignorados
        );
    }
    Ok(())
}

/// `oraculo pendientes --liberar`: libera el runtime anotado en `activo.json`, si hay uno.
fn oraculo_liberar(json: bool, home: &Path) -> Result<(), Error> {
    let liberado = if oracle::colab::hay_runtime_anotado(home) {
        let cli = oracle::colab::ColabProceso::desde_entorno()?;
        oracle::colab::liberar_anotado(&cli, home)?
    } else {
        None
    };
    if json {
        println!("{}", serde_json::json!({"liberado": liberado}));
    } else {
        match liberado {
            Some(sesion) => println!("runtime liberado: {sesion}"),
            None => println!("no hay runtime anotado"),
        }
    }
    Ok(())
}

/// `oraculo pendientes`: arma el paquete de los tests pendientes, o importa el junit de Colab.
fn oraculo_pendientes(
    json: bool,
    nombre: Option<&str>,
    repo: Option<&Path>,
    importar: Option<&Path>,
    colab: bool,
    liberar: bool,
) -> Result<(), Error> {
    let home = config::root_home();
    if liberar {
        return oraculo_liberar(json, &home);
    }
    let (Some(nombre), Some(repo)) = (nombre, repo) else {
        return Err(Error::Uso(
            "oraculo pendientes necesita --tarea y --repo".into(),
        ));
    };
    let tarea = Tarea::cargar(nombre)?;
    if let Some(junit) = importar {
        let resumen = oracle::pendientes::importar(&home, &tarea, junit)?;
        if json {
            println!(
                "{}",
                serde_json::json!({"importados": resumen.importados, "ignorados": resumen.ignorados})
            );
        } else {
            println!(
                "importados: {}, ignorados: {}",
                resumen.importados, resumen.ignorados
            );
        }
        return Ok(());
    }
    let paquete = oracle::pendientes::crear(&home, &tarea, repo)?;
    if colab {
        return oraculo_colab(json, &tarea, &home, paquete);
    }
    match paquete {
        None if json => println!("{}", serde_json::json!({"dir": null, "tests": []})),
        None => println!("tests: []"),
        Some(paquete) => {
            let dir = paquete.dir.display().to_string();
            if json {
                println!(
                    "{}",
                    serde_json::json!({"dir": dir, "tests": paquete.tests})
                );
            } else {
                println!("dir: {dir}");
                println!("tests: {}", serde_json::to_string(&paquete.tests)?);
            }
        }
    }
    Ok(())
}

/// Subcomandos `ficha`: revisan el spec o arman el borrador de `tarea.toml`.
/// `revisar` sale con 0 si no hay hallazgos ni ambigüedades, 1 si los hay, y 3 si el juez
/// pidió corrida y quedó `sin_veredicto`. Los errores de ficha salen con 2.
fn ficha_accion(json: bool, accion: &FichaAccion) -> Result<u8, Error> {
    match accion {
        FichaAccion::Revisar {
            spec,
            juez_proveedor,
            juez_modelo,
        } => {
            let texto = leer_spec(spec)?;
            let ctx = ContextoRevision::default();
            let revision = match juez_de(*juez_proveedor, juez_modelo.as_deref()) {
                Some(juez) => ficha::revisar_con_juez(&texto, &ctx, &juez)?,
                None => ficha::revisar(&texto, &ctx)?,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&revision)?);
            } else {
                imprimir_revision(&revision);
            }
            Ok(revision.codigo_de_salida())
        }
        FichaAccion::Tarea { spec, nombre } => {
            let texto = leer_spec(spec)?;
            let (ficha_leida, _) = ficha::extraer(&texto)?;
            let toml = ficha::borrador_tarea(&ficha_leida, nombre)?;
            if json {
                println!("{}", serde_json::json!({"toml": toml}));
            } else {
                print!("{toml}");
            }
            Ok(0)
        }
    }
}

fn leer_spec(spec: &Path) -> Result<String, ErrorFicha> {
    std::fs::read_to_string(spec).map_err(|fuente| ErrorFicha::Leer {
        ruta: spec.display().to_string(),
        fuente,
    })
}

/// Juez real si se pasan proveedor y modelo. Sin ellos no corre el juez.
fn juez_de(proveedor: Option<ProveedorJuez>, modelo: Option<&str>) -> Option<JuezProveedor> {
    let (proveedor, modelo) = (proveedor?, modelo?);
    Some(JuezProveedor {
        eleccion: Eleccion {
            via: Via::Api,
            proveedor: Some(Proveedor::from(proveedor)),
            modelo: Some(modelo.to_string()),
            razonamiento: true,
            motivo: "juez de ficha".into(),
        },
        corrida: "ficha-revisar".into(),
    })
}

/// Una línea por hallazgo y por ambigüedad: `linea: regla: detalle`.
fn imprimir_revision(revision: &Revision) {
    for h in &revision.hallazgos {
        println!("{}: {}: {}", h.linea, h.regla, h.detalle);
    }
    for a in &revision.ambiguedades {
        println!("{}: ambigüedad: {}", a.linea, a.frase);
    }
    if revision.juez.starts_with("sin_veredicto") {
        println!("juez: {}", revision.juez);
    }
    if revision.hallazgos.is_empty() && revision.ambiguedades.is_empty() {
        println!("sin hallazgos");
    }
}

fn etiqueta_estado(estado: Estado) -> &'static str {
    match estado {
        Estado::Verde => "verde",
        Estado::Rojo => "rojo",
        Estado::SinVeredicto => "sin_veredicto",
    }
}

/// Subcomandos sin código propio: salen con 0 o con error.
fn ejecutar_sin_codigo(cli: &Cli) -> Result<(), Error> {
    match &cli.comando {
        Comando::Esquema { tipo } => {
            println!("{}", esquema::json(tipo)?);
            Ok(())
        }
        Comando::Goal { accion } => {
            let repo = std::env::current_dir().map_err(goal::ErrorGoal::from)?;
            match accion {
                GoalAccion::Verificar => {
                    goal::verificar_goal(&repo)?;
                    let hash = goal::hash_del_repo(&repo)?;
                    if cli.json {
                        println!("{}", serde_json::json!({"ok": true, "hash": hash}));
                    } else {
                        println!("GOAL verificado: {hash}");
                    }
                    Ok(())
                }
                GoalAccion::Autorizar => {
                    goal::autorizar(&repo)?;
                    Ok(())
                }
            }
        }
        Comando::Oraculo {
            accion:
                OraculoAccion::Pendientes {
                    tarea,
                    repo,
                    importar,
                    colab,
                    liberar,
                },
        } => oraculo_pendientes(
            cli.json,
            tarea.as_deref(),
            repo.as_deref(),
            importar.as_deref(),
            *colab,
            *liberar,
        ),
        Comando::Observa { accion } => observa(cli.json, accion),
        Comando::Track { accion } => track_accion(cli.json, accion),
        Comando::Aprende { replay } => aprende_accion(cli.json, *replay),
        Comando::ImportarExterno { desde } => {
            let resumen = importar::importar_externo(desde)?;
            println!("{}", serde_json::to_string_pretty(&resumen)?);
            Ok(())
        }
        otro => Err(Error::NoImplementado(otro.ruta().to_string())),
    }
}

/// `aprende`: recalcula y escribe `politica.json`. Con `--replay`, mide la política sin escribirla.
fn aprende_accion(json: bool, replay: bool) -> Result<(), Error> {
    let home = config::root_home();
    let cfg = config::Config::cargar(&home.join("root.toml"))?;
    let filas = track::filas(&track::Filtro::default())?;
    if replay {
        let medicion = aprende::replay(&filas, &cfg);
        if json {
            println!("{}", serde_json::to_string_pretty(&medicion)?);
        } else {
            imprimir_replay(&medicion);
        }
        return Ok(());
    }
    let eventos = aprende::leer_eventos(&observe::ruta_eventos())?;
    let actuales = aprende::RolesActuales::de_config(&cfg);
    let pol = aprende::recalcular(&filas, &eventos, &actuales);
    aprende::escribir_politica(&home, &pol).map_err(aprende::ErrorAprende::from)?;
    registrar_propuestas(&pol)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&pol)?);
    } else {
        imprimir_politica(&pol, cfg.umbral_wilson);
        imprimir_propuestas(&pol.propuestas);
    }
    Ok(())
}

/// Un `Gate` por propuesta en `eventos.jsonl`. Sin propuestas, no escribe nada.
fn registrar_propuestas(pol: &route::Politica) -> Result<(), Error> {
    for p in &pol.propuestas {
        let mut extra = serde_json::Map::new();
        extra.insert("rol".into(), serde_json::to_value(p.rol)?);
        extra.insert("tarea".into(), serde_json::json!(p.tarea));
        extra.insert("modelo_actual".into(), serde_json::json!(p.modelo_actual));
        extra.insert(
            "modelo_propuesto".into(),
            serde_json::json!(p.modelo_propuesto),
        );
        extra.insert("evidencia".into(), serde_json::to_value(&p.evidencia)?);
        extra.insert(
            "diferencia".into(),
            serde_json::to_value(&p.diferencia_esperada)?,
        );
        observe::registrar(&observe::evento_gate("aprende", "propuesta", extra))
            .map_err(observe::ErrorObserve::from)?;
    }
    Ok(())
}

/// Sección «propuestas»: una línea por propuesta, o «sin propuestas».
fn imprimir_propuestas(propuestas: &[route::politica::Propuesta]) {
    if propuestas.is_empty() {
        println!("propuestas: sin propuestas");
        return;
    }
    println!("propuestas:");
    for p in propuestas {
        let (act, prop) = (&p.evidencia.actual, &p.evidencia.propuesto);
        println!(
            "{}:{}: {} -> {} (éxito {:+.3}, USD {:+.4}; evidencia: {} {}/{}/{:.3}, {} {}/{}/{:.3})",
            route::politica::nombre_rol(p.rol),
            p.tarea,
            p.modelo_actual,
            p.modelo_propuesto,
            p.diferencia_esperada.exito,
            -p.diferencia_esperada.usd,
            act.modelo,
            act.n,
            act.exitos,
            act.wilson_inf,
            prop.modelo,
            prop.n,
            prop.exitos,
            prop.wilson_inf,
        );
    }
}

/// Una línea por clave: el mejor modelo con su n, Wilson y USD medio.
fn imprimir_politica(pol: &route::Politica, umbral: f64) {
    if pol.estadisticas.is_empty() {
        println!("sin datos: política vacía");
    }
    for (clave, candidatos) in &pol.estadisticas {
        match aprende::mejor(candidatos, umbral) {
            Some(c) => println!(
                "{clave} → {} (n {}, wilson {:.2}, usd {:.4})",
                c.modelo, c.n, c.wilson_inf, c.usd_medio
            ),
            None => println!("{clave} → sin candidato sobre el umbral {umbral:.2}"),
        }
    }
}

/// Resumen legible de `aprende --replay`: las dos métricas y las mejoras.
fn imprimir_replay(r: &aprende::Replay) {
    println!(
        "entrenamiento: {} filas; medición: {} filas",
        r.entrenamiento, r.medicion
    );
    if let Some(aviso) = &r.advertencia {
        println!("advertencia: {aviso}");
    }
    imprimir_metrica("aprendida", &r.aprendida);
    imprimir_metrica("root.toml", &r.root_toml);
    println!("mejora en éxito: {}", fmt_opcional(r.mejora_exito, 3));
    println!(
        "mejora en USD por fila: {} (positivo ahorra)",
        fmt_opcional(r.mejora_usd, 4)
    );
}

fn imprimir_metrica(nombre: &str, m: &aprende::MetricaPolitica) {
    println!(
        "{nombre}: {} filas cubiertas, éxito {}, USD medio {}",
        m.cubiertas,
        fmt_opcional(m.exito, 3),
        fmt_opcional(m.usd_medio, 4)
    );
}

fn fmt_opcional(valor: Option<f64>, decimales: usize) -> String {
    valor.map_or("sin datos".to_string(), |v| format!("{v:.decimales$}"))
}

/// Subcomandos `track`: agregan una fila, las listan o comparan dos configuraciones.
fn track_accion(json: bool, accion: &TrackAccion) -> Result<(), Error> {
    match accion {
        TrackAccion::Agregar { fila } => {
            let texto = std::fs::read_to_string(fila).map_err(track::ErrorTrack::from)?;
            let nueva: Fila = serde_json::from_str(&texto).map_err(track::ErrorTrack::from)?;
            track::agregar(&nueva).map_err(track::ErrorTrack::from)?;
            if json {
                println!("{}", serde_json::json!({"agregada": nueva.id}));
            } else {
                println!("Fila agregada: {}", nueva.id);
            }
        }
        TrackAccion::Listar { tarea } => {
            let filtro = track::Filtro {
                tarea: tarea.clone(),
                config: None,
            };
            let filas = track::filas(&filtro)?;
            println!("{}", serde_json::to_string_pretty(&filas)?);
        }
        TrackAccion::Comparar { a, b, tarea } => {
            let comparacion = track::comparar(a, b, tarea)?;
            println!("{}", serde_json::to_string_pretty(&comparacion)?);
        }
    }
    Ok(())
}

/// Subcomandos `observa`: registran eventos y muestran el resumen de una corrida.
fn observa(json: bool, accion: &ObservaAccion) -> Result<(), Error> {
    match accion {
        ObservaAccion::Claude {
            archivo,
            corrida,
            via,
        } => {
            let mut eventos = observe::claude::leer_sesion_claude(archivo, corrida)?;
            let via = via.map(|v| match v {
                ViaRegistro::Cupo => Via::Cupo,
                ViaRegistro::Nube => Via::Nube,
            });
            if let Some(via) = via {
                eventos = observe::uso::con_via(eventos, via);
            }
            for ev in &eventos {
                observe::registrar(ev).map_err(observe::ErrorObserve::from)?;
            }
            imprimir_claude(json, eventos.len(), via);
        }
        ObservaAccion::Uso => {
            let estado = observe::uso::estado(observe::ahora().0)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&estado)?);
            } else {
                imprimir_uso(&estado);
            }
        }
        ObservaAccion::Evento {
            corrida,
            tipo,
            detalle,
        } => {
            observe::registrar_manual(corrida, (*tipo).into(), detalle.as_deref())?;
            imprimir_registrados(json, 1);
        }
        ObservaAccion::Resumen { corrida } => {
            let resumen = observe::resumir(corrida)?;
            println!("{}", serde_json::to_string_pretty(&resumen)?);
        }
    }
    Ok(())
}

/// Salida de `observa claude`: cuántos eventos y con qué vía. Sin `--via`, la vía es `cupo`.
fn imprimir_claude(json: bool, cuantos: usize, via: Option<Via>) {
    let etiqueta = match via {
        Some(Via::Nube) => "nube",
        _ => "cupo",
    };
    if json {
        println!(
            "{}",
            serde_json::json!({"registrados": cuantos, "via": etiqueta})
        );
    } else {
        println!("Registrados {cuantos} eventos con vía {etiqueta}");
    }
}

/// Una línea por tope: usado, tope, fracción y reapertura. Luego el aviso de la nube.
fn imprimir_uso(estado: &observe::uso::UsoMargenes) {
    let offset = observe::uso::offset_local();
    let filas = [
        ("cupo del bloque", &estado.margenes.cupo),
        ("API del mes", &estado.margenes.api),
        ("nube del mes", &estado.margenes.nube),
    ];
    for (nombre, m) in filas {
        let reabre = m
            .reabre
            .map_or("-".to_string(), |r| observe::uso::iso(r, offset));
        println!(
            "{nombre}: {:.2} de {:.2} USD ({:.0}%), reabre {reabre}",
            m.usado_usd,
            m.tope_usd,
            100.0 * m.fraccion
        );
    }
    println!("{}", estado.aviso);
}

fn imprimir_registrados(json: bool, cuantos: usize) {
    if json {
        println!("{}", serde_json::json!({"registrados": cuantos}));
    } else {
        println!("Registrados {cuantos} eventos");
    }
}
