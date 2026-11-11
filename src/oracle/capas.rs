//! Capas de `[arquitectura]` derivadas del mapa real de módulos. `proponer_capas` las arma con el mismo
//! lector que la sonda `arquitectura_nativa`. `verificar_capas` mira si las capas de una tarea pasan la
//! sonda y si el inyector `CicloEntreCapas` aplica sobre una copia del repo.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::borrado;
use crate::oracle::calibra::inyectar::{Defecto, inyectar};
use crate::oracle::perfiles::modulo_de;
use crate::oracle::perfiles::rust::{
    FALTA_CAPA, REGLA_CICLO, adyacencias, alcanzables, archivos_rs, aristas_de_capas,
    arquitectura_nativa, fuentes_o_sin_veredicto,
};
use crate::tipos::Dimension;

/// Semilla del inyector en la verificación. Solo importa si aplica o no: la semilla no cambia eso.
const SEMILLA: u64 = 42;

/// Una arista `origen -> destino` entre dos módulos del repo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Arista {
    pub origen: String,
    pub destino: String,
}

/// Resultado de `proponer_capas`. `capas` va de mayor a menor altura.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Propuesta {
    pub capas: Vec<Vec<String>>,
    pub modulos: Vec<String>,
    pub aristas: Vec<Arista>,
    /// Componentes de dos módulos o más: ciclos. Cada uno va en una sola capa.
    pub ciclos: Vec<Vec<String>>,
}

/// Resultado de `verificar_capas`. Sin fallas, las capas pasan.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Veredicto {
    pub fallas: Vec<String>,
}

/// Propone las capas del repo: una capa por altura del grafo de componentes, con los ciclos juntos.
/// Error si el repo no tiene fuentes `.rs`.
pub fn proponer_capas(repo: &Path) -> Result<Propuesta, String> {
    let archivos = fuentes_o_sin_veredicto(repo, Dimension::Arquitectura)
        .map_err(|sin| sin.motivo.unwrap_or_default())?;
    let modulos: BTreeSet<String> = archivos.iter().filter_map(|a| modulo_de(&a.ruta)).collect();
    // La raíz (`main`, `lib`) no es una capa: sus aristas no entran al grafo.
    let adj: BTreeMap<String, Vec<String>> =
        adyacencias(&aristas_de_capas(&archivos, &modulos), &modulos)
            .into_iter()
            .filter(|(origen, _)| modulos.contains(origen))
            .collect();
    let comunidades = componentes(&modulos, &adj);
    let mut grupo_de: BTreeMap<String, String> = BTreeMap::new();
    for comunidad in &comunidades {
        let representante = comunidad.iter().next().cloned().unwrap_or_default();
        for modulo in comunidad {
            grupo_de.insert(modulo.clone(), representante.clone());
        }
    }
    // Un grupo usa otro si algún módulo suyo importa un módulo del otro. Un ciclo es un solo grupo.
    let mut usa: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (origen, destinos) in &adj {
        for destino in destinos {
            let (grupo_origen, grupo_destino) = (&grupo_de[origen], &grupo_de[destino]);
            if grupo_origen != grupo_destino {
                usa.entry(grupo_origen.clone())
                    .or_default()
                    .insert(grupo_destino.clone());
            }
        }
    }
    let mut memo = BTreeMap::new();
    let mut por_altura: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for modulo in &modulos {
        let altura = altura(&grupo_de[modulo], &usa, &mut memo);
        por_altura.entry(altura).or_default().push(modulo.clone());
    }
    // La altura más alta va primero: una capa importa solo capas posteriores.
    let capas: Vec<Vec<String>> = por_altura.into_values().rev().collect();
    let ciclos: Vec<Vec<String>> = comunidades
        .iter()
        .filter(|c| c.len() >= 2)
        .map(|c| c.iter().cloned().collect())
        .collect();
    let aristas = adj
        .iter()
        .flat_map(|(origen, destinos)| {
            destinos.iter().map(move |destino| Arista {
                origen: origen.clone(),
                destino: destino.clone(),
            })
        })
        .collect();
    Ok(Propuesta {
        capas,
        modulos: modulos.into_iter().collect(),
        aristas,
        ciclos,
    })
}

