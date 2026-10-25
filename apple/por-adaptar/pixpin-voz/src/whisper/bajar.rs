//! **Bajar un fichero por HTTPS con lo que ya trae Windows** (WinHTTP).
//!
//! Solo se usa para el modelo de Whisper, una vez. WinHTTP y no un cliente
//! HTTP en Rust: viene con el sistema, usa los certificados y el proxy de
//! Windows, sigue solo las redirecciones (Hugging Face manda cada fichero a
//! su CDN) y no anade ni un crate. `URLDownloadToFileW` seria aun mas corto,
//! pero para saber por donde va pide implementar una interfaz COM entera;
//! con WinHTTP el avance sale de contar lo que se lee.
//!
//! Se escribe en `<nombre>.parte` y se renombra al acabar, como el movil
//! (`MotorWhisper.asegurarModelo`): un corte a medias nunca deja un fichero
//! con el nombre bueno y el contenido roto.
//!
//! Este modulo habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::ffi::c_void;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_OPEN_REQUEST_FLAGS,
    WINHTTP_QUERY_CONTENT_LENGTH, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::ErrorVoz;

/// Un asa de WinHTTP que se cierra sola.
struct Asa(*mut c_void);

impl Drop for Asa {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: el asa la devolvio WinHTTP, es de este `Asa` y no se
            // ha cerrado antes.
            let _ = unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

/// `https://servidor/ruta` en sus dos piezas. Solo HTTPS: el modelo no se
/// baja nunca en claro.
pub fn partir_url(url: &str) -> Option<(&str, &str)> {
    let resto = url.strip_prefix("https://")?;
    let (servidor, ruta) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    (!servidor.is_empty() && !servidor.contains(':')).then_some((servidor, ruta))
}

/// El fichero temporal mientras se baja.
pub fn ruta_parte(destino: &Path) -> PathBuf {
    let mut nombre = destino.file_name().unwrap_or_default().to_os_string();
    nombre.push(".parte");
    destino.with_file_name(nombre)
}

fn fallo(que: &str, detalle: impl std::fmt::Display) -> ErrorVoz {
    ErrorVoz::Descarga {
        que: que.to_string(),
        detalle: detalle.to_string(),
    }
}

/// **Baja `url` a `destino`.** `avance` recibe lo leido y, si el servidor
/// lo dice, el total. Devuelve los bytes escritos.
///
/// `cancelado` se mira en cada trozo de 64 KB: cerrar el chat corta la
/// descarga en el acto y borra el `.parte`.
pub fn bajar(
    url: &str,
    destino: &Path,
    avance: &mut dyn FnMut(u64, Option<u64>),
    cancelado: &AtomicBool,
) -> Result<u64, ErrorVoz> {
    let nombre = destino
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (servidor, ruta) = partir_url(url).ok_or_else(|| fallo(&nombre, "no es una URL https"))?;
    let parte = ruta_parte(destino);
    let salida = bajar_a(servidor, ruta, &parte, &nombre, avance, cancelado);
    match salida {
        Ok(n) => {
            // `rename` no pisa en todos los casos en Windows: se quita antes
            // el viejo si lo hay (un fichero de cero bytes de un intento
            // anterior).
            let _ = std::fs::remove_file(destino);
            std::fs::rename(&parte, destino).map_err(|e| fallo(&nombre, e))?;
            Ok(n)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&parte);
            Err(e)
        }
    }
}

