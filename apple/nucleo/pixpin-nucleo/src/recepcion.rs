//! **Lo que llega con un codigo, a su sitio** (la parte de guardar de
//! `recibir.rs` de Windows, puerto de `Recepcion` del movil). Es lo mismo
//! para quien espera con su QR y para quien llama con un codigo, y por eso
//! esta aqui una sola vez:
//!
//! - **Un proyecto** entra en la lista de proyectos: encima del que ya hay
//!   si es el mismo (los tres codigos) y se eligio «Actualizar el que tengo»,
//!   o aparte si se eligio «Crear como nuevo». Antes de escribir encima, la
//!   copia de seguridad; si la copia no se puede hacer, tampoco se escribe.
//! - **Un lienzo suelto**, a su proyecto ([`crate::lienzo_suelto::guardar`]).
//! - **Lo demas**, a «Mensajes guardados», como lo que es por dentro (un
//!   `informe.bin` que es un PDF entra como `informe.pdf`). **Lo que vuelve**
//!   (un archivo que ya esta aqui con sus tres codigos) se pone al dia en su
//!   misma burbuja en vez de duplicarse.
//!
//! [`que_sustituye`] dice, antes de aceptar, que de lo que llega ya se tiene:
//! es lo que hace que salgan «Actualizar el que tengo» y «Crear como nuevo».

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen::{self, Ficha};
use pixpin_proyecto::{cuaderno, vista};
use pixpin_sincro::envio::{self, Elemento};

use crate::lienzo_suelto;

/// Donde quedo una cosa recibida, para decirlo en «Recibido de X».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Donde {
    /// Entro en la lista de proyectos.
    Proyecto,
    /// Ya estaba y se puso al dia el que habia.
    ProyectoAlDia,
    /// Un archivo suelto, en «Mensajes guardados».
    Guardados,
    /// Ya estaba en un chat con sus tres codigos y se puso al dia alli.
    GuardadosAlDia,
    /// Llego, pero solo quedo en la carpeta `recibidos/`.
    Carpeta,
}

