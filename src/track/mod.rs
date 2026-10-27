//! Track: guarda una fila por corrida y compara configuraciones con la cota de Wilson.

pub mod stats;

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error as ErrorDerive;

use crate::config::root_home;
use crate::observe::jsonl;
use crate::tipos::{Estado, Fila, Rol};

/// Errores de Track. Salen con código 3.
#[derive(Debug, ErrorDerive)]
pub enum ErrorTrack {
    #[error("error de E/S: {0}")]
    Io(#[from] io::Error),
    #[error("error de JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{archivo}: la línea {linea} es inválida: {fuente}")]
    Linea {
        archivo: String,
        linea: usize,
        #[source]
        fuente: serde_json::Error,
    },
}

/// Filtro de filas. Un campo en `None` no filtra.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filtro {
    pub tarea: Option<String>,
    pub config: Option<String>,
}

/// Estadística de una configuración. Solo cuenta filas válidas (no invalidadas).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatModelo {
    pub etiqueta: String,
    pub n: u32,
    pub exitos: u32,
    pub wilson_inf: f64,
    pub usd_medio: f64,
}

/// Resultado de comparar dos configuraciones en una tarea.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparacion {
    pub tarea: String,
    pub a: StatModelo,
    pub b: StatModelo,
    pub mejor: Option<String>,
}

impl Filtro {
    fn admite(&self, fila: &Fila) -> bool {
        self.tarea.as_deref().is_none_or(|t| t == fila.tarea)
            && self.config.as_deref().is_none_or(|c| c == fila.config)
    }
}

/// Ruta de `runs.jsonl` dentro de `ROOT_HOME`.
pub fn ruta_runs() -> PathBuf {
    root_home().join("runs.jsonl")
}

/// Agrega una fila a `runs.jsonl` de `ROOT_HOME`.
pub fn agregar(f: &Fila) -> io::Result<()> {
    agregar_en(&ruta_runs(), f)
}

/// Agrega una fila al archivo indicado, con lock y rotación.
pub fn agregar_en(ruta: &Path, f: &Fila) -> io::Result<()> {
    let linea = serde_json::to_string(f).map_err(io::Error::other)?;
    jsonl::agregar(ruta, &linea, jsonl::UMBRAL_POR_DEFECTO)
}

/// Filas de `runs.jsonl` de `ROOT_HOME` que pasan el filtro.
pub fn filas(filtro: &Filtro) -> Result<Vec<Fila>, ErrorTrack> {
    filas_de(&ruta_runs(), filtro)
}

/// Filas del archivo indicado que pasan el filtro. Un archivo inexistente da una lista vacía.
pub fn filas_de(ruta: &Path, filtro: &Filtro) -> Result<Vec<Fila>, ErrorTrack> {
    let mut filas = Vec::new();
    for archivo in jsonl::archivos_en_orden(ruta)? {
        // El número de línea cuenta también las líneas en blanco, que se omiten. Es el de cada archivo.
        for (indice, linea) in jsonl::leer_texto(&archivo)?.lines().enumerate() {
            if linea.trim().is_empty() {
                continue;
            }
            let fila: Fila = serde_json::from_str(linea).map_err(|fuente| ErrorTrack::Linea {
                archivo: archivo.display().to_string(),
                linea: indice + 1,
                fuente,
            })?;
            if filtro.admite(&fila) {
                filas.push(fila);
            }
        }
    }
    Ok(filas)
}

/// Estadística por modelo del rol de las filas de `tipo`.
pub fn estadisticas(rol: Rol, tipo: &str) -> Result<Vec<StatModelo>, ErrorTrack> {
    let filtro = Filtro {
        tarea: Some(tipo.into()),
        config: None,
    };
    Ok(estadisticas_de(&filas(&filtro)?, rol))
}

/// Estadística por modelo de `rol` de una lista de filas, en orden de etiqueta.
/// Una fila sin modelo para ese rol usa su `config` como etiqueta.
pub fn estadisticas_de(filas: &[Fila], rol: Rol) -> Vec<StatModelo> {
    let mut grupos: BTreeMap<&str, Vec<&Fila>> = BTreeMap::new();
    for fila in filas {
        let etiqueta = fila
            .modelos
            .get(&rol)
            .map_or(fila.config.as_str(), String::as_str);
        grupos.entry(etiqueta).or_default().push(fila);
    }
    grupos
        .into_iter()
        .map(|(etiqueta, del_grupo)| stat_de(etiqueta, &del_grupo))
        .collect()
}

