//! Biblioteca de Root: evalúa, rutea y observa corridas.

pub mod aprende;
pub mod borrado;
pub mod cli;
pub mod config;
pub mod corre;
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

mod colab_calibra;
mod limpieza;
#[cfg(test)]
mod prueba_ejecutable;

pub use cli::Cli;
pub use error::Error;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cli::{
    ColabAccion, Comando, CorreAccion, FichaAccion, GoalAccion, ObservaAccion, OraculoAccion,
    ProveedorJuez, TrackAccion, ViaRegistro,
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
        } => limpieza::con_vigia(|| oraculo_evaluar(cli.json, tarea, repo)),
        Comando::Oraculo {
            accion: OraculoAccion::Informe { tarea, repo, md },
        } => oraculo_informe(tarea, repo, *md),
        Comando::Oraculo {
            accion: OraculoAccion::Capas { repo, tarea },
        } => oraculo_capas(cli.json, repo, tarea.as_deref()),
        Comando::Oraculo {
            accion:
                OraculoAccion::Calibrar {
                    tarea,
                    dimension,
                    n,
                    semilla,
                    repo,
                    tope_usd,
                },
        } => limpieza::con_vigia(|| {
            oraculo_calibrar(
                cli.json,
                tarea,
                dimension,
                *n,
                *semilla,
                repo.as_deref(),
                *tope_usd,
            )
        }),
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
        Comando::Oraculo {
            accion:
                OraculoAccion::Auditar {
                    repo,
                    salida,
                    tope_usd,
                    gitleaks,
                    incluir,
                    defectos,
                    por_archivo,
                    solo_medir,
                },
        } => {
            let opciones = oracle::auditar::OpcionesAuditar {
                repo: repo.clone(),
                salida: salida.clone(),
                tope_usd: *tope_usd,
                gitleaks: gitleaks.clone(),
                incluir: incluir.clone(),
                defectos: defectos.clone(),
                por_archivo: *por_archivo,
                solo_medir: solo_medir.clone(),
            };
            limpieza::con_vigia(|| oraculo_auditar(cli.json, &opciones))
        }
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
    tope_usd: Option<f64>,
) -> Result<u8, Error> {
    if let Some(usd) = tope_usd
        && (!usd.is_finite() || usd <= 0.0)
    {
        return Err(Error::Uso(format!(
            "--tope-usd debe ser un número finito mayor que 0, no {usd}"
        )));
    }
    let home = config::root_home();
    let presupuesto = tope_usd.map(|usd| oracle::calibra::Presupuesto { usd });
    let calibrada = oracle::calibra::calibrar_tarea_con(
        &home,
        nombre,
        dimension,
        n,
        semilla,
        repo,
        presupuesto.as_ref(),
        &|tarea, ruta, opciones| oracle::evaluar(ruta, tarea, opciones),
    )?;
    for linea in lineas_de_avisos(&calibrada.avisos) {
        eprintln!("{linea}");
    }
    let fila = calibrada.fila;
    if json {
        println!("{}", serde_json::to_string_pretty(&fila)?);
    } else if let Some(linea) = linea_sin_medir(&fila) {
        println!("{linea}");
        if fila.base_sin_veredicto.is_some()
            && let Some(ruta) = &calibrada.casos
        {
            println!("casos: {}", ruta.display());
        }
    } else {
        println!(
            "{}/{}: sensibilidad {}, {}, {}, {}, exactitud {}, casos {} (defectos {}, limpios {}, no aplicables {}, reemplazos {})",
            fila.tarea,
            fila.dimension.clave(),
            medida(fila.sensibilidad),
            oracle::calibra::texto_especificidad(&fila),
            oracle::calibra::texto_piso(&fila),
            oracle::calibra::texto_inyecciones(&fila),
            medida(fila.exactitud_balanceada),
            fila.n,
            fila.defectos,
            fila.limpios,
            fila.no_aplicable,
            fila.reemplazos
        );
        let calibraciones = oracle::calibra::Calibraciones::cargar(&home)?;
        let version = calibraciones.version_en(
            &home,
            &fila.tarea,
            fila.tarea_hash.as_deref().unwrap_or_default(),
            fila.dimension,
        );
        let juez = oracle::calibra::tiene_items_de_juez(fila.dimension);
        println!("{}", texto_version(&version, juez));
        if let Some(ruta) = &calibrada.casos {
            println!("casos: {}", ruta.display());
        }
    }
    Ok(0)
}

/// Línea humana de una fila que no mide la dimensión. `None` si la fila está medida.
/// Precedencia: `base_sin_veredicto`, luego `inestable`, luego `no_aplica`.
fn linea_sin_medir(fila: &oracle::calibra::Fila) -> Option<String> {
    let (tarea, dimension) = (&fila.tarea, fila.dimension.clave());
    if let Some(motivo) = &fila.base_sin_veredicto {
        return Some(format!("{tarea}/{dimension}: base sin veredicto: {motivo}"));
    }
    let motivo = fila.inestable.as_ref().or(fila.no_aplica.as_ref())?;
    Some(format!("{tarea}/{dimension}: {motivo}"))
}

/// Una línea `aviso: <texto>` por aviso, en el mismo orden.
fn lineas_de_avisos(avisos: &[String]) -> Vec<String> {
    avisos
        .iter()
        .map(|aviso| format!("aviso: {aviso}"))
        .collect()
}

