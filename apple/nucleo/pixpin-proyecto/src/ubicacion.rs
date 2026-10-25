//! Donde vive un proyecto: la zona habitual (`proyectos/<id>`) o una carpeta
//! que eligio el usuario.
//!
//! El usuario lo pidio asi: «que me deje seleccionar un lugar para guardar
//! todo lo relacionado a un proyecto en una carpeta especifica y si no
//! selecciono se ponga todo en la zona habitual».
//!
//! **Como**: el contenido se mueve a la carpeta elegida y `proyectos/<id>`
//! pasa a ser una union de directorio (`pixpin_sistema::union`) que apunta
//! alli. Asi los muchos sitios que escriben en `almacen::carpeta` —el
//! cuaderno, los adjuntos, los lienzos, sincronizar— siguen igual y escriben
//! donde el usuario quiere sin saberlo. La ruta se apunta tambien en la
//! ficha (`Ficha::ubicacion`) para ensenarla y para rehacer la union si
//! alguien la quita.
//!
//! **La regla de mover**: primero se copia todo, y el original solo se
//! borra cuando la copia esta entera y la union ya apunta a ella. Un fallo a
//! mitad —un fichero abierto por otro programa, un disco lleno— deja el
//! proyecto donde estaba y se lleva solo lo copiado a medias.

use std::io;
use std::path::{Path, PathBuf};

use pixpin_sistema::union;

use crate::almacen::{self, Indice};

/// Por que no se pudo cambiar de sitio un proyecto. Cada uno tiene su texto
/// (`clave`): quien lo ensena es la ventana, que es quien traduce.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("la carpeta esta en una unidad de red")]
    Red,
    #[error("no es una carpeta que exista en este equipo")]
    NoEsCarpeta,
    #[error("la carpeta esta dentro del propio proyecto o del almacen")]
    Dentro,
    #[error("la carpeta del proyecto no esta disponible: {0}")]
    NoDisponible(PathBuf),
    #[error("ese proyecto no esta en la lista")]
    NoEsta,
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Error {
    /// La clave del `.ftl` que lo explica al usuario.
    pub fn clave(&self) -> &'static str {
        match self {
            Error::Red => "ubicacion-error-red",
            Error::NoEsCarpeta => "ubicacion-error-no-es-carpeta",
            Error::Dentro => "ubicacion-error-dentro",
            Error::NoDisponible(_) => "ubicacion-no-disponible",
            Error::NoEsta | Error::Io(_) => "ubicacion-error-mover",
        }
    }
}

/// Donde esta un proyecto ahora mismo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    /// En `proyectos/<id>`, o todavia sin carpeta.
    Habitual,
    /// En la carpeta que eligio el usuario, y esa carpeta esta.
    Propia(PathBuf),
    /// En una carpeta que ahora no esta: el USB desenchufado, el disco
    /// desmontado, o la borro alguien.
    NoDisponible(PathBuf),
}

/// Donde esta, sin tocar nada.
pub fn estado(raiz: &Path, ficha: &almacen::Ficha) -> Estado {
    let enlace = almacen::carpeta(raiz, &ficha.id);
    if let Some(d) = union::es_union(&enlace)
        .then(|| union::destino(&enlace))
        .flatten()
    {
        return if d.is_dir() {
            Estado::Propia(d)
        } else {
            Estado::NoDisponible(d)
        };
    }
    match &ficha.ubicacion {
        // Sin union pero con ruta apuntada, y sin carpeta local: la union se
        // perdio. `preparar` la rehace; aqui solo se cuenta.
        Some(u) if std::fs::symlink_metadata(&enlace).is_err() => {
            let d = PathBuf::from(u);
            if d.is_dir() {
                Estado::Propia(d)
            } else {
                Estado::NoDisponible(d)
            }
        }
        _ => Estado::Habitual,
    }
}

/// Si se puede abrir: lo barato, para pintar la lista sin tocar el disco en
/// los proyectos de siempre.
pub fn disponible(ficha: &almacen::Ficha) -> bool {
    ficha
        .ubicacion
        .as_ref()
        .is_none_or(|u| Path::new(u).is_dir())
}

/// La carpeta de verdad del proyecto: la elegida si la tiene, o la de
/// siempre. Es la que se abre en el Explorador: la union en `AppData` no
/// le dice nada al usuario.
pub fn carpeta_real(raiz: &Path, id: &str) -> PathBuf {
    let enlace = almacen::carpeta(raiz, id);
    if union::es_union(&enlace) {
        union::destino(&enlace).unwrap_or(enlace)
    } else {
        enlace
    }
}

