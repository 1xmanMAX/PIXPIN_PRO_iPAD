//! **La mensajeria nativa de Chrome y Edge**: cada mensaje son 4 bytes con su
//! largo (en el orden de bytes del equipo, que en Windows es siempre el de
//! Intel) y despues el JSON en UTF-8.
//!
//! Los topes son los del navegador: de la extension al programa, hasta 64 MB;
//! del programa a la extension, 1 MB. El de entrada lo bajamos a
//! [`TOPE_ENTRADA`]: una captura de pagina entera en PNG cabe de sobra, y un
//! largo mayor es un mensaje roto o malicioso que no vale la pena ni leer.
//! El largo se mira ANTES de reservar memoria.

use std::io::{self, Read, Write};

/// Lo mas que se acepta de la extension.
pub const TOPE_ENTRADA: usize = 48 * 1024 * 1024;

/// Lo mas que el navegador acepta de vuelta.
pub const TOPE_SALIDA: usize = 1024 * 1024;

/// Por que no se pudo leer un mensaje.
#[derive(Debug, PartialEq, Eq)]
pub enum ErrorLectura {
    /// La entrada se cerro sin nada: el navegador no mando ningun mensaje.
    Fin,
    /// Llegaron menos bytes de los que decia el largo (o del largo mismo).
    Corto {
        esperados: usize,
        llegados: usize,
    },
    /// El largo pasa del tope: no se lee.
    Enorme(usize),
    /// Un largo de cero no es un JSON.
    Vacio,
    Io(String),
}

/// Lee el largo y el mensaje. No reserva nada antes de mirar el tope.
pub fn leer_mensaje(r: &mut impl Read, tope: usize) -> Result<Vec<u8>, ErrorLectura> {
    let mut largo = [0u8; 4];
    let n = leer_todo(r, &mut largo)?;
    if n == 0 {
        return Err(ErrorLectura::Fin);
    }
    if n < 4 {
        return Err(ErrorLectura::Corto {
            esperados: 4,
            llegados: n,
        });
    }
    let largo = u32::from_ne_bytes(largo) as usize;
    if largo == 0 {
        return Err(ErrorLectura::Vacio);
    }
    if largo > tope {
        return Err(ErrorLectura::Enorme(largo));
    }
    let mut datos = vec![0u8; largo];
    let n = leer_todo(r, &mut datos)?;
    if n < largo {
        return Err(ErrorLectura::Corto {
            esperados: largo,
            llegados: n,
        });
    }
    Ok(datos)
}

/// Como `read_exact`, pero dice cuanto llego si se acaba antes.
fn leer_todo(r: &mut impl Read, buf: &mut [u8]) -> Result<usize, ErrorLectura> {
    let mut hecho = 0;
    while hecho < buf.len() {
        match r.read(&mut buf[hecho..]) {
            Ok(0) => break,
            Ok(n) => hecho += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(ErrorLectura::Io(e.to_string())),
        }
    }
    Ok(hecho)
}

/// Escribe un mensaje con su largo delante y vacia la salida (el navegador
/// espera el mensaje entero antes de cerrar el puente).
pub fn escribir_mensaje(w: &mut impl Write, json: &[u8]) -> io::Result<()> {
    if json.len() > TOPE_SALIDA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "la respuesta pasa de 1 MB",
        ));
    }
    w.write_all(&(json.len() as u32).to_ne_bytes())?;
    w.write_all(json)?;
    w.flush()
}
