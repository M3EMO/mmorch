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

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cli::{
    Comando, FichaAccion, GoalAccion, ObservaAccion, OraculoAccion, ProveedorJuez, TrackAccion,
    ViaRegistro,
};
use ficha::{ContextoRevision, ErrorFicha, JuezProveedor, Revision};
use oracle::tarea::Tarea;
use tipos::{Eleccion, Estado, Fila, Proveedor, Rol, Via};

/// Ejecuta un subcomando y devuelve su código de salida. Los errores salen por `Err`.
/// Por ahora `route`, `llama`, `oraculo evaluar`, `ficha`, `esquema`, `goal`, `observa`, `track` e la importación externa tienen lógica.
pub fn ejecutar(cli: &Cli) -> Result<u8, Error> {
    // Una `ROOT_HOME` mala (relativa, `/` o `$HOME`) sale con 2 antes de tocar nada (B6).
    config::validar_root_home_actual()?;
    match &cli.comando {
        Comando::Oraculo {
            accion: OraculoAccion::Evaluar { tarea, repo },
        } => oraculo_evaluar(cli.json, tarea, repo),
        Comando::Oraculo {
            accion: OraculoAccion::Informe { tarea, repo, md },
        } => oraculo_informe(tarea, repo, *md),
        Comando::Oraculo {
            accion:
                OraculoAccion::Calibrar {
                    tarea,
                    dimension,
                    n,
                    semilla,
                    repo,
                },
        } => oraculo_calibrar(cli.json, tarea, dimension, *n, *semilla, repo.as_deref()),
        Comando::Oraculo {
            accion:
                OraculoAccion::Validar {
                    casos,
                    preparar_ciego,
                    fuentes,
                    semilla,
                },
        } => oraculo_validar(
            cli.json,
            casos,
            *preparar_ciego,
            fuentes.as_deref(),
            *semilla,
        ),
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

/// `oraculo calibrar`: calibra una dimensión con el Oracle de producción, agrega la fila a
/// `calibracion.json` de `ROOT_HOME` e imprime la fila. Los errores de uso salen con 2.
fn oraculo_calibrar(
    json: bool,
    nombre: &str,
    dimension: &str,
    n: u32,
    semilla: u64,
    repo: Option<&Path>,
) -> Result<u8, Error> {
    let home = config::root_home();
    let fila = oracle::calibra::calibrar_tarea(
        &home,
        nombre,
        dimension,
        n,
        semilla,
        repo,
        &|tarea, ruta| oracle::evaluar(ruta, tarea, &oracle::OpcionesOraculo::default()),
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&fila)?);
    } else {
        println!(
            "{}/{}: sensibilidad {}, especificidad {}, exactitud {}, casos {} (defectos {}, limpios {}, no aplicables {})",
            fila.tarea,
            fila.dimension.clave(),
            medida(fila.sensibilidad),
            medida(fila.especificidad),
            medida(fila.exactitud_balanceada),
            fila.n,
            fila.defectos,
            fila.limpios,
            fila.no_aplicable
        );
    }
    Ok(0)
}

/// Una métrica con dos decimales, o `sin medir` si no tiene casos de una clase.
fn medida(valor: Option<f32>) -> String {
    valor.map_or_else(|| "sin medir".to_string(), |v| format!("{v:.2}"))
}

/// `oraculo validar`: mide la validez del juicio de Root y escribe `casos/validez.json` (Tarea 12b). Con
/// `--preparar-ciego`, arma los paquetes a ciegas. Sale con 0 aunque una métrica no cumpla: es una medida.
fn oraculo_validar(
    json: bool,
    casos: &Path,
    preparar_ciego: bool,
    fuentes: Option<&Path>,
    semilla: Option<u64>,
) -> Result<u8, Error> {
    if preparar_ciego {
        let fuentes =
            fuentes.ok_or_else(|| Error::Uso("--preparar-ciego exige --fuentes".into()))?;
        let semilla = semilla.unwrap_or_else(oracle::validez::semilla_del_reloj);
        let preparado = oracle::validez::preparar_ciego(casos, fuentes, semilla)?;
        if preparado.sin_rubricas {
            eprintln!(
                "aviso: no hay rúbricas embebidas; el formulario lleva solo la decisión y el orden"
            );
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&preparado)?);
        } else {
            let ciego = casos.join("ciego");
            println!(
                "{} paquetes a ciegas en {}",
                preparado.paquetes.len(),
                ciego.display()
            );
            for r in &preparado.rastros {
                println!(
                    "rastros {}: {} apariciones, {} avisos, en {}",
                    r.id,
                    r.apariciones,
                    r.avisos,
                    ciego.join(&r.archivo).display()
                );
            }
        }
        return Ok(0);
    }
    let validez = oracle::validez::validar(casos)?;
    let texto = serde_json::to_string_pretty(&validez)?;
    let ruta = casos.join(oracle::validez::ARCHIVO);
    fs::write(&ruta, format!("{texto}\n"))
        .map_err(|e| oracle::ErrorOraculo::Infra(format!("escribir {}: {e}", ruta.display())))?;
    if json {
        println!("{texto}");
    } else {
        imprimir_validez(&validez);
    }
    Ok(0)
}