/// Deja el proyecto listo para abrirse: si tiene carpeta propia y su union
/// falta, la rehace; y si esa carpeta no esta, lo dice en vez de dejar que
/// el primer mensaje cree una vacia en la zona habitual.
///
/// La union se rehace AUNQUE su destino no este: una union rota hace fallar
/// a quien escriba (`create_dir_all` no la «repara»), y sin ella el primer
/// mensaje crearia `proyectos/<id>` vacia y los mensajes se quedarian ahi,
/// lejos del resto del proyecto, cuando el disco vuelva.
pub fn preparar(raiz: &Path, ficha: &almacen::Ficha) -> Result<(), Error> {
    let enlace = almacen::carpeta(raiz, &ficha.id);
    if union::es_union(&enlace) {
        return match union::destino(&enlace) {
            Some(d) if d.is_dir() => Ok(()),
            Some(d) => Err(Error::NoDisponible(d)),
            None => Err(Error::NoDisponible(enlace)),
        };
    }
    let Some(u) = &ficha.ubicacion else {
        return Ok(());
    };
    // Una carpeta de verdad en su sitio: lo que haya ahi es lo que manda.
    if std::fs::symlink_metadata(&enlace).is_ok() {
        return Ok(());
    }
    let destino = PathBuf::from(u);
    std::fs::create_dir_all(raiz.join("proyectos"))?;
    union::crear(&enlace, &destino)?;
    if destino.is_dir() {
        Ok(())
    } else {
        Err(Error::NoDisponible(destino))
    }
}

/// **Mueve el proyecto `id` a la carpeta `elegida`** y deja en su sitio la
/// union. Devuelve la carpeta donde queda.
///
/// Si `elegida` esta vacia, el proyecto va en ella; si no, en una subcarpeta
/// con el nombre del proyecto, para no mezclarlo con lo que el usuario ya
/// tuviera ahi. Solo discos de este equipo: una unidad de red se rechaza.
pub fn mover_a(raiz: &Path, id: &str, elegida: &Path) -> Result<PathBuf, Error> {
    mover_a_con(raiz, id, elegida, false)
}

/// `forzar_copia` salta el renombrado rapido, que es lo que pasa entre dos
/// discos: las pruebas lo usan para probar el camino de copiar en uno solo.
fn mover_a_con(
    raiz: &Path,
    id: &str,
    elegida: &Path,
    forzar_copia: bool,
) -> Result<PathBuf, Error> {
    if !id_valido(id) {
        return Err(Error::NoEsta);
    }
    let mut indice = Indice::leer_si_esta(raiz)?;
    let ficha = indice.buscar(id).cloned().ok_or(Error::NoEsta)?;
    let elegida = union::sin_prefijo_largo(elegida);
    if union::es_de_red(&elegida) {
        return Err(Error::Red);
    }
    if !elegida.is_absolute() || !elegida.is_dir() {
        return Err(Error::NoEsCarpeta);
    }
    let enlace = almacen::carpeta(raiz, id);
    let es_union = union::es_union(&enlace);
    let actual: Option<PathBuf> = if es_union {
        let d = union::destino(&enlace).ok_or_else(|| Error::NoDisponible(enlace.clone()))?;
        if !d.is_dir() {
            return Err(Error::NoDisponible(d));
        }
        Some(d)
    } else if enlace.is_dir() {
        Some(enlace.clone())
    } else {
        None
    };
    let elegida_c = canonica(&elegida);
    if let Some(a) = &actual {
        let a = canonica(a);
        // Elegir la carpeta donde ya esta: no hay nada que hacer.
        if es_union && a == elegida_c {
            return Ok(elegida);
        }
        // Ni la carpeta que la contiene: ya esta dentro de ella, y se
        // duplicaria en una subcarpeta «(2)» al lado de si mismo.
        if es_union && a.parent() == Some(elegida_c.as_path()) {
            return Ok(actual.clone().unwrap_or(a));
        }
        // Dentro de si mismo no se puede copiar: no acabaria nunca.
        if elegida_c.starts_with(&a) {
            return Err(Error::Dentro);
        }
    }
    // Ni dentro del almacen: se confundiria con los proyectos de la zona
    // habitual, y al volver se borraria lo que no es suyo.
    if elegida_c.starts_with(canonica(&raiz.join("proyectos"))) {
        return Err(Error::Dentro);
    }
    let nombre = if ficha.nombre.trim().is_empty() {
        format!("Proyecto {id}")
    } else {
        ficha.nombre.clone()
    };
    let destino = destino_en(&elegida, &nombre)?;
    let destino_nuevo = !destino.exists();

    match actual {
        // Todavia sin carpeta (recien creado): basta con la union.
        None => {
            std::fs::create_dir_all(&destino)?;
            std::fs::create_dir_all(raiz.join("proyectos"))?;
            if let Err(e) = union::crear(&enlace, &destino) {
                if destino_nuevo {
                    let _ = std::fs::remove_dir(&destino);
                }
                return Err(e.into());
            }
        }
        Some(_) if !es_union => {
            desde_la_habitual(raiz, id, &enlace, &destino, destino_nuevo, forzar_copia)?;
        }
        Some(origen) => {
            entre_propias(&enlace, &origen, &destino, destino_nuevo, forzar_copia)?;
        }
    }
    // La union ya manda; la ficha es para ensenarla. Si guardar la lista
    // falla, el proyecto ya esta bien donde esta: se dice, sin deshacer.
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == id) {
        f.ubicacion = Some(destino.display().to_string());
    }
    indice.guardar(raiz)?;
    Ok(destino)
}