/// Componentes fuertemente conexas del grafo. Dos módulos están en la misma si se alcanzan entre sí.
/// Cada componente sale una vez, en el orden de su módulo menor.
fn componentes(
    modulos: &BTreeSet<String>,
    adj: &BTreeMap<String, Vec<String>>,
) -> Vec<BTreeSet<String>> {
    let mut vistos: BTreeSet<String> = BTreeSet::new();
    let mut salida = Vec::new();
    for modulo in modulos {
        if vistos.contains(modulo) {
            continue;
        }
        let mut comunidad = BTreeSet::from([modulo.clone()]);
        for otro in alcanzables(modulo, adj) {
            if alcanzables(&otro, adj).contains(modulo) {
                comunidad.insert(otro);
            }
        }
        vistos.extend(comunidad.iter().cloned());
        salida.push(comunidad);
    }
    salida
}

/// Altura de un grupo: 1 si no usa a otro grupo; si no, 1 más la mayor altura de los que usa.
fn altura(
    grupo: &str,
    usa: &BTreeMap<String, BTreeSet<String>>,
    memo: &mut BTreeMap<String, usize>,
) -> usize {
    if let Some(valor) = memo.get(grupo) {
        return *valor;
    }
    let mayor = usa
        .get(grupo)
        .into_iter()
        .flatten()
        .map(|otro| altura(otro, usa, memo))
        .max()
        .unwrap_or(0);
    let valor = mayor + 1;
    memo.insert(grupo.to_string(), valor);
    valor
}

/// Verifica `capas` contra el repo. Copia el repo en `dir_trabajo` para probar `CicloEntreCapas` y borra
/// esa copia al terminar. Ignora los hallazgos de ciclo: los ciclos de una misma capa son de la sonda.
pub fn verificar_capas(repo: &Path, capas: &[Vec<String>], dir_trabajo: &Path) -> Veredicto {
    let mut fallas = Vec::new();
    if capas.len() < 2 {
        fallas.push(format!(
            "menos de 2 capas: hay {}, se necesitan 2 o más",
            capas.len()
        ));
    }
    let sonda = arquitectura_nativa(repo, capas, &[]);
    if sonda.valor.is_none() {
        fallas.push(format!(
            "la sonda de arquitectura no da veredicto: {}",
            sonda.motivo.unwrap_or_default()
        ));
    }
    for hallazgo in &sonda.hallazgos {
        if hallazgo.detalle.starts_with(REGLA_CICLO) {
            // Un ciclo entre módulos de una misma capa lo reporta la sonda. Esta verificación no lo cuenta.
            continue;
        }
        if hallazgo.detalle.starts_with(FALTA_CAPA) {
            fallas.push(hallazgo.detalle.clone());
        } else {
            fallas.push(format!(
                "uso hacia una capa anterior en {}:{}: {}",
                hallazgo.archivo, hallazgo.linea, hallazgo.detalle
            ));
        }
    }
    if let Err(motivo) = ciclo_entre_capas_aplica(repo, capas, dir_trabajo) {
        fallas.push(motivo);
    }
    Veredicto { fallas }
}

/// Prueba `CicloEntreCapas` sobre una copia de las fuentes en `dir_trabajo`. Borra la copia con `borrado`.
/// Error si el inyector no aplica o si falla la copia o el borrado.
fn ciclo_entre_capas_aplica(
    repo: &Path,
    capas: &[Vec<String>],
    dir_trabajo: &Path,
) -> Result<(), String> {
    let error = |e: io::Error| format!("no se pudo probar CicloEntreCapas: {e}");
    fs::create_dir_all(dir_trabajo).map_err(error)?;
    // El nombre lleva el pid: dos corridas a la vez no chocan. Un resto de otra corrida se borra antes.
    let copia = dir_trabajo.join(format!("capas-{}", std::process::id()));
    borrado::borrar_dir_dentro(dir_trabajo, &copia).map_err(error)?;
    fs::create_dir(&copia).map_err(error)?;
    let aplicado = copiar_fuentes(repo, &copia)
        .and_then(|()| inyectar(Defecto::CicloEntreCapas, &copia, capas, SEMILLA));
    let limpieza = borrado::borrar_dir_dentro(dir_trabajo, &copia);
    let aplicado = match (aplicado, limpieza) {
        (Err(e), _) => return Err(error(e)),
        (_, Err(e)) => return Err(format!("no se pudo borrar la copia de prueba: {e}")),
        (Ok(aplicado), Ok(())) => aplicado,
    };
    match aplicado {
        Some(_) => Ok(()),
        None => Err(
            "no aplica CicloEntreCapas: hace falta un módulo de una capa posterior que tenga archivo en src/ \
             y otro de una anterior"
                .into(),
        ),
    }
}