/// Resumen legible de `validez.json`: una línea por métrica, con su umbral si lo tiene.
fn imprimir_validez(v: &oracle::validez::Validez) {
    let linea = |nombre: &str, m: &oracle::validez::Metrica| {
        let cumple = match (m.cumple, m.umbral) {
            (Some(true), Some(u)) => format!(", cumple (umbral {u:.2})"),
            (Some(false), Some(u)) => format!(", no cumple (umbral {u:.2})"),
            (Some(true), None) => ", cumple".to_string(),
            (Some(false), None) => ", no cumple".to_string(),
            (None, _) => String::new(),
        };
        let motivo = m
            .motivo
            .as_deref()
            .map(|t| format!(" ({t})"))
            .unwrap_or_default();
        println!("{nombre}: {}{cumple}{motivo}", medida(m.valor));
    };
    linea("exactitud de orden", &v.exactitud_orden);
    linea("recall de defectos", &v.recall);
    linea("acuerdo en la decisión", &v.acuerdo_decision);
    linea("spearman del orden", &v.spearman);
    for k in &v.kappa_dimension {
        let motivo = k
            .motivo
            .as_deref()
            .map(|t| format!(" ({t})"))
            .unwrap_or_default();
        println!("kappa {}: {}{motivo}", k.clave, medida(k.valor));
    }
    for aviso in &v.avisos {
        println!("aviso: {aviso}");
    }
}