/// De `proyectos/<id>` (una carpeta de verdad) a `destino`.
fn desde_la_habitual(
    raiz: &Path,
    id: &str,
    enlace: &Path,
    destino: &Path,
    destino_nuevo: bool,
    forzar_copia: bool,
) -> Result<(), Error> {
    // En el mismo disco, renombrar es instantaneo y no puede quedar a medias.
    if !forzar_copia && destino_nuevo && std::fs::rename(enlace, destino).is_ok() {
        if let Err(e) = union::crear(enlace, destino) {
            let _ = std::fs::rename(destino, enlace);
            return Err(e.into());
        }
        return Ok(());
    }
    copiar_o_limpiar(enlace, destino, destino_nuevo)?;
    // Se aparta el original en vez de borrarlo: si la union no se puede
    // crear, vuelve a su sitio tal cual.
    let apartado = raiz.join("proyectos").join(format!("{id}.moviendo"));
    if std::fs::symlink_metadata(&apartado).is_ok() {
        limpiar(destino, destino_nuevo);
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "quedo un traslado a medias de antes",
        )
        .into());
    }
    if let Err(e) = std::fs::rename(enlace, &apartado) {
        // Algo lo tiene abierto: el proyecto se queda donde estaba.
        limpiar(destino, destino_nuevo);
        return Err(e.into());
    }
    if let Err(e) = union::crear(enlace, destino) {
        let _ = std::fs::rename(&apartado, enlace);
        limpiar(destino, destino_nuevo);
        return Err(e.into());
    }
    // Ya esta todo en su sitio nuevo y la union apunta alli: lo apartado es
    // una copia de mas. Si no se puede borrar ahora no se pierde nada.
    let _ = std::fs::remove_dir_all(&apartado);
    Ok(())
}

/// De una carpeta elegida a otra.
fn entre_propias(
    enlace: &Path,
    origen: &Path,
    destino: &Path,
    destino_nuevo: bool,
    forzar_copia: bool,
) -> Result<(), Error> {
    if !forzar_copia && destino_nuevo && std::fs::rename(origen, destino).is_ok() {
        let rehecha = union::quitar(enlace).and_then(|_| union::crear(enlace, destino));
        if let Err(e) = rehecha {
            let _ = std::fs::rename(destino, origen);
            if !union::es_union(enlace) {
                let _ = union::crear(enlace, origen);
            }
            return Err(e.into());
        }
        return Ok(());
    }
    copiar_o_limpiar(origen, destino, destino_nuevo)?;
    let rehecha = union::quitar(enlace).and_then(|_| union::crear(enlace, destino));
    if let Err(e) = rehecha {
        if !union::es_union(enlace) {
            let _ = union::crear(enlace, origen);
        }
        limpiar(destino, destino_nuevo);
        return Err(e.into());
    }
    vaciar_lo_movido(origen);
    Ok(())
}