/// Una métrica con dos decimales, o `sin medir` si no tiene casos de una clase.
fn medida(valor: Option<f32>) -> String {
    valor.map_or_else(|| "sin medir".to_string(), |v| format!("{v:.2}"))
}

/// Casos acertados más que faltan para que la cota de Wilson de `aciertos` de `total` llegue a la cota mínima.
/// Cada caso nuevo es un acierto. Da 0 si la cota ya llega.
fn casos_que_faltan(aciertos: u32, total: u32) -> u32 {
    (0..)
        .find(|k| {
            oracle::calibra::wilson_inferior(aciertos + k, total + k)
                >= oracle::calibra::COTA_WILSON_MINIMA
        })
        .unwrap_or(0)
}

/// Una clase de la versión: su cota de Wilson y, si no llega a la cota mínima, los casos que faltan.
fn texto_cota(nombre: &str, aciertos: u32, total: u32) -> String {
    let cota = oracle::calibra::wilson_inferior(aciertos, total);
    let faltan = casos_que_faltan(aciertos, total);
    if faltan == 0 {
        return format!("{nombre} Wilson {cota:.2}");
    }
    format!(
        "{nombre} Wilson {cota:.2} (faltan {faltan} casos sin fallos para {})",
        oracle::calibra::COTA_WILSON_MINIMA
    )
}

/// Texto de la versión de Root para `calibrar`. Un juez muestra la cota de Wilson de sensibilidad y de
/// especificidad con los casos que faltan. Un oráculo muestra si hay un defecto detectado y si los limpios dieron bien.
fn texto_version(version: &oracle::calibra::Version, juez: bool) -> String {
    if juez {
        return format!(
            "versión: {}, {}",
            texto_cota("sensibilidad", version.detectados, version.defectos),
            texto_cota(
                "especificidad",
                version.limpios_bien,
                version.limpios_con_veredicto
            )
        );
    }
    let defectos = match version.detectados {
        0 => "sin defecto detectado".to_string(),
        1 => "1 defecto detectado".to_string(),
        n => format!("{n} defectos detectados"),
    };
    let mal =
        version.limpios_con_veredicto - version.limpios_bien.min(version.limpios_con_veredicto);
    let limpios = match mal {
        0 => "limpios bien".to_string(),
        1 => "1 limpio mal".to_string(),
        n => format!("{n} limpios mal"),
    };
    format!("versión: {defectos}, {limpios}")
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
        let home = config::root_home();
        let preparado = oracle::validez::preparar_ciego(casos, fuentes, semilla, &home)?;
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
            // Los conteos de rastros no salen aquí: Opus los lee desde este archivo (desempate 18).
            println!(
                "revisión de rastros: {}",
                oracle::validez::ruta_rastros_en(&home).display()
            );
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

/// `oraculo capas`: sin `--tarea`, propone las capas del repo. Con `--tarea`, verifica las capas de la tarea:
/// sale con 1 si no pasan. Las copias de prueba van a `ROOT_HOME/tmp` y se borran al terminar.
fn oraculo_capas(json: bool, repo: &Path, tarea: Option<&str>) -> Result<u8, Error> {
    let Some(nombre) = tarea else {
        let propuesta = oracle::capas::proponer_capas(repo).map_err(Error::Uso)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&propuesta)?);
        } else {
            println!("{}", oracle::capas::linea_toml(&propuesta.capas));
        }
        return Ok(0);
    };
    let tarea = Tarea::cargar(nombre)?;
    let capas = tarea.arquitectura.map(|a| a.capas).unwrap_or_default();
    let trabajo = config::root_home().join("tmp");
    let veredicto = oracle::capas::verificar_capas(repo, &capas, &trabajo);
    if json {
        println!("{}", serde_json::to_string_pretty(&veredicto)?);
    } else if veredicto.fallas.is_empty() {
        println!("{nombre}: las capas pasan la verificación");
    } else {
        for falla in &veredicto.fallas {
            println!("falla: {falla}");
        }
    }
    Ok(if veredicto.fallas.is_empty() { 0 } else { 1 })
}

/// `oraculo auditar`: valida `--tope-usd`, audita el repo y muestra un resumen. Los errores de uso salen con 2.
/// Sale con 3 si quedó incompleta, con 0 si no. El JSON de `--salida` se escribe igual.
fn oraculo_auditar(json: bool, opciones: &oracle::auditar::OpcionesAuditar) -> Result<u8, Error> {
    if let Some(usd) = opciones.tope_usd
        && (!usd.is_finite() || usd <= 0.0)
    {
        return Err(Error::Uso(format!(
            "--tope-usd debe ser un número finito mayor que 0, no {usd}"
        )));
    }
    let auditoria = oracle::auditar::auditar(&config::root_home(), opciones)?;
    terminar_auditoria(json, &auditoria)
}

/// Imprime la auditoría y devuelve el código de salida: 3 si quedó incompleta, 0 si no.
fn terminar_auditoria(json: bool, auditoria: &oracle::auditar::Auditoria) -> Result<u8, Error> {
    let valor = serde_json::to_value(auditoria)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&valor)?);
    } else {
        println!("{}", resumen_auditoria(&valor));
    }
    Ok(if auditoria.incompleta { 3 } else { 0 })
}