/// Estadística de una configuración. Las filas invalidadas no cuentan.
fn stat_de(etiqueta: &str, filas: &[&Fila]) -> StatModelo {
    let validas: Vec<&Fila> = filas
        .iter()
        .copied()
        .filter(|fila| !fila.resumen.invalida)
        .collect();
    let n = u32::try_from(validas.len()).unwrap_or(u32::MAX);
    let exitos = u32::try_from(
        validas
            .iter()
            .filter(|fila| fila.veredicto.estado == Estado::Verde)
            .count(),
    )
    .unwrap_or(u32::MAX);
    let usd_total: f64 = validas
        .iter()
        .map(|fila| fila.resumen.usd_api + fila.resumen.usd_cupo)
        .sum();
    let usd_medio = if n == 0 {
        0.0
    } else {
        usd_total / f64::from(n)
    };
    StatModelo {
        etiqueta: etiqueta.into(),
        n,
        exitos,
        wilson_inf: stats::wilson_inferior(exitos, n),
        usd_medio,
    }
}

/// Compara dos configuraciones de `tarea` en `runs.jsonl` de `ROOT_HOME`.
pub fn comparar(a: &str, b: &str, tarea: &str) -> Result<Comparacion, ErrorTrack> {
    comparar_en(&ruta_runs(), a, b, tarea)
}