/// **Vuelve a la zona habitual**: lo trae de la carpeta elegida a
/// `proyectos/<id>`, quita la union y olvida la ruta.
pub fn volver_a_habitual(raiz: &Path, id: &str) -> Result<(), Error> {
    volver_con(raiz, id, false)
}

fn volver_con(raiz: &Path, id: &str, forzar_copia: bool) -> Result<(), Error> {
    if !id_valido(id) {
        return Err(Error::NoEsta);
    }
    let mut indice = Indice::leer_si_esta(raiz)?;
    if indice.buscar(id).is_none() {
        return Err(Error::NoEsta);
    }
    let enlace = almacen::carpeta(raiz, id);
    if union::es_union(&enlace) {
        let origen = union::destino(&enlace).ok_or_else(|| Error::NoDisponible(enlace.clone()))?;
        if !origen.is_dir() {
            // Sin el disco no hay que traer: quitar la union ahora dejaria el
            // proyecto vacio en la zona habitual.
            return Err(Error::NoDisponible(origen));
        }
        traer(raiz, id, &enlace, &origen, forzar_copia)?;
    }
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == id) {
        f.ubicacion = None;
    }
    indice.guardar(raiz)?;
    Ok(())
}

fn traer(
    raiz: &Path,
    id: &str,
    enlace: &Path,
    origen: &Path,
    forzar_copia: bool,
) -> Result<(), Error> {
    if !forzar_copia {
        union::quitar(enlace)?;
        if std::fs::rename(origen, enlace).is_ok() {
            return Ok(());
        }
        // Otro disco (o algo abierto): se vuelve a poner la union y se copia.
        union::crear(enlace, origen)?;
    }
    let llegando = raiz.join("proyectos").join(format!("{id}.volviendo"));
    if std::fs::symlink_metadata(&llegando).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "quedo una vuelta a medias de antes",
        )
        .into());
    }
    copiar_o_limpiar(origen, &llegando, true)?;
    union::quitar(enlace)?;
    if let Err(e) = std::fs::rename(&llegando, enlace) {
        let _ = union::crear(enlace, origen);
        let _ = std::fs::remove_dir_all(&llegando);
        return Err(e.into());
    }
    vaciar_lo_movido(origen);
    Ok(())
}

/// La carpeta concreta dentro de la elegida: ella misma si esta vacia, o una
/// subcarpeta con el nombre del proyecto que no pise ninguna que ya este.
fn destino_en(elegida: &Path, nombre: &str) -> io::Result<PathBuf> {
    if std::fs::read_dir(elegida)?.next().is_none() {
        return Ok(elegida.to_path_buf());
    }
    let base = almacen::nombre_seguro(nombre);
    let mut intento = elegida.join(&base);
    let mut n = 2;
    while std::fs::symlink_metadata(&intento).is_ok() {
        intento = elegida.join(format!("{base} ({n})"));
        n += 1;
        if n > 9999 {
            return Err(io::Error::other("demasiadas carpetas con ese nombre"));
        }
    }
    Ok(intento)
}

/// Copia `origen` en `destino`; si falla, se lleva lo copiado y deja
/// `destino` como estaba.
fn copiar_o_limpiar(origen: &Path, destino: &Path, destino_nuevo: bool) -> io::Result<()> {
    let hecho = std::fs::create_dir_all(destino).and_then(|_| copiar_arbol(origen, destino));
    if hecho.is_err() {
        limpiar(destino, destino_nuevo);
    }
    hecho
}

/// Todo lo de `origen` dentro de `destino`, comprobando el tamano de cada
/// fichero: una copia corta no puede pasar por buena, que despues se borra
/// el original.
fn copiar_arbol(origen: &Path, destino: &Path) -> io::Result<()> {
    for e in std::fs::read_dir(origen)? {
        let e = e?;
        let tipo = e.file_type()?;
        let de = e.path();
        let a = destino.join(e.file_name());
        if tipo.is_symlink() {
            // Un enlace dentro de un proyecto no lo pone PixPin; copiarlo o
            // seguirlo podria llevarse otra cosa. Mejor no mover nada.
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("hay un enlace dentro del proyecto: {}", de.display()),
            ));
        }
        if tipo.is_dir() {
            std::fs::create_dir(&a)?;
            copiar_arbol(&de, &a)?;
        } else {
            let copiados = std::fs::copy(&de, &a)?;
            if copiados != e.metadata()?.len() {
                return Err(io::Error::other(format!(
                    "copia incompleta: {}",
                    de.display()
                )));
            }
        }
    }
    Ok(())
}