/// Una línea con los conteos de la auditoría. Lee los campos por nombre desde su JSON.
fn resumen_auditoria(valor: &serde_json::Value) -> String {
    let cuenta = |campo: &str| valor[campo].as_array().map_or(0, Vec::len);
    let costo = valor["costo_usd"].as_f64().unwrap_or(0.0);
    let llamadas = valor["llamadas"].as_u64().unwrap_or(0);
    let incompleta = valor["incompleta"].as_bool().unwrap_or(false);
    let mut linea = format!(
        "auditoría: hallazgos {}, descartados {}, llamadas {llamadas}, costo ${costo:.4}, incompleta: {}",
        cuenta("hallazgos"),
        cuenta("descartados"),
        if incompleta { "sí" } else { "no" }
    );
    let medicion = &valor["medicion"];
    if !medicion.is_null() {
        let encontrados = medicion["encontrados"].as_u64().unwrap_or(0);
        let leidos = medicion["casos_leidos"].as_u64().unwrap_or(0);
        let casos = medicion["casos"].as_u64().unwrap_or(0);
        linea.push_str(&format!(
            ", encontrados {encontrados} de {leidos} casos leídos ({casos} en total)"
        ));
    }
    linea
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

/// `colab empaquetar`: arma el paquete para Colab e imprime dónde quedó.
/// `n` y `semilla` llegan de la CLI, con sus valores por defecto. Siempre va por `empaquetar_cli_con`.
fn colab_empaquetar(
    json: bool,
    pedido: colab_calibra::PedidoEmpaquetar,
    n: u32,
    semilla: u64,
) -> Result<(), Error> {
    colab_empaquetar_con(json, pedido, n, semilla, &colab_calibra::empaquetar_cli_con)
}

/// Igual que `colab_empaquetar`, pero la función que arma el paquete se inyecta para testear.
fn colab_empaquetar_con(
    json: bool,
    pedido: colab_calibra::PedidoEmpaquetar,
    n: u32,
    semilla: u64,
    armar: &dyn Fn(
        colab_calibra::PedidoEmpaquetar,
        u32,
        u64,
    ) -> Result<colab_calibra::Resumen, oracle::ErrorOraculo>,
) -> Result<(), Error> {
    let resumen = armar(pedido, n, semilla)?;
    let salida = resumen.salida.display().to_string();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "salida": salida,
                "archivos": resumen.archivos,
                "trabajos": resumen.trabajos,
                "sha256": resumen.sha256,
            })
        );
    } else {
        println!("{salida}: {} archivos", resumen.archivos);
        println!("{}", linea_sha(&resumen));
    }
    Ok(())
}

/// Línea que `colab empaquetar` imprime con el SHA-256 del tar.
fn linea_sha(resumen: &colab_calibra::Resumen) -> String {
    format!("ROOT_SHA_PAQUETE={}", resumen.sha256)
}

/// `colab calibrar`: corre cada paquete en su runtime, importa cada vuelta e imprime el resumen de todos.
/// Si algún runtime falla, el comando sale con error después de imprimir todo.
fn colab_calibrar(
    json: bool,
    paquetes: Vec<PathBuf>,
    raiz_root: Option<PathBuf>,
    sin_stop: bool,
) -> Result<(), Error> {
    let resumen = colab_calibra::calibrar_cli(paquetes, raiz_root, sin_stop)?;
    let fallidos = resumen
        .runtimes
        .iter()
        .filter(|r| r.resultado.is_err())
        .count();
    if json {
        let runtimes: Vec<serde_json::Value> =
            resumen.runtimes.iter().map(runtime_a_json).collect();
        println!(
            "{}",
            serde_json::json!({"runtimes": runtimes, "fallidos": fallidos})
        );
    } else {
        for runtime in &resumen.runtimes {
            imprimir_runtime(runtime);
        }
    }
    if fallidos > 0 {
        return Err(oracle::ErrorOraculo::Colab(format!(
            "{fallidos} de {} runtimes fallaron",
            resumen.runtimes.len()
        ))
        .into());
    }
    Ok(())
}

/// Un runtime como objeto JSON: su paquete, su sesión, si terminó bien y su resumen o su error.
fn runtime_a_json(runtime: &colab_calibra::ResultadoRuntime) -> serde_json::Value {
    let (resumen, error) = match &runtime.resultado {
        Ok(resumen) => (
            serde_json::to_value(resumen).unwrap_or(serde_json::Value::Null),
            serde_json::Value::Null,
        ),
        Err(motivo) => (serde_json::Value::Null, serde_json::json!(motivo)),
    };
    serde_json::json!({
        "paquete": runtime.paquete.display().to_string(),
        "sesion": runtime.sesion,
        "ok": runtime.resultado.is_ok(),
        "resumen": resumen,
        "error": error,
        "codigos_de_trabajos": runtime.codigos_de_trabajos,
        "abierto": runtime.abierto.as_ref().map(|a| serde_json::json!({
            "sesion": a.sesion,
            "carpeta": a.carpeta,
            "trabajo_cmd": a.trabajo_cmd,
        })),
    })
}