/// Compara dos configuraciones de `tarea` en el archivo indicado.
pub fn comparar_en(ruta: &Path, a: &str, b: &str, tarea: &str) -> Result<Comparacion, ErrorTrack> {
    let filtro = Filtro {
        tarea: Some(tarea.into()),
        config: None,
    };
    let todas = filas_de(ruta, &filtro)?;
    let de = |config: &str| {
        let del_config: Vec<&Fila> = todas.iter().filter(|f| f.config == config).collect();
        stat_de(config, &del_config)
    };
    let sa = de(a);
    let sb = de(b);
    let mejor = match sa.wilson_inf.partial_cmp(&sb.wilson_inf) {
        Some(Ordering::Greater) => Some(sa.etiqueta.clone()),
        Some(Ordering::Less) => Some(sb.etiqueta.clone()),
        _ => None,
    };
    Ok(Comparacion {
        tarea: tarea.into(),
        a: sa,
        b: sb,
        mejor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tipos::{Resumen, Rol, Veredicto};
    use tempfile::TempDir;

    fn fila(tarea: &str, config: &str, estado: Estado) -> Fila {
        Fila {
            tarea: tarea.into(),
            config: config.into(),
            veredicto: Veredicto {
                estado,
                ..Veredicto::default()
            },
            ..Fila::default()
        }
    }

    fn invalida(mut f: Fila) -> Fila {
        f.resumen.invalida = true;
        f
    }

    fn con_usd(mut f: Fila, usd_api: f64) -> Fila {
        f.resumen = Resumen {
            usd_api,
            ..Resumen::default()
        };
        f
    }

    #[test]
    fn track_agregar_y_filas_ida_y_vuelta() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("runs.jsonl");
        let f = fila("memora", "haiku", Estado::Verde);
        agregar_en(&ruta, &f).expect("agrega");
        let leidas = filas_de(&ruta, &Filtro::default()).expect("lee");
        assert_eq!(leidas, vec![f]);
    }

    #[test]
    fn track_filas_lee_archivos_rotados_antes_que_el_vigente() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("runs.jsonl");
        // Umbral de 100 bytes: cada fila supera el umbral, así que cada escritura rota.
        let escribe = |id: &str| {
            let mut f = fila("memora", "haiku", Estado::Verde);
            f.id = id.into();
            let linea = serde_json::to_string(&f).expect("serializa");
            jsonl::agregar(&ruta, &linea, 100).expect("escribe");
        };
        escribe("f-1");
        escribe("f-2");
        escribe("f-3");
        assert!(dir.path().join("runs.jsonl.2").exists());
        let ids: Vec<String> = filas_de(&ruta, &Filtro::default())
            .expect("lee")
            .into_iter()
            .map(|f| f.id)
            .collect();
        assert_eq!(ids, vec!["f-1", "f-2", "f-3"]);
    }

    #[test]
    fn track_filas_de_archivo_inexistente_da_lista_vacia() {
        let dir = TempDir::new().expect("temp dir");
        let leidas =
            filas_de(&dir.path().join("no.jsonl"), &Filtro::default()).expect("no es error");
        assert!(leidas.is_empty());
    }

    #[test]
    fn track_filtro_por_tarea_y_por_config() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("runs.jsonl");
        agregar_en(&ruta, &fila("memora", "haiku", Estado::Verde)).expect("a");
        agregar_en(&ruta, &fila("brasa", "haiku", Estado::Rojo)).expect("b");
        agregar_en(&ruta, &fila("memora", "opus", Estado::Rojo)).expect("c");

        let por_tarea = Filtro {
            tarea: Some("memora".into()),
            ..Filtro::default()
        };
        assert_eq!(filas_de(&ruta, &por_tarea).expect("lee").len(), 2);

        let por_config = Filtro {
            config: Some("haiku".into()),
            ..Filtro::default()
        };
        assert_eq!(filas_de(&ruta, &por_config).expect("lee").len(), 2);

        let ambos = Filtro {
            tarea: Some("memora".into()),
            config: Some("haiku".into()),
        };
        let unica = filas_de(&ruta, &ambos).expect("lee");
        assert_eq!(unica.len(), 1);
        assert_eq!(unica[0].veredicto.estado, Estado::Verde);
    }

    #[test]
    fn track_linea_invalida_da_error_con_numero_de_linea() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("runs.jsonl");
        let valida = serde_json::to_string(&fila("memora", "haiku", Estado::Verde)).expect("json");
        // Línea 1 válida, línea 2 en blanco (se tolera), línea 3 inválida.
        std::fs::write(&ruta, format!("{valida}\n\nno es json\n")).expect("escribe");
        match filas_de(&ruta, &Filtro::default()) {
            Err(ErrorTrack::Linea { linea, .. }) => assert_eq!(linea, 3),
            otro => panic!("se esperaba error de línea 3, llegó {otro:?}"),
        }
    }

    #[test]
    fn track_estadisticas_agrupan_por_modelo_del_rol() {
        let con_modelo = |modelo: &str| {
            let mut f = con_usd(fila("memora", "haiku", Estado::Verde), 1.0);
            f.modelos.insert(Rol::Construye, modelo.into());
            f
        };
        let filas = vec![con_modelo("modelo-a"), con_modelo("modelo-b")];
        let stats = estadisticas_de(&filas, Rol::Construye);
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].etiqueta, "modelo-a");
        assert_eq!(stats[1].etiqueta, "modelo-b");
    }

    #[test]
    fn track_estadisticas_sin_modelo_del_rol_usan_la_config() {
        let sin_modelos = con_usd(fila("memora", "haiku", Estado::Verde), 1.0);
        let stats = estadisticas_de(&[sin_modelos], Rol::Construye);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].etiqueta, "haiku");
    }

    #[test]
    fn track_estadisticas_excluyen_invalidas_y_cuentan_verde() {
        let filas = vec![
            con_usd(fila("memora", "haiku", Estado::Verde), 1.0),
            con_usd(fila("memora", "haiku", Estado::Rojo), 0.0),
            invalida(con_usd(fila("memora", "haiku", Estado::Verde), 9.0)),
            con_usd(fila("memora", "opus", Estado::Verde), 2.0),
        ];
        let stats = estadisticas_de(&filas, Rol::Construye);
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].etiqueta, "haiku");
        assert_eq!(stats[0].n, 2);
        assert_eq!(stats[0].exitos, 1);
        assert_eq!(stats[0].usd_medio, 0.5);
        assert!((stats[0].wilson_inf - stats::wilson_inferior(1, 2)).abs() < 1e-12);
        assert_eq!(stats[1].etiqueta, "opus");
        assert_eq!(stats[1].n, 1);
        assert_eq!(stats[1].usd_medio, 2.0);
    }

    #[test]
    fn track_comparar_elige_la_config_con_mayor_wilson() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("runs.jsonl");
        for _ in 0..5 {
            agregar_en(&ruta, &fila("memora", "fuerte", Estado::Verde)).expect("a");
        }
        agregar_en(&ruta, &fila("memora", "debil", Estado::Verde)).expect("b");
        for _ in 0..4 {
            agregar_en(&ruta, &fila("memora", "debil", Estado::Rojo)).expect("c");
        }
        agregar_en(&ruta, &fila("otra", "fuerte", Estado::Rojo)).expect("otra tarea");

        let cmp = comparar_en(&ruta, "fuerte", "debil", "memora").expect("compara");
        assert_eq!(cmp.a.n, 5);
        assert_eq!(cmp.b.exitos, 1);
        assert_eq!(cmp.mejor.as_deref(), Some("fuerte"));
    }

    #[test]
    fn track_comparar_empate_da_none() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("runs.jsonl");
        agregar_en(&ruta, &fila("memora", "p", Estado::Verde)).expect("p");
        agregar_en(&ruta, &fila("memora", "q", Estado::Verde)).expect("q");
        let cmp = comparar_en(&ruta, "p", "q", "memora").expect("compara");
        assert_eq!(cmp.mejor, None);
    }

    #[test]
    fn track_comparar_config_sin_filas_da_n_cero() {
        let dir = TempDir::new().expect("temp dir");
        let ruta = dir.path().join("runs.jsonl");
        agregar_en(&ruta, &fila("memora", "p", Estado::Verde)).expect("p");
        let cmp = comparar_en(&ruta, "p", "nadie", "memora").expect("no es error");
        assert_eq!(cmp.b.n, 0);
        assert_eq!(cmp.b.wilson_inf, 0.0);
        assert_eq!(cmp.mejor.as_deref(), Some("p"));
    }
}