/// Deshace una copia a medias: la carpeta entera si se creo para esto, o
/// solo lo de dentro si era la elegida (vacia al empezar).
fn limpiar(destino: &Path, destino_nuevo: bool) {
    if destino_nuevo {
        let _ = std::fs::remove_dir_all(destino);
    } else {
        vaciar(destino);
    }
}

fn vaciar(dir: &Path) {
    if let Ok(l) = std::fs::read_dir(dir) {
        for e in l.flatten() {
            let p = e.path();
            // `remove_dir_all` no sigue enlaces; `remove_file` tampoco.
            let _ = if e.file_type().is_ok_and(|t| t.is_dir()) {
                std::fs::remove_dir_all(&p)
            } else {
                std::fs::remove_file(&p)
            };
        }
    }
}

/// Borra el sitio viejo, que ya esta copiado entero en el nuevo. La carpeta
/// en si solo se quita si queda vacia.
fn vaciar_lo_movido(origen: &Path) {
    vaciar(origen);
    let _ = std::fs::remove_dir(origen);
}

fn canonica(p: &Path) -> PathBuf {
    std::fs::canonicalize(p)
        .map(|c| union::sin_prefijo_largo(&c))
        .unwrap_or_else(|_| p.to_path_buf())
}

/// Un `id` es un nombre de carpeta: con barras o puntos sacaria de
/// `proyectos/` lo que se mueve. Los de verdad son letras, cifras y guiones.
fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::almacen::Ficha;

    /// Un almacen y una «carpeta del usuario» fuera de el, en temporales.
    struct Montaje {
        base: PathBuf,
    }

    impl Montaje {
        fn raiz(&self) -> PathBuf {
            self.base.join("PixPinMax")
        }
        fn usuario(&self) -> PathBuf {
            self.base.join("Documentos del usuario")
        }
    }

    impl Drop for Montaje {
        fn drop(&mut self) {
            // Tambien comprueba de paso que limpiar no sigue las uniones: la
            // carpeta del usuario esta fuera de la raiz y se borra aparte.
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn montar(etiqueta: &str) -> Montaje {
        let base = std::env::temp_dir().join(format!(
            "pixpin-ubicacion-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let m = Montaje { base };
        std::fs::create_dir_all(m.usuario()).unwrap();
        m
    }

    /// Un proyecto con cuaderno, un adjunto y un lienzo.
    fn proyecto(raiz: &Path, id: &str, nombre: &str) {
        let mut i = Indice::leer(raiz);
        i.proyectos.push(Ficha {
            id: id.into(),
            nombre: nombre.into(),
            uid: Some("VVT587BFCA".into()),
            creado: 5,
            aparato: Some("K7Q2".into()),
            tocado: 10,
            ..Default::default()
        });
        i.guardar(raiz).unwrap();
        let dir = almacen::carpeta(raiz, id);
        std::fs::create_dir_all(dir.join("archivos")).unwrap();
        std::fs::create_dir_all(dir.join("lienzos")).unwrap();
        std::fs::write(dir.join("guardados.jsonl"), b"{}\n").unwrap();
        std::fs::write(dir.join("archivos").join("plano.png"), b"png").unwrap();
        std::fs::write(dir.join("lienzos").join("a.excalidraw"), b"{}").unwrap();
    }

    fn ficha(raiz: &Path, id: &str) -> Ficha {
        Indice::leer(raiz).buscar(id).cloned().unwrap()
    }

    fn entero(dir: &Path) {
        assert_eq!(std::fs::read(dir.join("guardados.jsonl")).unwrap(), b"{}\n");
        assert_eq!(
            std::fs::read(dir.join("archivos/plano.png")).unwrap(),
            b"png"
        );
        assert_eq!(
            std::fs::read(dir.join("lienzos/a.excalidraw")).unwrap(),
            b"{}"
        );
    }

    #[test]
    fn mover_a_una_carpeta_vacia_lo_deja_alli_y_se_sigue_usando_por_su_sitio_de_siempre() {
        let m = montar("vacia");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra");
        let elegida = m.usuario().join("Obra PixPin");
        std::fs::create_dir(&elegida).unwrap();

        let destino = mover_a(&raiz, "p1", &elegida).unwrap();
        assert_eq!(destino, elegida, "vacia: el proyecto va en ella misma");
        entero(&elegida);
        let enlace = almacen::carpeta(&raiz, "p1");
        assert!(union::es_union(&enlace));
        assert_eq!(
            ficha(&raiz, "p1").ubicacion.as_deref(),
            Some(elegida.to_str().unwrap())
        );
        assert_eq!(
            estado(&raiz, &ficha(&raiz, "p1")),
            Estado::Propia(elegida.clone())
        );
        assert_eq!(carpeta_real(&raiz, "p1"), elegida);

        // Lo que se escribe por `carpeta()` acaba en la del usuario.
        almacen::guardar_adjunto(&raiz, "p1", "nota.txt", b"hola").unwrap();
        assert!(elegida.join("archivos/nota.txt").is_file());

        // Y empaquetar para el movil lo lleva todo a traves de la union.
        let bytes = almacen::empaquetar(&raiz, "p1", 99).unwrap();
        let p = crate::Paquete::desde_bytes(&bytes).expect("un .pixpin legible");
        assert_eq!(p.proyecto.nombre, "Obra");
        assert_eq!(p.entrada("archivos/plano.png"), Some(&b"png"[..]));
        assert_eq!(p.entrada("archivos/nota.txt"), Some(&b"hola"[..]));
        assert_eq!(p.entrada("lienzos/a.excalidraw"), Some(&b"{}"[..]));
        assert_eq!(p.entrada("guardados.jsonl"), Some(&b"{}\n"[..]));
    }

    #[test]
    fn una_carpeta_con_cosas_recibe_una_subcarpeta_con_el_nombre_del_proyecto() {
        let m = montar("subcarpeta");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra: fase 1");
        proyecto(&raiz, "p2", "Obra: fase 1");
        std::fs::write(m.usuario().join("mio.docx"), b"del usuario").unwrap();

        let d1 = mover_a(&raiz, "p1", &m.usuario()).unwrap();
        assert_eq!(d1, m.usuario().join("Obra_ fase 1"));
        entero(&d1);
        // Caso negativo: lo que ya tenia el usuario ni se mueve ni se mezcla.
        assert_eq!(
            std::fs::read(m.usuario().join("mio.docx")).unwrap(),
            b"del usuario"
        );
        assert!(!m.usuario().join("guardados.jsonl").exists());
        // Otro con el mismo nombre no pisa al primero.
        let d2 = mover_a(&raiz, "p2", &m.usuario()).unwrap();
        assert_eq!(d2, m.usuario().join("Obra_ fase 1 (2)"));
        entero(&d1);
        entero(&d2);
    }

    #[test]
    fn una_ruta_de_red_se_rechaza_sin_tocar_nada() {
        let m = montar("red");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra");
        let r = mover_a(&raiz, "p1", Path::new(r"\\servidor\compartida\obra"));
        assert!(matches!(r, Err(Error::Red)), "{r:?}");
        assert_eq!(r.unwrap_err().clave(), "ubicacion-error-red");
        let enlace = almacen::carpeta(&raiz, "p1");
        assert!(!union::es_union(&enlace));
        entero(&enlace);
        assert_eq!(ficha(&raiz, "p1").ubicacion, None);
    }

    #[test]
    fn elegir_una_carpeta_dentro_del_propio_proyecto_o_del_almacen_se_rechaza() {
        let m = montar("dentro");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra");
        let dentro = almacen::carpeta(&raiz, "p1").join("archivos");
        assert!(matches!(mover_a(&raiz, "p1", &dentro), Err(Error::Dentro)));
        assert!(matches!(
            mover_a(&raiz, "p1", &raiz.join("proyectos")),
            Err(Error::Dentro)
        ));
        // Caso negativo: una ruta que no existe tampoco vale.
        assert!(matches!(
            mover_a(&raiz, "p1", &m.usuario().join("no esta")),
            Err(Error::NoEsCarpeta)
        ));
        entero(&almacen::carpeta(&raiz, "p1"));
    }

    // Solo Windows: el fallo se provoca abriendo un fichero sin compartir
    // (`share_mode(0)`), y en Mac/Linux nadie puede impedir que otro lo lea.
    #[cfg(windows)]
    #[test]
    fn un_fallo_a_mitad_de_copiar_no_pierde_nada_y_no_deja_restos() {
        use std::os::windows::fs::OpenOptionsExt;
        let m = montar("fallo");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra");
        let enlace = almacen::carpeta(&raiz, "p1");
        // Otro programa tiene un fichero abierto sin dejar leerlo: la copia
        // falla a mitad, como entre discos con uno que se llena.
        std::fs::write(enlace.join("archivos").join("z-bloqueado.bin"), b"x").unwrap();
        let cerrojo = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(enlace.join("archivos").join("z-bloqueado.bin"))
            .unwrap();

        let r = mover_a_con(&raiz, "p1", &m.usuario(), true);
        assert!(r.is_err(), "{r:?}");
        drop(cerrojo);
        // Todo sigue en su sitio, sin union, sin ruta apuntada...
        assert!(!union::es_union(&enlace));
        entero(&enlace);
        assert!(enlace.join("archivos/z-bloqueado.bin").is_file());
        assert_eq!(ficha(&raiz, "p1").ubicacion, None);
        // ...y en la carpeta del usuario no quedo nada a medias.
        assert_eq!(std::fs::read_dir(m.usuario()).unwrap().count(), 0);

        // Con el fichero ya libre, el mismo traslado sale bien.
        let d = mover_a_con(&raiz, "p1", &m.usuario(), true).unwrap();
        entero(&d);
        assert!(union::es_union(&enlace));
        assert!(!raiz.join("proyectos/p1.moviendo").exists());
    }

    #[test]
    fn volver_a_la_habitual_lo_trae_todo_y_quita_la_union() {
        for forzar in [false, true] {
            let m = montar(&format!("volver-{forzar}"));
            let raiz = m.raiz();
            proyecto(&raiz, "p1", "Obra");
            let d = mover_a_con(&raiz, "p1", &m.usuario(), forzar).unwrap();
            volver_con(&raiz, "p1", forzar).unwrap();
            let enlace = almacen::carpeta(&raiz, "p1");
            assert!(!union::es_union(&enlace));
            entero(&enlace);
            assert_eq!(ficha(&raiz, "p1").ubicacion, None);
            assert_eq!(estado(&raiz, &ficha(&raiz, "p1")), Estado::Habitual);
            assert!(!d.join("guardados.jsonl").exists(), "se movio, no se copio");
            assert!(!raiz.join("proyectos/p1.volviendo").exists());
        }
    }

    #[test]
    fn se_puede_pasar_de_una_carpeta_propia_a_otra() {
        let m = montar("otra");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra");
        let a = m.usuario().join("A");
        let b = m.usuario().join("B");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        mover_a(&raiz, "p1", &a).unwrap();
        // Elegir otra vez la misma no hace nada.
        assert_eq!(mover_a(&raiz, "p1", &a).unwrap(), a);
        entero(&a);
        let d = mover_a_con(&raiz, "p1", &b, true).unwrap();
        assert_eq!(d, b);
        entero(&b);
        assert!(!a.exists() || std::fs::read_dir(&a).unwrap().count() == 0);
        assert_eq!(carpeta_real(&raiz, "p1"), b);
    }

    #[test]
    fn un_proyecto_sin_carpeta_todavia_nace_en_la_elegida() {
        let m = montar("sin-carpeta");
        let raiz = m.raiz();
        let mut i = Indice::default();
        let f = Ficha::nueva("Nuevo", 1, "PC");
        i.proyectos.push(f.clone());
        i.guardar(&raiz).unwrap();
        let d = mover_a(&raiz, &f.id, &m.usuario()).unwrap();
        crate::cuaderno::anadir(
            &almacen::carpeta(&raiz, &f.id),
            &crate::cuaderno::Mensaje::nota(
                "hola",
                &crate::cuaderno::Sello {
                    cuando: 1,
                    numero: 1,
                    aparato: "PC".into(),
                    proyecto: f.id.clone(),
                },
            ),
        )
        .unwrap();
        assert!(d.join("guardados.jsonl").is_file());
    }

    #[test]
    fn borrar_un_proyecto_reubicado_no_toca_la_carpeta_del_usuario() {
        let m = montar("borrar");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra");
        let d = mover_a(&raiz, "p1", &m.usuario()).unwrap();
        let (quitados, sin_mover) =
            almacen::borrar_proyectos(&raiz, &["p1".to_string()], 777).unwrap();
        assert_eq!((quitados, sin_mover.len()), (1, 0));
        // A la papelera fue la union; lo del usuario sigue entero.
        let en = almacen::papelera(&raiz).join("p1-777");
        assert!(union::es_union(&en));
        entero(&d);
        assert!(std::fs::symlink_metadata(almacen::carpeta(&raiz, "p1")).is_err());
        // Se recupera con su ruta...
        let lista = almacen::en_papelera(&raiz);
        assert_eq!(lista.len(), 1);
        assert_eq!(
            lista[0].ficha.ubicacion.as_deref(),
            Some(d.to_str().unwrap())
        );
        almacen::recuperar(&raiz, &lista[0], 800).unwrap();
        entero(&almacen::carpeta(&raiz, "p1"));
        assert_eq!(
            ficha(&raiz, "p1").ubicacion.as_deref(),
            Some(d.to_str().unwrap())
        );
        // ...y purgar la papelera (borrarla entera) no entra en la union.
        almacen::borrar_proyectos(&raiz, &["p1".to_string()], 900).unwrap();
        std::fs::remove_dir_all(almacen::papelera(&raiz)).unwrap();
        entero(&d);
    }

    #[test]
    fn con_la_carpeta_ausente_se_avisa_y_no_se_crea_una_vacia_en_la_habitual() {
        let m = montar("ausente");
        let raiz = m.raiz();
        proyecto(&raiz, "p1", "Obra");
        let elegida = m.usuario().join("USB");
        std::fs::create_dir(&elegida).unwrap();
        let d = mover_a(&raiz, "p1", &elegida).unwrap();
        // El USB se desenchufa (aqui: la carpeta cambia de nombre).
        let fuera = m.usuario().join("USB fuera");
        std::fs::rename(&d, &fuera).unwrap();
        let f = ficha(&raiz, "p1");
        assert!(!disponible(&f));
        assert_eq!(estado(&raiz, &f), Estado::NoDisponible(d.clone()));
        assert!(matches!(preparar(&raiz, &f), Err(Error::NoDisponible(_))));
        // Caso negativo: escribir el primer mensaje falla, no «repara».
        assert!(almacen::guardar_adjunto(&raiz, "p1", "x.txt", b"x").is_err());
        assert!(union::es_union(&almacen::carpeta(&raiz, "p1")));
        // Volver a la habitual sin el disco no deja un proyecto vacio.
        assert!(matches!(
            volver_a_habitual(&raiz, "p1"),
            Err(Error::NoDisponible(_))
        ));
        assert!(union::es_union(&almacen::carpeta(&raiz, "p1")));

        // Si ademas alguien quito la union, preparar la rehace (rota) en vez
        // de dejar el sitio libre para una carpeta vacia.
        union::quitar(&almacen::carpeta(&raiz, "p1")).unwrap();
        assert!(matches!(preparar(&raiz, &f), Err(Error::NoDisponible(_))));
        assert!(union::es_union(&almacen::carpeta(&raiz, "p1")));
        assert!(std::fs::create_dir_all(almacen::carpeta(&raiz, "p1").join("archivos")).is_err());

        // El USB vuelve: todo esta donde estaba.
        std::fs::rename(&fuera, &d).unwrap();
        assert!(disponible(&f));
        preparar(&raiz, &f).unwrap();
        entero(&almacen::carpeta(&raiz, "p1"));
    }

    #[test]
    fn recibir_del_movil_no_le_quita_al_proyecto_su_carpeta() {
        let mut i = Indice::default();
        let mut f = Ficha::nueva("Obra", 1, "K7Q2");
        f.ubicacion = Some(r"D:\Obra".into());
        i.proyectos.push(f.clone());
        let llega = Ficha {
            ubicacion: None,
            nombre: "Obra nueva".into(),
            ..f.clone()
        };
        let (que, id) = i.recibir(llega);
        assert_eq!(que, almacen::Recibido::Actualizado);
        let puesta = i.buscar(&id).unwrap();
        assert_eq!(puesta.nombre, "Obra nueva");
        assert_eq!(puesta.ubicacion.as_deref(), Some(r"D:\Obra"));
        // Y una ficha sin ruta no escribe el campo: los indices de siempre
        // salen igual que antes.
        let texto = serde_json::to_string(&Ficha::nueva("x", 1, "PC")).unwrap();
        assert!(!texto.contains("ubicacion"));
    }
}