/// Texto de un runtime: la importación o el motivo del fallo, y el código de cada trabajo.
fn imprimir_runtime(runtime: &colab_calibra::ResultadoRuntime) {
    println!("{} ({})", runtime.paquete.display(), runtime.sesion);
    match &runtime.resultado {
        Ok(resumen) => {
            println!(
                "  entraron {} filas; no entraron {}",
                resumen.entraron,
                resumen.rechazadas.len()
            );
            for rechazada in &resumen.rechazadas {
                println!(
                    "  no entra {} {}: {}",
                    rechazada.tarea, rechazada.dimension, rechazada.motivo
                );
            }
            println!("  casos copiados: {}", resumen.casos_copiados);
            if let Some(respaldo) = &resumen.respaldo {
                println!("  respaldo: {}", respaldo.display());
            }
            for aviso in &resumen.avisos {
                println!("  {aviso}");
            }
        }
        Err(motivo) => println!("  falló: {motivo}"),
    }
    for codigo in &runtime.codigos_de_trabajos {
        println!(
            "  trabajo {} {}: código {}",
            codigo.tarea, codigo.dimension, codigo.codigo
        );
    }
    // Sin sangría: las líneas ROOT_ se copian y se buscan con grep.
    for linea in colab_calibra::lineas_runtime_abierto(runtime) {
        println!("{linea}");
    }
}

/// `colab importar`: revisa el paquete de vuelta de Colab, importa lo que pasa e imprime el resumen.
fn colab_importar(json: bool, pedido: colab_calibra::PedidoImportar) -> Result<(), Error> {
    let resumen = colab_calibra::importar_cli(pedido)?;
    if json {
        println!("{}", serde_json::to_string(&resumen)?);
        return Ok(());
    }
    println!(
        "entraron {} filas; no entraron {}",
        resumen.entraron,
        resumen.rechazadas.len()
    );
    for rechazada in &resumen.rechazadas {
        println!(
            "no entra {} {}: {}",
            rechazada.tarea, rechazada.dimension, rechazada.motivo
        );
    }
    println!("casos copiados: {}", resumen.casos_copiados);
    if let Some(respaldo) = &resumen.respaldo {
        println!("respaldo: {}", respaldo.display());
    }
    for aviso in &resumen.avisos {
        println!("{aviso}");
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
        // La identidad de la versión es el hash del contenido, no el HEAD de git (Tarea 54, H2).
        let identidad = oracle::identidad_de(repo, &tarea)?;
        let resumen = oracle::pendientes::importar(&home, &tarea, junits, &identidad)?;
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
    let cache = oracle::descarga::cache_de_con_error(&home, &tarea.lenguaje, repo, &tarea.imagen)?;
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
            solo_deterministas,
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
                None if *solo_deterministas => ficha::revisar_solo_deterministas(&texto, &ctx)?,
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
    if revision.juez.starts_with("sin_veredicto") || revision.juez == ficha::JUEZ_NO_PEDIDO {
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
        Comando::Colab {
            accion:
                ColabAccion::Empaquetar {
                    binario,
                    trabajos,
                    repo,
                    rustup_sha256,
                    salida,
                    raiz_root,
                    n,
                    semilla,
                    juez,
                },
        } => colab_empaquetar(
            cli.json,
            colab_calibra::PedidoEmpaquetar {
                binario: binario.clone(),
                trabajos: trabajos.clone(),
                repos: repo.clone(),
                rustup_sha256: rustup_sha256.clone(),
                salida: salida.clone(),
                raiz_root: raiz_root.clone(),
                juez: *juez,
            },
            *n,
            *semilla,
        ),
        Comando::Colab {
            accion:
                ColabAccion::Importar {
                    archivo,
                    raiz_root,
                    sha,
                    ida,
                },
        } => colab_importar(
            cli.json,
            colab_calibra::PedidoImportar {
                archivo: archivo.clone(),
                raiz_root: raiz_root.clone(),
                sha: sha.clone(),
                ida: ida.clone(),
            },
        ),
        Comando::Colab {
            accion:
                ColabAccion::Calibrar {
                    paquete,
                    raiz_root,
                    sin_stop,
                },
        } => colab_calibrar(cli.json, paquete.clone(), raiz_root.clone(), *sin_stop),
        Comando::Observa { accion } => observa(cli.json, accion),
        Comando::Corre {
            accion: Some(CorreAccion::RevisarLectura { tarea, transcript }),
            ..
        } => revisar_lectura_accion(cli.json, tarea, transcript),
        Comando::Corre {
            tarea: Some(tarea),
            config: Some(config),
            reanuda,
            accion: None,
        } => corre_accion(cli.json, tarea, config, reanuda.as_deref()),
        Comando::Corre { .. } => Err(Error::Uso(
            "corre exige --tarea y --config, o el subcomando revisar-lectura".into(),
        )),
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

/// `corre`: arma y corre una tarea con una configuración, con el `ROOT_HOME`, el `$HOME` y el repo del
/// proceso. Imprime la fila: con `--json`, la fila completa; sin él, un resumen corto.
fn corre_accion(json: bool, tarea: &str, config: &str, reanuda: Option<&str>) -> Result<(), Error> {
    let home = config::root_home();
    let home_usuario = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
        Error::Uso("HOME no está definida: corre la necesita para la carpeta de corridas".into())
    })?;
    let repo_root = std::env::current_dir().map_err(|fuente| corre::ErrorCorre::Io {
        contexto: "leer el directorio actual".into(),
        fuente,
    })?;
    let entorno: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(clave, valor)| Some((clave.into_string().ok()?, valor.into_string().ok()?)))
        .collect();
    let ejecutor = corre::EjecutorContenedorBuilder {
        imagen: corre::contenedor::IMAGEN_BUILDER.to_string(),
        credencial: corre::contenedor::credencial_de(&home_usuario),
        home: home.clone(),
        home_usuario: home_usuario.clone(),
        timeout: corre::contenedor::TIMEOUT_BUILDER,
    };
    let deps = corre::DepsCorre {
        home: &home,
        home_usuario: &home_usuario,
        repo_root: &repo_root,
        ejecutor: &ejecutor,
        evaluar: &oracle::evaluar,
        entorno: &entorno,
    };
    let fila = match reanuda {
        None => corre::correr(&deps, tarea, config)?,
        Some(id) => corre::correr_reanudando(&deps, tarea, config, id)?,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&fila)?);
    } else {
        let invalida = if fila.resumen.invalida {
            ", corrida inválida: leyó el oráculo"
        } else {
            ""
        };
        println!(
            "corrida {}: {} con {}: veredicto {}, {} turnos, {:.4} USD de cupo{invalida}",
            fila.id,
            fila.tarea,
            fila.config,
            etiqueta_estado(fila.veredicto.estado),
            fila.resumen.turnos,
            fila.resumen.usd_cupo,
        );
    }
    Ok(())
}