/// Copia las fuentes `.rs` del repo a `destino`, con sus rutas relativas. El inyector solo toca `src/`.
fn copiar_fuentes(repo: &Path, destino: &Path) -> io::Result<()> {
    let archivos = archivos_rs(repo).map_err(io::Error::other)?;
    for archivo in archivos {
        let dest = destino.join(&archivo.ruta);
        if let Some(padre) = dest.parent() {
            fs::create_dir_all(padre)?;
        }
        fs::copy(repo.join(&archivo.ruta), &dest)?;
    }
    Ok(())
}

/// Texto TOML de las capas: `capas = [["a"], ["b", "c"]]`. Los nombres van entre comillas JSON.
pub fn linea_toml(capas: &[Vec<String>]) -> String {
    let arreglos: Vec<String> = capas
        .iter()
        .map(|capa| {
            let nombres: Vec<String> = capa
                .iter()
                .map(|m| serde_json::to_string(m).unwrap_or_default())
                .collect();
            format!("[{}]", nombres.join(", "))
        })
        .collect();
    format!("capas = [{}]", arreglos.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escribir(repo: &Path, ruta: &str, texto: &str) {
        let p = repo.join(ruta);
        fs::create_dir_all(p.parent().expect("con padre")).expect("crea carpeta");
        fs::write(p, texto).expect("escribe el fuente");
    }

    /// Repo con `a -> b -> c`. Cada módulo es un archivo de `src/`.
    fn repo_cadena(dir: &Path) {
        escribir(dir, "Cargo.toml", "[package]\nname = \"fixture\"\n");
        escribir(dir, "src/lib.rs", "pub mod a;\npub mod b;\npub mod c;\n");
        escribir(dir, "src/a.rs", "use crate::b;\npub fn a() {}\n");
        escribir(dir, "src/b.rs", "use crate::c;\npub fn b() {}\n");
        escribir(dir, "src/c.rs", "pub fn c() {}\n");
    }

    /// Repo con `a -> b` y un ciclo `b <-> c`.
    fn repo_con_ciclo(dir: &Path) {
        escribir(dir, "Cargo.toml", "[package]\nname = \"fixture\"\n");
        escribir(dir, "src/lib.rs", "pub mod a;\npub mod b;\npub mod c;\n");
        escribir(dir, "src/a.rs", "use crate::b;\npub fn a() {}\n");
        escribir(dir, "src/b.rs", "use crate::c;\npub fn b() {}\n");
        escribir(dir, "src/c.rs", "use crate::b;\npub fn c() {}\n");
    }

    fn capas(v: &[&[&str]]) -> Vec<Vec<String>> {
        v.iter()
            .map(|c| c.iter().map(|m| m.to_string()).collect())
            .collect()
    }

    fn trabajo() -> tempfile::TempDir {
        tempfile::tempdir().expect("árbol de trabajo de prueba")
    }

    #[test]
    fn propone_una_capa_por_altura_en_cadena() {
        let dir = tempfile::tempdir().expect("repo");
        repo_cadena(dir.path());
        let p = proponer_capas(dir.path()).expect("propone");
        assert_eq!(p.capas, capas(&[&["a"], &["b"], &["c"]]));
    }

    #[test]
    fn ciclo_entre_b_y_c_va_en_la_misma_capa() {
        let dir = tempfile::tempdir().expect("repo");
        repo_con_ciclo(dir.path());
        let p = proponer_capas(dir.path()).expect("propone");
        assert_eq!(p.capas, capas(&[&["a"], &["b", "c"]]));
        assert_eq!(p.ciclos, capas(&[&["b", "c"]]));
    }

    #[test]
    fn propone_capas_con_imports_agrupados() {
        let dir = tempfile::tempdir().expect("repo");
        escribir(dir.path(), "Cargo.toml", "[package]\nname = \"fixture\"\n");
        escribir(
            dir.path(),
            "src/lib.rs",
            "pub mod a;\npub mod b;\npub mod c;\n",
        );
        escribir(
            dir.path(),
            "src/a.rs",
            "use crate::{\n    b::{f, g},\n    c,\n};\npub fn a() {}\n",
        );
        escribir(
            dir.path(),
            "src/b.rs",
            "use crate::{\n    c::h,\n};\npub fn b() {}\n",
        );
        escribir(dir.path(), "src/c.rs", "pub fn c() {}\n");
        let p = proponer_capas(dir.path()).expect("propone");
        assert_eq!(p.capas, capas(&[&["a"], &["b"], &["c"]]));
    }

    #[test]
    fn aristas_hacia_la_raiz_no_cuentan() {
        let dir = tempfile::tempdir().expect("repo");
        escribir(
            dir.path(),
            "src/lib.rs",
            "pub struct Config;\npub mod a;\nuse crate::a::f;\n",
        );
        escribir(
            dir.path(),
            "src/a.rs",
            "use crate::Config;\npub fn f() {}\n",
        );
        let p = proponer_capas(dir.path()).expect("propone");
        assert_eq!(p.capas, capas(&[&["a"]]));
        assert!(p.aristas.is_empty(), "{:?}", p.aristas);
    }

    #[test]
    fn repo_sin_fuentes_rs_da_error_al_proponer() {
        let dir = tempfile::tempdir().expect("repo");
        escribir(dir.path(), "Cargo.toml", "[package]\nname = \"fixture\"\n");
        assert!(proponer_capas(dir.path()).is_err());
    }

    #[test]
    fn linea_toml_junta_cada_capa_en_un_arreglo() {
        assert_eq!(
            linea_toml(&capas(&[&["a"], &["b", "c"]])),
            r#"capas = [["a"], ["b", "c"]]"#
        );
    }

    #[test]
    fn capas_propuestas_pasan_la_verificacion_y_borran_su_copia() {
        let dir = tempfile::tempdir().expect("repo");
        repo_cadena(dir.path());
        let propuesta = proponer_capas(dir.path()).expect("propone");
        let trabajo = trabajo();
        let v = verificar_capas(dir.path(), &propuesta.capas, trabajo.path());
        assert!(v.fallas.is_empty(), "{:?}", v.fallas);
        let restos: Vec<_> = fs::read_dir(trabajo.path()).expect("lee").collect();
        assert!(restos.is_empty(), "la copia de prueba quedó: {restos:?}");
    }

    #[test]
    fn propuesta_con_ciclo_pasa_la_verificacion() {
        let dir = tempfile::tempdir().expect("repo");
        repo_con_ciclo(dir.path());
        let propuesta = proponer_capas(dir.path()).expect("propone");
        let trabajo = trabajo();
        let v = verificar_capas(dir.path(), &propuesta.capas, trabajo.path());
        assert!(v.fallas.is_empty(), "{:?}", v.fallas);
    }

    #[test]
    fn capa_que_no_existe_no_pasa_y_nombra_las_dos_faltas() {
        let dir = tempfile::tempdir().expect("repo");
        repo_cadena(dir.path());
        let trabajo = trabajo();
        let v = verificar_capas(dir.path(), &capas(&[&["patron"]]), trabajo.path());
        assert!(
            v.fallas.iter().any(|f| f.contains("falta la capa")),
            "{:?}",
            v.fallas
        );
        assert!(
            v.fallas.iter().any(|f| f.contains("menos de 2 capas")),
            "{:?}",
            v.fallas
        );
    }

    #[test]
    fn una_capa_no_aplica_ciclo_entre_capas() {
        let dir = tempfile::tempdir().expect("repo");
        repo_cadena(dir.path());
        let trabajo = trabajo();
        let v = verificar_capas(dir.path(), &capas(&[&["a"]]), trabajo.path());
        assert!(
            v.fallas.iter().any(|f| f.contains("menos de 2 capas")),
            "{:?}",
            v.fallas
        );
        assert!(
            v.fallas
                .iter()
                .any(|f| f.contains("no aplica CicloEntreCapas")),
            "{:?}",
            v.fallas
        );
    }

    #[test]
    fn uso_agregado_hacia_una_capa_anterior_no_pasa() {
        let dir = tempfile::tempdir().expect("repo");
        repo_cadena(dir.path());
        escribir(dir.path(), "src/c.rs", "use crate::a;\npub fn c() {}\n");
        let trabajo = trabajo();
        let v = verificar_capas(
            dir.path(),
            &capas(&[&["a"], &["b"], &["c"]]),
            trabajo.path(),
        );
        assert!(
            v.fallas
                .iter()
                .any(|f| f.contains("uso hacia una capa anterior")),
            "{:?}",
            v.fallas
        );
        assert!(
            !v.fallas.iter().any(|f| f.contains(REGLA_CICLO)),
            "los ciclos no cuentan: {:?}",
            v.fallas
        );
    }
}