/// `oraculo informe`: imprime el informe del veredicto, en JSON o en markdown con `--md`. El código de
/// salida sale del estado, como en `evaluar`. Sin informe (sin veredicto), sale con 3 y lo dice en stderr.
fn oraculo_informe(nombre: &str, repo: &Path, md: bool) -> Result<u8, Error> {
    let veredicto = oracle::evaluar_por_nombre(nombre, repo)?;
    let Some(informe) = &veredicto.informe else {
        eprintln!(
            "sin informe: el veredicto no tiene decisión (estado {})",
            etiqueta_estado(veredicto.estado)
        );
        return Ok(oracle::codigo_de_estado(veredicto.estado));
    };
    if md {
        print!("{}", oracle::informe::markdown(nombre, informe));
    } else {
        println!("{}", serde_json::to_string_pretty(informe)?);
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
    let tope = std::time::Duration::from_secs_f64(cfg.colab.minutos_max * 60.0);
    let sesion = oracle::colab::nombre_sesion(&tarea.nombre);
    let corrida = oracle::colab::correr_en_colab(
        &cli,
        &paquete.dir,
        tarea,
        home,
        &sesion,
        &senales,
        tope,
        &oracle::colab::CompiladorPodman,
    )
    .and_then(|resultado| {
        let resumen =
            oracle::pendientes::importar(home, tarea, &resultado.junits, &paquete.commit)?;
        Ok((resultado, resumen))
    });
    // Con la corrida terminada, el paquete (con los tests ocultos) y el junit bajado se borran (A1, B18).
    let limpieza = oracle::pendientes::limpiar_paquete(home, tarea, &paquete.dir, &sesion);
    let (resultado, resumen) = corrida?;
    limpieza
        .map_err(|e| oracle::ErrorOraculo::Infra(format!("borrar el paquete de Colab: {e}")))?;
    let mut avisos = paquete.avisos.clone();
    avisos.extend(resumen.avisos.iter().cloned());
    if json {
        println!(
            "{}",
            serde_json::json!({
                "importados": resumen.importados,
                "ignorados": resumen.ignorados,
                "gpu": resultado.gpu,
                "sesion": resultado.sesion,
                "avisos": avisos,
                "confianza": resumen.confianza,
            })
        );
    } else {
        for aviso in &avisos {
            eprintln!("aviso: {aviso}");
        }
        if let Some(confianza) = &resumen.confianza {
            eprintln!("confianza: {confianza}");
        }
        println!(
            "gpu: {}, sesion: {}, importados: {}, ignorados: {}",
            resultado.gpu, resultado.sesion, resumen.importados, resumen.ignorados
        );
    }
    Ok(())
}

/// `oraculo pendientes --vigilar`: la red de seguridad del runtime anotado. Sin `activo.json`, no
/// toca Colab.
fn oraculo_vigilar(json: bool, home: &Path) -> Result<(), Error> {
    let cfg = config::Config::cargar(&home.join("root.toml"))?;
    let tope = std::time::Duration::from_secs_f64(cfg.colab.minutos_max * 60.0);
    let vigilancia = if oracle::colab::hay_runtime_anotado(home) {
        let cli = oracle::colab::ColabProceso::desde_entorno()?;
        oracle::colab::vigilar(&cli, home, tope)?
    } else {
        oracle::colab::Vigilancia::SinRuntime
    };
    let (estado, causa) = match &vigilancia {
        oracle::colab::Vigilancia::SinRuntime => ("sin_runtime", None),
        oracle::colab::Vigilancia::Vigente => ("vigente", None),
        oracle::colab::Vigilancia::Liberado { causa } => ("liberado", Some(causa.as_str())),
    };
    if json {
        println!(
            "{}",
            serde_json::json!({"vigilancia": estado, "causa": causa})
        );
    } else {
        match causa {
            Some(causa) => println!("runtime liberado: {causa}"),
            None if estado == "vigente" => println!("runtime vigente"),
            None => println!("no hay runtime anotado"),
        }
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
    junits: &[PathBuf],
    colab: bool,
    liberar: bool,
    vigilar: bool,
) -> Result<(), Error> {
    let home = config::root_home();
    if liberar {
        return oraculo_liberar(json, &home);
    }
    if vigilar {
        return oraculo_vigilar(json, &home);
    }
    let (Some(nombre), Some(repo)) = (nombre, repo) else {
        return Err(Error::Uso(
            "oraculo pendientes necesita --tarea y --repo".into(),
        ));
    };
    oracle::pendientes::nombre_de_tarea_valido(nombre).map_err(Error::Uso)?;
    let tarea = Tarea::cargar(nombre)?;
    oracle::pendientes::nombre_de_tarea_valido(&tarea.nombre).map_err(Error::Uso)?;
    // Un solo `pendientes` por tarea a la vez: el turno se toma antes de tocar el paquete o Colab, y
    // se suelta al salir de esta función, también ante error. Si otra corrida lo tiene, sale con 3.
    let _turno = oracle::pendientes::tomar_turno(&home, &tarea.nombre)?;
    if !junits.is_empty() {
        let commit = oracle::commit_de(repo);
        let resumen = oracle::pendientes::importar(&home, &tarea, junits, &commit)?;
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "importados": resumen.importados,
                    "ignorados": resumen.ignorados,
                    "avisos": resumen.avisos,
                    "confianza": resumen.confianza,
                })
            );
        } else {
            for aviso in &resumen.avisos {
                eprintln!("aviso: {aviso}");
            }
            if let Some(confianza) = &resumen.confianza {
                eprintln!("confianza: {confianza}");
            }
            println!(
                "importados: {}, ignorados: {}",
                resumen.importados, resumen.ignorados
            );
        }
        return Ok(());
    }
    let cache = oracle::descarga::cache_de(&home, &tarea.lenguaje, repo, &tarea.imagen);
    let paquete = oracle::pendientes::crear(&home, &tarea, repo, cache.as_deref())?;
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
            juez_max_tokens,
        } => {
            let texto = leer_spec(spec)?;
            let ctx = ContextoRevision::default();
            let tope = juez_max_tokens.unwrap_or(crate::proveedores::MAX_TOKENS_JUEZ);
            let juez = juez_de(*juez_proveedor, juez_modelo.as_deref());
            // El juez de la ficha razona: un modelo fuera de la lista cerrada sale con 2 antes de llamar.
            if let Some(juez) = &juez {
                route::exigir_modelo_que_razona(&juez.eleccion)?;
            }
            // Con juez, los precios y el tope de API salen de root.toml: un archivo inválido es error de uso.
            if juez_proveedor.is_some() {
                config::Config::cargar(&config::root_home().join("root.toml"))?;
            }
            let revision = match juez {
                Some(juez) => ficha::revisar_con_tope(&texto, &ctx, &juez, tope)?,
                None => ficha::revisar(&texto, &ctx)?,
            };
            // Solo con veredicto del juez: la revisión queda en ROOT_HOME/fichas para que `evaluar` la lea.
            ficha::guardar_si_hay_juicio(&config::root_home(), &texto, &revision)?;
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
            rol: Some(Rol::Juzga),
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
                    vigilar,
                },
        } => oraculo_pendientes(
            cli.json,
            tarea.as_deref(),
            repo.as_deref(),
            importar,
            *colab,
            *liberar,
            *vigilar,
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
    registrar_propuestas_forma(&pol)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&pol)?);
    } else {
        imprimir_politica(&pol, cfg.umbral_wilson);
        imprimir_propuestas(&pol.propuestas);
        imprimir_propuestas_forma(&pol.propuestas_forma);
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

/// Un `Gate` por propuesta de forma en `eventos.jsonl`. Sin propuestas, no escribe nada.
fn registrar_propuestas_forma(pol: &route::Politica) -> Result<(), Error> {
    for p in &pol.propuestas_forma {
        let mut extra = serde_json::Map::new();
        extra.insert("tarea".into(), serde_json::json!(p.tarea));
        extra.insert(
            "forma_actual".into(),
            serde_json::to_value(&p.actual.forma)?,
        );
        extra.insert(
            "forma_propuesta".into(),
            serde_json::to_value(&p.propuesta.forma)?,
        );
        let (actual, propuesta) = (
            serde_json::to_value(&p.actual)?,
            serde_json::to_value(&p.propuesta)?,
        );
        extra.insert(
            "evidencia".into(),
            serde_json::json!({ "actual": actual, "propuesta": propuesta }),
        );
        extra.insert(
            "diferencia".into(),
            serde_json::to_value(&p.diferencia_esperada)?,
        );
        observe::registrar(&observe::evento_gate("aprende", "propuesta-forma", extra))
            .map_err(observe::ErrorObserve::from)?;
    }
    Ok(())
}

/// Forma en una línea: roles unidos con `+`, verificador y config.
fn describir_forma(forma: &tipos::Forma) -> String {
    let roles = if forma.roles.is_empty() {
        "sin roles".to_string()
    } else {
        forma
            .roles
            .iter()
            .map(|r| route::politica::nombre_rol(*r))
            .collect::<Vec<_>>()
            .join("+")
    };
    let verificador = if forma.verificador {
        "con verificador"
    } else {
        "sin verificador"
    };
    format!("{roles} ({verificador}, config {})", forma.config)
}

/// Sección «propuestas de forma»: una línea por propuesta, o «sin propuestas».
fn imprimir_propuestas_forma(propuestas: &[route::politica::PropuestaForma]) {
    if propuestas.is_empty() {
        println!("propuestas de forma: sin propuestas");
        return;
    }
    println!("propuestas de forma:");
    for p in propuestas {
        let (act, prop) = (&p.actual, &p.propuesta);
        println!(
            "{}: {} -> {} (éxito {:+.3}, USD {:+.4}; evidencia: actual {}/{}/{:.3}, propuesta {}/{}/{:.3})",
            p.tarea,
            describir_forma(&act.forma),
            describir_forma(&prop.forma),
            p.diferencia_esperada.exito,
            -p.diferencia_esperada.usd,
            act.n,
            act.exitos,
            act.wilson_inf,
            prop.n,
            prop.exitos,
            prop.wilson_inf,
        );
    }
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
            for gate in observe::claude::gates_de_costo_desconocido(&eventos) {
                observe::registrar(&gate).map_err(observe::ErrorObserve::from)?;
            }
            if via == Some(Via::Nube) {
                observe::uso::gate_nube_si_lleno(observe::ahora().0)?;
            }
            imprimir_claude(json, eventos.len(), via);
        }
        ObservaAccion::Costo { usd, motivo, fecha } => {
            let ahora = observe::ahora().0;
            let offset = observe::uso::offset_local();
            let evento =
                observe::evento_costo_manual(*usd, motivo, fecha.as_deref(), ahora, offset)?;
            observe::registrar(&evento).map_err(observe::ErrorObserve::from)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&evento)?);
            } else {
                println!("Registrado un costo de API de {usd} USD: {motivo}");
            }
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
    imprimir_reservas_viejas(&estado.reservas_viejas);
    println!("{}", estado.aviso);
}

/// Una línea por reserva abierta y vieja, con su id, corrida, modelo, antigüedad y monto.
fn imprimir_reservas_viejas(viejas: &[observe::uso::ReservaVieja]) {
    if viejas.is_empty() {
        println!("Reservas abiertas de más de dos veces timeout_juez_s: ninguna.");
        return;
    }
    println!("Reservas abiertas de más de dos veces timeout_juez_s:");
    for r in viejas {
        let id = r.reserva.as_deref().unwrap_or("sin id");
        let modelo = r
            .modelo
            .as_deref()
            .map_or(String::new(), |m| format!(", modelo {m}"));
        println!(
            "  {id}: corrida {}{modelo}, antigüedad {:.1} h, {:.2} USD",
            r.corrida,
            r.antiguedad_s / 3600.0,
            r.usd
        );
    }
    println!(
        "Solo una persona corrige estas reservas con `r00t observa costo`, que solo suma gasto."
    );
}

fn imprimir_registrados(json: bool, cuantos: usize) {
    if json {
        println!("{}", serde_json::json!({"registrados": cuantos}));
    } else {
        println!("Registrados {cuantos} eventos");
    }
}