/// `corre revisar-lectura`: cuenta las lecturas del oráculo en el transcript. Solo imprime conteos.
fn revisar_lectura_accion(json: bool, tarea: &str, transcript: &Path) -> Result<(), Error> {
    let home_usuario = std::env::var_os("HOME").map(PathBuf::from);
    let conteo = revisar_lectura(
        &config::root_home(),
        home_usuario.as_deref(),
        tarea,
        transcript,
    )?;
    if json {
        let objeto = json_revision_lectura(tarea, &conteo);
        println!("{}", serde_json::to_string_pretty(&objeto)?);
    } else {
        println!("{}", texto_revision_lectura(tarea, &conteo));
    }
    Ok(())
}

/// Cuenta las lecturas del oráculo de `tarea` en `transcript`, con `home` como ROOT_HOME. Es error de
/// uso si el transcript no existe o si la tarea no tiene `oraculos/<tarea>/oculto/`. No escribe nada.
fn revisar_lectura(
    home: &Path,
    home_usuario: Option<&Path>,
    tarea: &str,
    transcript: &Path,
) -> Result<observe::transcript::ConteoLecturas, Error> {
    oracle::pendientes::nombre_de_tarea_valido(tarea).map_err(Error::Uso)?;
    if !transcript.is_file() {
        return Err(Error::Uso(format!(
            "no existe el transcript: {}",
            transcript.display()
        )));
    }
    let oculto = home.join("oraculos").join(tarea).join("oculto");
    if !oculto.is_dir() {
        return Err(Error::Uso(format!(
            "la tarea {tarea} no tiene oráculo oculto: falta {}",
            oculto.display()
        )));
    }
    let huellas = observe::transcript::HuellasOraculo::armar(home, home_usuario, tarea)?;
    Ok(observe::transcript::contar_lecturas(transcript, &huellas)?)
}

/// Texto de `revisar-lectura`: los conteos y una línea de veredicto. Nunca la cita ni la huella.
fn texto_revision_lectura(tarea: &str, conteo: &observe::transcript::ConteoLecturas) -> String {
    let cuenta = format!(
        "tarea {tarea}: huella {}, nombre {}, ruta {}",
        conteo.huella, conteo.nombre, conteo.ruta
    );
    let veredicto = if conteo.marcado() {
        "marcado"
    } else {
        "sin marcas"
    };
    format!("{cuenta}\n{veredicto}")
}

/// JSON de `revisar-lectura`: solo conteos y el veredicto.
fn json_revision_lectura(
    tarea: &str,
    conteo: &observe::transcript::ConteoLecturas,
) -> serde_json::Value {
    serde_json::json!({
        "tarea": tarea,
        "huella": conteo.huella,
        "nombre": conteo.nombre,
        "ruta": conteo.ruta,
        "marcado": conteo.marcado(),
    })
}