impl Donde {
    /// El texto de [`crate::RecibidoEnvio::donde`].
    pub fn clave(self) -> &'static str {
        match self {
            Donde::Proyecto => "proyecto",
            Donde::ProyectoAlDia => "proyecto_al_dia",
            Donde::Guardados => "guardados",
            Donde::GuardadosAlDia => "guardados_al_dia",
            Donde::Carpeta => "carpeta",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guardado {
    pub nombre: String,
    pub donde: Donde,
    /// La ficha del chat donde quedo (vacio si solo en la carpeta).
    pub chat: String,
}

/// Que hacer con cada cosa que llega y ya se tiene, por su `identidad`:
/// `true` es «crear como nuevo» y lo que no esta en la tabla, «actualizar el
/// que tengo», que es lo que viene puesto en el movil.
pub type ComoNuevo = BTreeMap<String, bool>;

/// La ficha que describe un proyecto que llega, solo con sus tres codigos:
/// es lo unico que hace falta para saber si aqui ya esta esa misma cosa.
///
/// El `id` sale de la identidad (`proyecto:<id>`) porque un proyecto de antes
/// de los codigos no trae `uid`, y entonces el codigo unico se saca de su id,
/// igual que en Android.
pub fn ficha_que_llega(e: &Elemento) -> Ficha {
    Ficha {
        id: e
            .identidad
            .strip_prefix("proyecto:")
            .unwrap_or(&e.identidad)
            .to_string(),
        nombre: e.nombre.clone(),
        uid: e.uid.clone(),
        creado: e.creado,
        aparato: e.aparato.clone(),
        ..Default::default()
    }
}

/// Si un mensaje que ya esta aqui es esa misma cosa que llega: los tres
/// codigos iguales (`Codigos.mismos` del movil). Sin codigo de chat no se
/// puede afirmar, y ante la duda se duplica en vez de pisar.
fn mismo_mensaje(m: &cuaderno::Mensaje, e: &Elemento) -> bool {
    m.uid.is_some()
        && m.uid == e.uid
        && m.codigo_chat().is_some()
        && m.codigo_chat() == e.codigo_de_chat
        && m.cuando == e.creado
}

/// **Lo que aqui ES cada cosa que llega**, por su nombre, para poder ofrecer
/// ponerlo al dia (`Recepcion.queSustituye` del movil).
///
/// Va por los tres codigos y nada mas: mismo nombre no es la misma cosa, y
/// dos proyectos que se llamen «Tesis» en dos aparatos distintos son dos
/// proyectos. Lo que no coincide en los tres no sale aqui y entra aparte.
pub fn que_sustituye(raiz: &Path, elementos: &[Elemento]) -> BTreeMap<String, String> {
    let indice = almacen::Indice::leer(raiz);
    let mut salida = BTreeMap::new();
    for e in elementos {
        let nombre = if e.tipo == envio::PROYECTO {
            indice.misma(&ficha_que_llega(e)).map(|f| f.nombre.clone())
        } else {
            mensaje_que_sustituye(raiz, &indice, e).map(|(_, _, m)| {
                if m.nombre.trim().is_empty() {
                    e.nombre.clone()
                } else {
                    m.nombre.clone()
                }
            })
        };
        if let Some(n) = nombre {
            salida.insert(e.identidad.clone(), n);
        }
    }
    salida
}

/// El mensaje de cualquier chat de este aparato con los tres codigos de lo
/// que llega: en que proyecto esta, en que linea del cuaderno y cual es.
fn mensaje_que_sustituye(
    raiz: &Path,
    indice: &almacen::Indice,
    e: &Elemento,
) -> Option<(String, usize, cuaderno::Mensaje)> {
    // Sin codigo unico o sin fecha no hay tres codigos que comparar.
    if e.uid.is_none() || e.creado <= 0 {
        return None;
    }
    for f in &indice.proyectos {
        let carpeta = almacen::carpeta(raiz, &f.id);
        let Ok(cuaderno) = cuaderno::Cuaderno::leer_de(&carpeta) else {
            continue;
        };
        if let Some((i, m)) = cuaderno
            .mensajes
            .iter()
            .enumerate()
            .find(|(_, m)| mismo_mensaje(m, e))
        {
            return Some((f.id.clone(), i, m.clone()));
        }
    }
    None
}

/// El nombre con el que se guarda algo que llega, ya con su extension: un
/// proyecto o un lienzo es un `.pixpin` aunque el movil lo mande sin ella
/// (el primer envio de prueba llego como `123` siendo un proyecto entero).
pub fn nombre_con_extension(e: &Elemento) -> String {
    let nombre = envio::nombre_sano(&e.nombre);
    if e.tipo != envio::PROYECTO && e.tipo != envio::LIENZO {
        return nombre;
    }
    if nombre.to_ascii_lowercase().ends_with(".pixpin") {
        nombre
    } else {
        format!("{nombre}.pixpin")
    }
}

/// **Lo que llego, a su sitio**: los proyectos a la lista, los lienzos a su
/// proyecto, lo suelto a «Mensajes guardados». `id` es el de este aparato.
pub fn guardar_lo_recibido(
    raiz: &Path,
    id: &str,
    cosas: &[(Elemento, PathBuf)],
    como_nuevo: &ComoNuevo,
    ahora: i64,
) -> Vec<Guardado> {
    // El codigo CORTO del aparato (`6ARJ`), no su identificador entero: es
    // lo que va en el codigo de chat de cada mensaje.
    let aparato = pixpin_proyecto::codigos::de_aparato(id);
    let mut guardados = Vec::new();
    // Un proyecto entra en la LISTA de proyectos, no como adjunto: un
    // `.pixpin` es una conversacion entera, y cada chat es un proyecto.
    for (e, r) in cosas.iter().filter(|(e, _)| e.tipo == envio::PROYECTO) {
        let con_extension = r.with_file_name(nombre_con_extension(e));
        let ruta = if con_extension != *r && std::fs::rename(r, &con_extension).is_ok() {
            con_extension
        } else {
            r.clone()
        };
        let nuevo = como_nuevo.get(&e.identidad).copied().unwrap_or(false);
        let hecho = pixpin_proyecto::Paquete::abrir(&ruta)
            .map_err(|e| e.to_string())
            .and_then(|p| {
                guardar_un_proyecto(raiz, e, p, &aparato, nuevo, ahora).map_err(|x| x.to_string())
            });
        guardados.push(match hecho {
            Ok((ficha, al_dia)) => Guardado {
                nombre: e.nombre.clone(),
                donde: if al_dia {
                    Donde::ProyectoAlDia
                } else {
                    Donde::Proyecto
                },
                chat: ficha.id,
            },
            Err(_) => a_la_carpeta(raiz, e, &ruta),
        });
    }
    // **Un lienzo suelto, a su proyecto** (`Recepcion.guardarLienzo` del
    // movil): llega como un `.pixpin` de una hoja.
    let mut resto = Vec::new();
    for (e, r) in cosas.iter().filter(|(e, _)| e.tipo != envio::PROYECTO) {
        if e.tipo != envio::LIENZO {
            resto.push((e.clone(), r.clone()));
            continue;
        }
        let nuevo = como_nuevo.get(&e.identidad).copied().unwrap_or(false);
        let motivo = format!("Antes de recibir «{}»", e.nombre);
        let hecho = lienzo_suelto::guardar(raiz, e, r, &aparato, nuevo, ahora, &|ficha| {
            copia_antes_de_tocar(raiz, ficha, &motivo, ahora)
        });
        match hecho {
            Ok(llegada) => {
                let (donde, chat) = match llegada {
                    lienzo_suelto::Llegada::AlDia(f) => (Donde::ProyectoAlDia, f),
                    lienzo_suelto::Llegada::Anadido(f)
                    | lienzo_suelto::Llegada::ProyectoNuevo(f) => (Donde::Proyecto, f),
                };
                guardados.push(Guardado {
                    nombre: e.nombre.clone(),
                    donde,
                    chat,
                });
            }
            // Si no se entiende como lienzo, al menos queda en el chat.
            Err(_) => resto.push((e.clone(), r.clone())),
        }
    }
    // Y lo suelto, al cuaderno, para que se vea en el chat.
    match al_cuaderno(raiz, &aparato, &resto, como_nuevo, ahora) {
        Ok(sueltos) => guardados.extend(sueltos),
        Err(_) => {
            for (e, r) in &resto {
                guardados.push(a_la_carpeta(raiz, e, r));
            }
        }
    }
    guardados
}

/// Lo que no se pudo poner en su sitio no se pierde: queda en `recibidos/`
/// (el temporal de la recepcion se borra al acabar).
fn a_la_carpeta(raiz: &Path, e: &Elemento, ruta: &Path) -> Guardado {
    let carpeta = raiz.join("recibidos");
    let destino = envio::ruta_libre(&carpeta, &nombre_con_extension(e));
    let _ = std::fs::create_dir_all(&carpeta).and_then(|_| {
        std::fs::rename(ruta, &destino).or_else(|_| std::fs::copy(ruta, &destino).map(|_| ()))
    });
    Guardado {
        nombre: e.nombre.clone(),
        donde: Donde::Carpeta,
        chat: String::new(),
    }
}

/// Un proyecto que llega, a su sitio: encima del que ya hay si el usuario
/// eligio «actualizar el que tengo», o aparte si eligio «crear como nuevo».
/// Devuelve la ficha y si fue lo primero.
///
/// **Antes de escribir encima, la copia de seguridad** (`Copias` del movil,
/// 15-sep-2026): el usuario perdio lienzos de «Tesis» al recibir un envio.
/// Si la copia no se puede hacer, tampoco se escribe.
pub fn guardar_un_proyecto(
    raiz: &Path,
    e: &Elemento,
    paquete: pixpin_proyecto::Paquete,
    aparato: &str,
    como_nuevo: bool,
    ahora: i64,
) -> std::io::Result<(Ficha, bool)> {
    let mismo = if como_nuevo {
        None
    } else {
        almacen::Indice::leer(raiz)
            .misma(&ficha_que_llega(e))
            .map(|f| f.id.clone())
    };
    let Some(id) = mismo else {
        let p = if como_nuevo {
            almacen::renovar(&paquete, ahora)
        } else {
            paquete
        };
        return almacen::importar_paquete(raiz, &p, aparato).map(|f| (f, false));
    };
    copia_antes_de_tocar(
        raiz,
        &id,
        &format!("Antes de recibir «{}»", e.nombre),
        ahora,
    )?;
    almacen::actualizar_paquete(raiz, &id, &paquete).map(|f| (f, true))
}

/// Guarda como esta un chat antes de que un envio lo pise
/// (`pixpin_sincro::copias`, puerto de `sincro/Copias.kt`).
pub fn copia_antes_de_tocar(
    raiz: &Path,
    ficha: &str,
    motivo: &str,
    ahora: i64,
) -> std::io::Result<()> {
    let Some(chat) = vista::chat_de_ficha(raiz, ficha) else {
        return Ok(());
    };
    let disco = vista::DiscoPc::nuevo(raiz);
    pixpin_sincro::copias::hacer(&disco, &chat, motivo, ahora).map(|_| ())
}

/// Mete en el cuaderno de «Mensajes guardados» lo que llego y no es un
/// proyecto. **Lo que ya esta aqui con los tres codigos** se pone al dia en
/// su sitio, sin crear otra burbuja.
pub fn al_cuaderno(
    raiz: &Path,
    aparato: &str,
    cosas: &[(Elemento, PathBuf)],
    como_nuevo: &ComoNuevo,
    ahora: i64,
) -> std::io::Result<Vec<Guardado>> {
    let sueltos: Vec<_> = cosas
        .iter()
        .filter(|(e, _)| e.tipo != envio::PROYECTO)
        .collect();
    if sueltos.is_empty() {
        return Ok(Vec::new());
    }
    let ficha = almacen::asegurar_guardados(raiz, ahora, aparato)?;
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    let previos = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    // El numero sigue al ultimo del cuaderno: es el que ordena la
    // conversacion y el que entra en el codigo de chat.
    let mut numero = previos.mensajes.iter().map(|m| m.numero).max().unwrap_or(0);
    let indice = almacen::Indice::leer(raiz);
    let mut salida = Vec::new();
    for (elemento, ruta) in sueltos {
        let nuevo = como_nuevo
            .get(&elemento.identidad)
            .copied()
            .unwrap_or(false);
        let antes = (!nuevo)
            .then(|| mensaje_que_sustituye(raiz, &indice, elemento))
            .flatten();
        let bytes = std::fs::read(ruta)?;
        // Un `informe.bin` (o un `informe` sin extension) que por dentro es
        // un PDF o una imagen se guarda como lo que es (Android v0.108.0).
        let nombre = envio::nombre_por_contenido(
            &nombre_con_extension(elemento),
            &bytes[..bytes.len().min(16)],
        );
        let (donde, chat) = match antes {
            Some((suyo, _, viejo)) => {
                copia_antes_de_tocar(raiz, &suyo, &format!("Antes de recibir «{nombre}»"), ahora)?;
                poner_al_dia(raiz, &suyo, &viejo, &nombre, &bytes)?;
                (Donde::GuardadosAlDia, suyo)
            }
            None => {
                numero += 1;
                let relativa = almacen::guardar_adjunto(raiz, &ficha.id, &nombre, &bytes)?;
                let cuando = match (nuevo, elemento.creado) {
                    // «Crear como nuevo» estrena tambien la fecha: es otra
                    // cosa desde ahora.
                    (false, c) if c > 0 => c,
                    _ => ahora,
                };
                let mut m = cuaderno::Mensaje::adjunto(
                    cuaderno::clase_de_nombre(&nombre),
                    &nombre,
                    &relativa,
                    bytes.len() as i64,
                    &cuaderno::Sello {
                        cuando,
                        numero,
                        aparato: aparato.to_string(),
                        proyecto: ficha.id.clone(),
                    },
                );
                // Los tres codigos que traia se conservan: son con lo que se
                // sabra despues si esto que llego es lo mismo que ya habia.
                // Al crear como nuevo NO, que para eso se eligio.
                if !nuevo {
                    m.uid = elemento.uid.clone();
                    m.aparato = elemento.aparato.clone().or(Some(aparato.to_string()));
                }
                cuaderno::anadir(&carpeta, &m)?;
                (Donde::Guardados, ficha.id.clone())
            }
        };
        salida.push(Guardado {
            nombre: elemento.nombre.clone(),
            donde,
            chat,
        });
    }
    // La ficha sube en la lista, como con cualquier mensaje nuevo.
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
        f.tocado = ahora;
        let _ = indice.guardar(raiz);
    }
    Ok(salida)
}

/// Escribe lo que llega **encima del archivo que ya tenia** ese mensaje, sin
/// moverlo: asi el chat, que senala esa ruta, ensena lo nuevo en la misma
/// burbuja (`Recepcion.guardarArchivo` del movil cuando `antes != null`).
fn poner_al_dia(
    raiz: &Path,
    ficha: &str,
    viejo: &cuaderno::Mensaje,
    nombre: &str,
    bytes: &[u8],
) -> std::io::Result<()> {
    let carpeta = almacen::carpeta(raiz, ficha);
    let relativa = match viejo.ruta.as_deref().filter(|r| !r.is_empty()) {
        Some(r) => {
            let destino = carpeta.join(r);
            if let Some(padre) = destino.parent() {
                std::fs::create_dir_all(padre)?;
            }
            std::fs::write(&destino, bytes)?;
            r.to_string()
        }
        // Un mensaje sin archivo (una nota) que ahora llega con uno: se le
        // guarda al lado en vez de perderlo.
        None => almacen::guardar_adjunto(raiz, ficha, nombre, bytes)?,
    };
    let puesto = cuaderno::Mensaje {
        ruta: Some(relativa),
        bytes: bytes.len() as i64,
        ..viejo.clone()
    };
    cuaderno::reemplazar(&carpeta, &puesto).map(|_| ())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta_temporal(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-recepcion-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Un `.pixpin` como el que manda el movil, con sus tres codigos.
    fn paquete(nombre: &str, uid: &str, creado: i64) -> pixpin_proyecto::Paquete {
        let proyecto = pixpin_proyecto::Proyecto {
            id: "p-movil".into(),
            nombre: nombre.into(),
            hojas: vec![pixpin_proyecto::Hoja {
                id: "h1".into(),
                nombre: "Plano".into(),
                dibujo: Some("d1".into()),
                uid: Some("AAAAAAAAAA".into()),
                ..Default::default()
            }],
            tocado: creado,
            uid: Some(uid.into()),
            creado,
            aparato: Some("K7Q2".into()),
            ..Default::default()
        };
        let mut p = pixpin_proyecto::Paquete::nuevo(Default::default(), proyecto);
        p.poner_entrada("lienzos/d1.excalidraw", b"{}".to_vec());
        pixpin_proyecto::Paquete::desde_bytes(&p.a_bytes().unwrap()).unwrap()
    }

    fn elemento_de_proyecto(nombre: &str, uid: &str, creado: i64) -> Elemento {
        Elemento {
            tipo: envio::PROYECTO.into(),
            nombre: nombre.into(),
            bytes: 10,
            mime: None,
            identidad: "proyecto:p-movil".into(),
            proyecto: None,
            proyecto_nombre: None,
            creado,
            uid: Some(uid.into()),
            codigo_de_chat: None,
            aparato: Some("K7Q2".into()),
        }
    }

    #[test]
    fn un_proyecto_que_vuelve_del_movil_se_reconoce_y_uno_parecido_no() {
        let raiz = carpeta_temporal("sustituye");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        almacen::importar_paquete(&raiz, &p, "ZZZZ").unwrap();

        let vuelve = elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let hay = que_sustituye(&raiz, std::slice::from_ref(&vuelve));
        assert_eq!(
            hay.get("proyecto:p-movil").map(String::as_str),
            Some("Tesis")
        );

        // Casos negativos: mismo nombre no es la misma cosa. Con otro codigo
        // unico, otra fecha u otro aparato entra aparte y no pisa nada.
        for otro in [
            elemento_de_proyecto("Tesis", "2222222222", 1_757_939_357_123),
            elemento_de_proyecto("Tesis", "VVT587BFCA", 1),
            Elemento {
                aparato: Some("OTRO".into()),
                ..elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123)
            },
        ] {
            assert!(
                que_sustituye(&raiz, std::slice::from_ref(&otro)).is_empty(),
                "{otro:?} no es lo mismo"
            );
        }
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn actualizar_pone_al_dia_el_que_habia_y_deja_copia_de_como_estaba() {
        let raiz = carpeta_temporal("actualizar");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let antes = almacen::importar_paquete(&raiz, &p, "ZZZZ").unwrap();

        let e = elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let vuelve = paquete("Tesis al día", "VVT587BFCA", 1_757_939_357_123);
        let (ficha, al_dia) =
            guardar_un_proyecto(&raiz, &e, vuelve, "ZZZZ", false, 1_757_939_400_000).unwrap();
        assert!(al_dia);
        assert_eq!(ficha.id, antes.id, "el proyecto de aqui conserva su id");
        assert_eq!(ficha.nombre, "Tesis al día");
        assert_eq!(
            almacen::Indice::leer(&raiz).proyectos.len(),
            1,
            "no duplica"
        );

        // Y queda como estaba antes, en «copias», que es lo que pidio el
        // usuario tras perder lienzos: poder volver.
        let chat = vista::chat_de_ficha(&raiz, &ficha.id).unwrap();
        let disco = vista::DiscoPc::nuevo(&raiz);
        let copias = pixpin_sincro::copias::lista(&disco, &chat);
        assert_eq!(copias.len(), 1, "una copia de antes de recibir");
        assert!(
            copias[0].motivo.contains("Antes de recibir"),
            "{:?}",
            copias[0].motivo
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn crear_como_nuevo_deja_los_dos_y_no_toca_el_que_habia() {
        let raiz = carpeta_temporal("como-nuevo");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let antes = almacen::importar_paquete(&raiz, &p, "ZZZZ").unwrap();

        let e = elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let vuelve = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let (ficha, al_dia) =
            guardar_un_proyecto(&raiz, &e, vuelve, "ZZZZ", true, 1_757_939_400_000).unwrap();
        assert!(!al_dia);
        assert_ne!(ficha.id, antes.id);
        assert!(!ficha.misma_que(&antes), "estrena codigos y conviven");
        assert_eq!(almacen::Indice::leer(&raiz).proyectos.len(), 2);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    fn elemento_suelto(nombre: &str) -> Elemento {
        Elemento {
            tipo: "archivo".into(),
            mime: Some("application/octet-stream".into()),
            identidad: format!("archivo:{nombre}"),
            uid: None,
            aparato: None,
            ..elemento_de_proyecto(nombre, "x", 0)
        }
    }

    #[test]
    fn un_bin_que_es_un_pdf_o_una_foto_entra_al_chat_como_lo_que_es() {
        let raiz = carpeta_temporal("bin");
        std::fs::create_dir_all(&raiz).unwrap();
        let pdf = raiz.join("llegado-1");
        std::fs::write(&pdf, b"%PDF-1.4 informe").unwrap();
        let foto = raiz.join("llegado-2");
        std::fs::write(&foto, [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]).unwrap();
        let otro = raiz.join("llegado-3");
        std::fs::write(&otro, b"\0\0\0\0 vete a saber").unwrap();
        let cosas = vec![
            (elemento_suelto("informe.bin"), pdf),
            (elemento_suelto("Fachada"), foto),
            (elemento_suelto("datos.bin"), otro),
        ];
        al_cuaderno(&raiz, "ZZZZ", &cosas, &ComoNuevo::new(), 5).unwrap();
        let ficha = almacen::asegurar_guardados(&raiz, 0, "ZZZZ").unwrap();
        let mensajes = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, &ficha.id))
            .unwrap()
            .mensajes;
        let nombres: Vec<&str> = mensajes.iter().map(|m| m.nombre.as_str()).collect();
        assert_eq!(nombres, ["informe.pdf", "Fachada.jpg", "datos.bin"]);
        // La foto es una foto en el chat, no un archivo cualquiera.
        assert_eq!(mensajes[1].clase, Some(cuaderno::Clase::Imagen));
        // Y el fichero guardado lleva la extension buena.
        assert!(
            mensajes[0]
                .ruta
                .as_deref()
                .is_some_and(|r| r.ends_with(".pdf")),
            "{:?}",
            mensajes[0].ruta
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// **Un archivo que vuelve** (mandado desde aqui, editado en el movil y
    /// devuelto) se pone al dia en su burbuja en vez de duplicarse; con
    /// «crear como nuevo», entra aparte.
    #[test]
    fn un_archivo_que_vuelve_se_pone_al_dia_en_su_burbuja() {
        let raiz = carpeta_temporal("vuelve");
        std::fs::create_dir_all(&raiz).unwrap();
        let primero = raiz.join("llegado-1");
        std::fs::write(&primero, b"%PDF-1.4 v1").unwrap();
        let mut e = elemento_suelto("plano.pdf");
        e.uid = Some("UUUUUUUUUU".into());
        e.aparato = Some("K7Q2".into());
        e.creado = 1_757_000_000_000;
        let hecho =
            al_cuaderno(&raiz, "ZZZZ", &[(e.clone(), primero)], &ComoNuevo::new(), 5).unwrap();
        assert_eq!(hecho[0].donde, Donde::Guardados);
        let ficha = almacen::asegurar_guardados(&raiz, 0, "ZZZZ").unwrap();
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        let m = cuaderno::Cuaderno::leer_de(&carpeta).unwrap().mensajes[0].clone();
        // Lo que ira de vuelta con el: sus tres codigos.
        e.codigo_de_chat = m.codigo_chat();
        assert!(e.codigo_de_chat.is_some());

        let hay = que_sustituye(&raiz, std::slice::from_ref(&e));
        assert_eq!(hay.get(&e.identidad).map(String::as_str), Some("plano.pdf"));
        let segundo = raiz.join("llegado-2");
        std::fs::write(&segundo, b"%PDF-1.4 v2").unwrap();
        let hecho =
            al_cuaderno(&raiz, "ZZZZ", &[(e.clone(), segundo)], &ComoNuevo::new(), 6).unwrap();
        assert_eq!(hecho[0].donde, Donde::GuardadosAlDia);
        let mensajes = cuaderno::Cuaderno::leer_de(&carpeta).unwrap().mensajes;
        assert_eq!(mensajes.len(), 1, "la misma burbuja");
        let ruta = carpeta.join(mensajes[0].ruta.as_deref().unwrap());
        assert_eq!(std::fs::read(ruta).unwrap(), b"%PDF-1.4 v2");

        // Caso negativo: «crear como nuevo» no toca la que habia.
        let tercero = raiz.join("llegado-3");
        std::fs::write(&tercero, b"%PDF-1.4 v3").unwrap();
        let nuevo: ComoNuevo = [(e.identidad.clone(), true)].into();
        let hecho = al_cuaderno(&raiz, "ZZZZ", &[(e, tercero)], &nuevo, 7).unwrap();
        assert_eq!(hecho[0].donde, Donde::Guardados);
        assert_eq!(
            cuaderno::Cuaderno::leer_de(&carpeta)
                .unwrap()
                .mensajes
                .len(),
            2
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_que_no_se_entiende_queda_en_recibidos_y_no_se_pierde() {
        let raiz = carpeta_temporal("carpeta");
        std::fs::create_dir_all(&raiz).unwrap();
        let roto = raiz.join("llegado");
        std::fs::write(&roto, b"no es un zip").unwrap();
        let e = elemento_de_proyecto("Roto", "VVT587BFCA", 1);
        let g = guardar_lo_recibido(
            &raiz,
            "aparato-de-prueba",
            &[(e, roto)],
            &ComoNuevo::new(),
            5,
        );
        assert_eq!(g[0].donde, Donde::Carpeta);
        assert!(raiz.join("recibidos/Roto.pixpin").is_file());
        assert!(almacen::Indice::leer(&raiz).proyectos.is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