fn bajar_a(
    servidor: &str,
    ruta: &str,
    parte: &Path,
    nombre: &str,
    avance: &mut dyn FnMut(u64, Option<u64>),
    cancelado: &AtomicBool,
) -> Result<u64, ErrorVoz> {
    if cancelado.load(Ordering::Relaxed) {
        return Err(ErrorVoz::Cancelada);
    }
    // SAFETY: todas las cadenas son literales o `HSTRING` vivos durante
    // cada llamada; cada asa se envuelve en `Asa` en cuanto existe, asi que
    // se cierra en todos los caminos y en orden inverso (peticion, conexion,
    // sesion), que es el que pide WinHTTP.
    unsafe {
        let mut sesion = Asa(WinHttpOpen(
            w!("PixPin"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if sesion.0.is_null() {
            // El proxy automatico es de Windows 8.1 en adelante.
            sesion = Asa(WinHttpOpen(
                w!("PixPin"),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ));
        }
        if sesion.0.is_null() {
            return Err(fallo(nombre, windows::core::Error::from_thread()));
        }
        // Resolver, conectar y enviar con 30 s; recibir, 60 s entre trozo y
        // trozo (como el movil): una red lenta sigue, una muerta se nota.
        let _ = WinHttpSetTimeouts(sesion.0, 30_000, 30_000, 30_000, 60_000);
        let conexion = Asa(WinHttpConnect(
            sesion.0,
            &HSTRING::from(servidor),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ));
        if conexion.0.is_null() {
            return Err(fallo(nombre, windows::core::Error::from_thread()));
        }
        let peticion = Asa(WinHttpOpenRequest(
            conexion.0,
            w!("GET"),
            &HSTRING::from(ruta),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_OPEN_REQUEST_FLAGS(WINHTTP_FLAG_SECURE.0),
        ));
        if peticion.0.is_null() {
            return Err(fallo(nombre, windows::core::Error::from_thread()));
        }
        WinHttpSendRequest(peticion.0, None, None, 0, 0, 0).map_err(|e| fallo(nombre, e))?;
        WinHttpReceiveResponse(peticion.0, std::ptr::null_mut()).map_err(|e| fallo(nombre, e))?;

        let numero = |que: u32| -> Option<u64> {
            let mut n: u32 = 0;
            let mut largo = std::mem::size_of::<u32>() as u32;
            WinHttpQueryHeaders(
                peticion.0,
                que | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&mut n as *mut u32).cast()),
                &mut largo,
                std::ptr::null_mut(),
            )
            .ok()
            .map(|_| n as u64)
        };
        let estado = numero(WINHTTP_QUERY_STATUS_CODE).unwrap_or(0);
        if estado != 200 {
            return Err(fallo(nombre, format!("el servidor contesto {estado}")));
        }
        // Mas de 4 GB no cabe en el numero de 32 bits, pero el modelo mayor
        // pesa 130 MB.
        let total = numero(WINHTTP_QUERY_CONTENT_LENGTH);

        let fichero = std::fs::File::create(parte).map_err(|e| fallo(nombre, e))?;
        let mut escritor = std::io::BufWriter::with_capacity(1 << 20, fichero);
        let mut buf = vec![0u8; 64 * 1024];
        let mut leidos: u64 = 0;
        loop {
            if cancelado.load(Ordering::Relaxed) {
                return Err(ErrorVoz::Cancelada);
            }
            let mut n: u32 = 0;
            WinHttpReadData(
                peticion.0,
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                &mut n,
            )
            .map_err(|e| fallo(nombre, e))?;
            if n == 0 {
                break;
            }
            escritor
                .write_all(&buf[..n as usize])
                .map_err(|e| fallo(nombre, e))?;
            leidos += n as u64;
            avance(leidos, total);
        }
        escritor.flush().map_err(|e| fallo(nombre, e))?;
        drop(escritor);
        if let Some(t) = total
            && t != leidos
        {
            return Err(fallo(
                nombre,
                format!("llegaron {leidos} bytes de {t}: la conexion se corto"),
            ));
        }
        if leidos == 0 {
            return Err(fallo(nombre, "el servidor mando un fichero vacio"));
        }
        Ok(leidos)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_url_https_se_parte_en_servidor_y_ruta() {
        assert_eq!(
            partir_url("https://huggingface.co/csukuangfj/x/resolve/main/base-tokens.txt"),
            Some((
                "huggingface.co",
                "/csukuangfj/x/resolve/main/base-tokens.txt"
            ))
        );
        assert_eq!(
            partir_url("https://ejemplo.org"),
            Some(("ejemplo.org", "/"))
        );
    }

    #[test]
    fn una_url_sin_https_o_con_puerto_no_se_baja() {
        assert_eq!(partir_url("http://huggingface.co/a"), None);
        assert_eq!(partir_url("ftp://x/y"), None);
        assert_eq!(partir_url("https:///sin-servidor"), None);
        assert_eq!(partir_url("https://x:8080/a"), None);
    }

    #[test]
    fn el_temporal_lleva_parte_detras_del_nombre_entero() {
        assert_eq!(
            ruta_parte(Path::new("C:/d/whisper/base/base-encoder.int8.onnx")),
            Path::new("C:/d/whisper/base/base-encoder.int8.onnx.parte")
        );
    }

    #[test]
    fn cancelar_antes_de_empezar_no_deja_ni_el_temporal() {
        let dir = std::env::temp_dir().join(format!("pixpin-voz-bajar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let destino = dir.join("nada.txt");
        let r = bajar(
            "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base/resolve/main/base-tokens.txt",
            &destino,
            &mut |_, _| {},
            &AtomicBool::new(true),
        );
        // Se mira antes de abrir la conexion: ni siquiera sale a la red.
        assert!(matches!(r, Err(ErrorVoz::Cancelada)), "{r:?}");
        assert!(!destino.exists());
        assert!(!ruta_parte(&destino).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