/// Subcomandos `observa`: registran eventos y muestran el resumen de una corrida.
fn observa(json: bool, accion: &ObservaAccion) -> Result<(), Error> {
    match accion {
        ObservaAccion::Claude {
            archivo,
            corrida,
            via,
        } => {
            let via = via.map(|v| match v {
                ViaRegistro::Cupo => Via::Cupo,
                ViaRegistro::Nube => Via::Nube,
            });
            let registro = observe::claude::registrar_sesion_claude(
                archivo,
                corrida,
                via,
                &observe::ruta_eventos(),
            )?;
            for id in &registro.ya_registradas {
                eprintln!("aviso: la sesión {id} ya está en el log; no se cuenta de nuevo");
            }
            if via == Some(Via::Nube) {
                observe::uso::gate_nube_si_lleno(observe::ahora().0)?;
            }
            imprimir_claude(json, registro.eventos, via);
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use serde_json::json;

    fn version_de(
        defectos: u32,
        detectados: u32,
        limpios: u32,
        bien: u32,
    ) -> oracle::calibra::Version {
        oracle::calibra::Version {
            defectos,
            detectados,
            limpios_con_veredicto: limpios,
            limpios_bien: bien,
            limpios,
            limpios_sin_veredicto: 0,
            defectos_sin_veredicto: 0,
            tareas: vec!["t".into()],
        }
    }

    fn fila_sin_medir(tarea: &str, dimension: &str) -> oracle::calibra::Fila {
        serde_json::from_value(serde_json::json!({
            "tarea": tarea,
            "dimension": dimension,
            "n": 0,
            "semilla": 1,
            "ts": 1.0
        }))
        .expect("fila mínima")
    }

    #[test]
    fn linea_sin_medir_base_sin_veredicto_dice_el_motivo() {
        let motivo =
            "juicio 1 de la base sin veredicto: cobertura 6 de 10 de peso, menor que dos tercios";
        let fila = oracle::calibra::Fila {
            base_sin_veredicto: Some(motivo.into()),
            ..fila_sin_medir("s2-t1", "arquitectura")
        };
        assert_eq!(
            linea_sin_medir(&fila),
            Some(format!("s2-t1/arquitectura: base sin veredicto: {motivo}"))
        );
    }

    #[test]
    fn linea_sin_medir_inestable_usa_el_formato_de_siempre() {
        let fila = oracle::calibra::Fila {
            inestable: Some("ítems variaron".into()),
            ..fila_sin_medir("s2-t1", "arquitectura")
        };
        assert_eq!(
            linea_sin_medir(&fila),
            Some("s2-t1/arquitectura: ítems variaron".to_string())
        );
    }

    #[test]
    fn linea_sin_medir_no_aplica_usa_el_formato_de_siempre() {
        let fila = oracle::calibra::Fila {
            no_aplica: Some("sin Dockerfile".into()),
            ..fila_sin_medir("s2-t1", "arquitectura")
        };
        assert_eq!(
            linea_sin_medir(&fila),
            Some("s2-t1/arquitectura: sin Dockerfile".to_string())
        );
    }

    #[test]
    fn linea_sin_medir_fila_medida_da_none() {
        let fila = fila_sin_medir("s2-t1", "arquitectura");
        assert_eq!(linea_sin_medir(&fila), None);
    }

    #[test]
    fn texto_version_de_juez_con_cinco_de_cinco_dice_la_cota_y_los_casos_que_faltan() {
        let texto = texto_version(&version_de(5, 5, 5, 5), true);
        assert!(texto.contains("sensibilidad Wilson 0.57"), "{texto}");
        assert!(texto.contains("especificidad Wilson 0.57"), "{texto}");
        assert!(
            texto.contains("faltan 11 casos sin fallos para 0.8"),
            "{texto}"
        );
    }

    #[test]
    fn texto_version_de_juez_que_ya_llega_no_pide_casos() {
        let texto = texto_version(&version_de(16, 16, 16, 16), true);
        assert!(texto.contains("sensibilidad Wilson 0.81"), "{texto}");
        assert!(!texto.contains("faltan"), "{texto}");
    }

    #[test]
    fn texto_version_de_oraculo_dice_defecto_detectado_y_limpios_bien() {
        let texto = texto_version(&version_de(1, 1, 3, 3), false);
        assert!(
            texto.contains("1 defecto detectado, limpios bien"),
            "{texto}"
        );
    }

    #[test]
    fn texto_version_de_oraculo_sin_defecto_o_con_limpio_mal_lo_dice() {
        let texto = texto_version(&version_de(2, 0, 3, 2), false);
        assert!(texto.contains("sin defecto detectado"), "{texto}");
        assert!(texto.contains("1 limpio mal"), "{texto}");
    }

    #[test]
    fn lineas_de_avisos_escribe_una_linea_por_aviso_en_orden() {
        let avisos = vec!["primero".to_string(), "segundo".to_string()];
        assert_eq!(
            lineas_de_avisos(&avisos),
            vec!["aviso: primero".to_string(), "aviso: segundo".to_string()]
        );
    }

    #[test]
    fn lineas_de_avisos_sin_avisos_es_vacio() {
        assert!(lineas_de_avisos(&[]).is_empty());
    }

    #[test]
    fn oraculo_auditar_tope_usd_no_positivo_sale_con_uso_sin_tocar_nada() {
        for valor in ["0", "-1"] {
            let cli = Cli::try_parse_from([
                "r00t",
                "oraculo",
                "auditar",
                "--repo",
                "/repo-que-no-existe",
                "--salida",
                "/salida-que-no-existe.json",
                "--tope-usd",
                valor,
                "--gitleaks",
                "/gitleaks-que-no-existe.json",
            ])
            .expect("los flags parsean");
            let error = ejecutar(&cli).expect_err("el tope no positivo es error de uso");
            assert!(
                matches!(error, Error::Uso(_)),
                "--tope-usd {valor}: {error:?}"
            );
            assert_eq!(error.codigo_de_salida(), 2, "--tope-usd {valor}");
            assert!(
                error.to_string().contains("finito mayor que 0"),
                "--tope-usd {valor}: {error}"
            );
        }
    }

    /// Auditoría mínima para probar la salida. Solo cambian `incompleta` y `medicion`.
    fn auditoria_de(
        incompleta: bool,
        medicion: Option<oracle::auditar::Medicion>,
    ) -> oracle::auditar::Auditoria {
        oracle::auditar::Auditoria {
            hash_prompt: String::new(),
            modelo: String::new(),
            archivos: Vec::new(),
            llamadas: 0,
            costo_usd: 0.0,
            hallazgos: Vec::new(),
            descartados: Vec::new(),
            incompleta,
            sin_leer: Vec::new(),
            avisos: Vec::new(),
            medicion,
            lineas_leidas: 0,
            candidatos_por_1000_lineas: None,
        }
    }

    #[test]
    fn auditoria_incompleta_sale_con_3() {
        let auditoria = auditoria_de(true, None);
        let codigo = terminar_auditoria(true, &auditoria).expect("la salida no falla");
        assert_eq!(codigo, 3, "una auditoría incompleta sale con 3");
    }

    #[test]
    fn auditoria_completa_sale_con_0() {
        let auditoria = auditoria_de(false, None);
        let codigo = terminar_auditoria(true, &auditoria).expect("la salida no falla");
        assert_eq!(codigo, 0, "una auditoría completa sale con 0");
    }

    #[test]
    fn el_resumen_cuenta_encontrados_sobre_casos_leidos() {
        let valor = serde_json::json!({
            "hallazgos": [],
            "descartados": [],
            "llamadas": 1,
            "costo_usd": 0.5,
            "incompleta": false,
            "medicion": {"encontrados": 1, "casos": 4, "casos_leidos": 2, "detalle": []}
        });
        let resumen = resumen_auditoria(&valor);
        assert!(
            resumen.contains("encontrados 1 de 2 casos leídos (4 en total)"),
            "resumen: {resumen}"
        );
    }

    #[test]
    fn el_resumen_sin_medicion_no_menciona_encontrados() {
        let valor = serde_json::json!({
            "hallazgos": [],
            "descartados": [],
            "llamadas": 0,
            "costo_usd": 0.0,
            "incompleta": false,
            "medicion": null
        });
        let resumen = resumen_auditoria(&valor);
        assert!(!resumen.contains("encontrados"), "resumen: {resumen}");
    }

    /// Pedido mínimo de `colab empaquetar` para los tests de `colab_empaquetar_con`.
    fn pedido_empaquetar() -> colab_calibra::PedidoEmpaquetar {
        colab_calibra::PedidoEmpaquetar {
            binario: PathBuf::from("b"),
            trabajos: vec!["t:d".into()],
            repos: vec!["t=r".into()],
            rustup_sha256: "0".repeat(64),
            salida: PathBuf::from("s.tar.gz"),
            raiz_root: None,
            juez: false,
        }
    }

    #[test]
    fn linea_sha_da_la_linea_del_sha_del_paquete() {
        let resumen = colab_calibra::Resumen {
            salida: PathBuf::from("s.tar.gz"),
            archivos: 1,
            trabajos: 1,
            sha256: "ab12".into(),
        };
        assert_eq!(linea_sha(&resumen), "ROOT_SHA_PAQUETE=ab12");
    }

    #[test]
    fn colab_empaquetar_pasa_n_y_semilla_a_la_funcion() {
        let recibidos = std::cell::RefCell::new(Vec::new());
        let registrar = |_pedido: colab_calibra::PedidoEmpaquetar, n: u32, semilla: u64| {
            recibidos.borrow_mut().push((n, semilla));
            Ok(colab_calibra::Resumen {
                salida: PathBuf::from("s.tar.gz"),
                archivos: 1,
                trabajos: 1,
                sha256: String::new(),
            })
        };
        colab_empaquetar_con(false, pedido_empaquetar(), 8, 7, &registrar)
            .expect("empaquetar con n y semilla explícitos");
        assert_eq!(*recibidos.borrow(), vec![(8, 7)]);
    }

    #[test]
    fn colab_empaquetar_con_defectos_pasa_4_y_42() {
        let recibidos = std::cell::RefCell::new(Vec::new());
        let registrar = |_pedido: colab_calibra::PedidoEmpaquetar, n: u32, semilla: u64| {
            recibidos.borrow_mut().push((n, semilla));
            Ok(colab_calibra::Resumen {
                salida: PathBuf::from("s.tar.gz"),
                archivos: 1,
                trabajos: 1,
                sha256: String::new(),
            })
        };
        colab_empaquetar_con(
            false,
            pedido_empaquetar(),
            colab_calibra::N_POR_DEFECTO,
            colab_calibra::SEMILLA_POR_DEFECTO,
            &registrar,
        )
        .expect("empaquetar con defectos");
        assert_eq!(*recibidos.borrow(), vec![(4, 42)]);
    }

    #[test]
    fn colab_empaquetar_sin_banderas_pasa_4_y_42_desde_la_cli() {
        // Parsea la CLI sin `--n` ni `--semilla` y sigue los valores hasta el armador.
        let args: Vec<String> = [
            "r00t",
            "colab",
            "empaquetar",
            "--binario",
            "b",
            "--trabajos",
            "t:d",
            "--repo",
            "t=r",
            "--rustup-sha256",
            &"0".repeat(64),
            "--salida",
            "s.tar.gz",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let cli = Cli::try_parse_from(args).expect("colab empaquetar sin banderas parsea");
        let (n, semilla) = match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Empaquetar { n, semilla, .. },
            } => (n, semilla),
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        };
        let recibidos = std::cell::RefCell::new(Vec::new());
        let registrar = |_pedido: colab_calibra::PedidoEmpaquetar, n: u32, semilla: u64| {
            recibidos.borrow_mut().push((n, semilla));
            Ok(colab_calibra::Resumen {
                salida: PathBuf::from("s.tar.gz"),
                archivos: 1,
                trabajos: 1,
                sha256: String::new(),
            })
        };
        colab_empaquetar_con(false, pedido_empaquetar(), n, semilla, &registrar)
            .expect("empaquetar sin banderas");
        assert_eq!(*recibidos.borrow(), vec![(4, 42)]);
    }

    /// Línea única del oráculo oculto. Nunca debe salir en la salida de `revisar-lectura`.
    const LINEA_OCULTA_LIB: &str = "assert sincronizar(tmp, \"clave-secreta-42\") == [1, 2, 3]";

    /// `ROOT_HOME` de prueba con `oraculos/x/oculto/t.py` que trae `LINEA_OCULTA_LIB`.
    fn raiz_con_oculto_lib(tmp: &tempfile::TempDir) -> PathBuf {
        let root = tmp.path().canonicalize().expect("canónica").join("root");
        let oculto = root.join("oraculos/x/oculto");
        fs::create_dir_all(&oculto).expect("carpeta oculta");
        fs::write(
            oculto.join("t.py"),
            format!("def test_indice(tmp):\n    {LINEA_OCULTA_LIB}\n"),
        )
        .expect("escribe el oculto");
        root
    }

    /// Transcript con una llamada `Bash` cuya salida es `salida`. Devuelve su ruta.
    fn transcript_revision(tmp: &tempfile::TempDir, salida: &str) -> PathBuf {
        let ruta = tmp.path().join("transcript.jsonl");
        let llamada = json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "r1", "name": "Bash", "input": {"command": "cat oculto"}}]}});
        let resultado = json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "r1", "content": salida}]}});
        fs::write(&ruta, format!("{llamada}\n{resultado}\n")).expect("escribe el transcript");
        ruta
    }

    /// Lista (ruta, bytes) de todos los archivos bajo `raiz`, ordenada.
    fn archivos_con_bytes(raiz: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn recorre(dir: &Path, salida: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entrada in fs::read_dir(dir).expect("lee la carpeta") {
                let ruta = entrada.expect("entrada").path();
                if ruta.is_dir() {
                    recorre(&ruta, salida);
                } else {
                    let bytes = fs::read(&ruta).expect("lee el archivo");
                    salida.push((ruta, bytes));
                }
            }
        }
        let mut salida = Vec::new();
        recorre(raiz, &mut salida);
        salida.sort();
        salida
    }

    #[test]
    fn revisar_lectura_con_huella_real_da_una_lectura_y_no_imprime_la_linea() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let root = raiz_con_oculto_lib(&tmp);
        let ruta = transcript_revision(&tmp, &format!("salida: {LINEA_OCULTA_LIB}"));
        let conteo = revisar_lectura(&root, None, "x", &ruta).expect("revisa");
        assert_eq!(conteo.huella, 1, "{conteo:?}");
        assert!(conteo.marcado());
        let texto = texto_revision_lectura("x", &conteo);
        assert!(!texto.contains("clave-secreta-42"), "{texto}");
        assert!(texto.contains("marcado"), "{texto}");
        let json = json_revision_lectura("x", &conteo).to_string();
        assert!(!json.contains("clave-secreta-42"), "{json}");
        assert!(json.contains("\"huella\":1"), "{json}");
    }

    #[test]
    fn revisar_lectura_transcript_limpio_da_cero_y_sin_marcas() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let root = raiz_con_oculto_lib(&tmp);
        let ruta = transcript_revision(&tmp, "nada que ver con el oráculo");
        let conteo = revisar_lectura(&root, None, "x", &ruta).expect("revisa");
        assert_eq!(conteo, observe::transcript::ConteoLecturas::default());
        assert!(!conteo.marcado());
        let texto = texto_revision_lectura("x", &conteo);
        assert!(texto.contains("sin marcas"), "{texto}");
        let json = json_revision_lectura("x", &conteo);
        assert_eq!(json["marcado"], false);
    }

    #[test]
    fn revisar_lectura_transcript_inexistente_da_error_de_uso_con_la_ruta() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let root = raiz_con_oculto_lib(&tmp);
        let falta = tmp.path().join("no-existe.jsonl");
        let error = revisar_lectura(&root, None, "x", &falta).expect_err("debe fallar");
        match error {
            Error::Uso(mensaje) => {
                assert!(mensaje.contains(&falta.display().to_string()), "{mensaje}")
            }
            otro => panic!("se esperaba Uso, llegó {otro:?}"),
        }
    }

    #[test]
    fn revisar_lectura_tarea_sin_oculto_da_error_de_uso_con_la_ruta() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let root = raiz_con_oculto_lib(&tmp);
        fs::create_dir_all(root.join("oraculos/y")).expect("tarea sin oculto");
        let ruta = transcript_revision(&tmp, "x");
        let esperada = root.join("oraculos/y/oculto");
        let error = revisar_lectura(&root, None, "y", &ruta).expect_err("debe fallar");
        match error {
            Error::Uso(mensaje) => assert!(
                mensaje.contains(&esperada.display().to_string()),
                "{mensaje}"
            ),
            otro => panic!("se esperaba Uso, llegó {otro:?}"),
        }
    }

    #[test]
    fn revisar_lectura_no_crea_ni_cambia_archivos_del_root_home() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let root = raiz_con_oculto_lib(&tmp);
        let ruta = transcript_revision(&tmp, &format!("{LINEA_OCULTA_LIB}\ndef test_indice(tmp):"));
        let antes = archivos_con_bytes(&root);
        let conteo = revisar_lectura(&root, None, "x", &ruta).expect("revisa");
        assert!(conteo.marcado(), "{conteo:?}");
        assert_eq!(archivos_con_bytes(&root), antes);
    }
}
