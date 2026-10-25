//! **`pixpin-aligerar.exe <entrada> <salida> <nivel> <hilos> <prioridad>`**:
//! el compresor de PDF de PixPin (pdfsqueeze), en su propio ejecutable.
//!
//! Lo lanza `pixpinmax.exe` desde `apps/pixpin/src/aligerar.rs`, que es donde
//! vive todo lo demas (la cola, el cambio en su sitio, los ajustes). Aqui solo
//! se comprime un fichero en otro. El protocolo:
//!
//! - **Salida estandar**: `p <n>` con el avance (0-100, solo cuando cambia) y,
//!   al acabar, `memoria <bytes>` con el pico de memoria del proceso.
//! - **Codigo de salida**: ver [`codigo`].
//! - **Entrada estandar**: tiene que estar abierta mientras trabaja. Cuando se
//!   cierra —PixPin se cerro o murio—, se va ([`codigo::SIN_PADRE`]), para no
//!   quedarse comprimiendo para nadie.
//! - `<nivel>`: `sin-perdida`, `equilibrado`, `pequeno` o `extremo`.
//! - `<prioridad>`: `baja` lo pone por debajo de lo normal (ya se lanza asi;
//!   esto lo asegura si alguien lo lanza a mano), `normal` no lo toca.
//!
//! La salida solo se escribe si pdfsqueeze gano algo; si no, no hay fichero.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

/// Como se llama el ejecutable, para que `pixpinmax` lo busque a su lado.
pub const NOMBRE_EXE: &str = "pixpin-aligerar.exe";

/// Codigos de salida.
pub mod codigo {
    /// Escrito, y mas pequeno que la entrada.
    pub const MENOR: i32 = 0;
    /// pdfsqueeze no gano nada: no se escribio nada.
    pub const NO_MENOR: i32 = 10;
    /// PDF cifrado: no se toca.
    pub const CIFRADO: i32 = 11;
    /// Argumentos mal, entrada que no se lee, o un fallo de pdfsqueeze.
    pub const FALLO: i32 = 12;
    pub const CANCELADO: i32 = 13;
    /// Se cerro su entrada estandar: PixPin ya no esta.
    pub const SIN_PADRE: i32 = 14;
}

/// El perfil de pdfsqueeze de cada nivel de PixPin (son los mismos cuatro).
pub fn perfil_de(nivel: &str) -> Option<pdfsqueeze_core::Profile> {
    use pdfsqueeze_core::Profile;
    Some(match nivel {
        "sin-perdida" => Profile::Lossless,
        "equilibrado" => Profile::Balanced,
        "pequeno" => Profile::Small,
        "extremo" => Profile::Extreme,
        _ => return None,
    })
}

/// Comprime `entrada` en `salida` con el perfil y los hilos dados, y dice
/// como acabo con un [`codigo`]. `avance` recibe el porcentaje desde los
/// hilos de trabajo y devuelve `false` para cancelar.
///
/// pdfsqueeze corre en su propio grupo de rayon, que no se mezcla con el
/// global. Lo usa el `main` de aqui y, en sus pruebas, `pixpinmax`.
pub fn comprimir(
    entrada: &Path,
    salida: &Path,
    perfil: pdfsqueeze_core::Profile,
    hilos: usize,
    avance: &(dyn Fn(u8) -> bool + Sync),
) -> i32 {
    let Ok(bytes) = std::fs::read(entrada) else {
        return codigo::FALLO;
    };
    let mut opciones = pdfsqueeze_core::Options::from_profile(perfil);
    // 0 = el grupo en el que se corra, que es el de aqui abajo. Con un numero,
    // pdfsqueeze intentaria montar el global de rayon.
    opciones.threads = 0;
    let Ok(grupo) = rayon::ThreadPoolBuilder::new()
        .num_threads(hilos.max(1))
        .thread_name(|i| format!("pdfsqueeze-{i}"))
        .build()
    else {
        return codigo::FALLO;
    };
    let r = grupo.install(|| {
        pdfsqueeze_core::pipeline::compress_with(&bytes, &opciones, &|p| avance(p.percent))
    });
    match r {
        Ok((datos, informe)) => {
            if informe.returned_original || datos.len() >= bytes.len() {
                return codigo::NO_MENOR;
            }
            match std::fs::write(salida, &datos) {
                Ok(()) => codigo::MENOR,
                Err(_) => {
                    let _ = std::fs::remove_file(salida);
                    codigo::FALLO
                }
            }
        }
        Err(pdfsqueeze_core::Error::Encrypted) => codigo::CIFRADO,
        Err(pdfsqueeze_core::Error::Cancelled) => codigo::CANCELADO,
        Err(_) => codigo::FALLO,
    }
}

/// Todo el programa: los argumentos (sin el nombre del ejecutable) y el
/// codigo con el que salir.
pub fn ejecutar(args: &[std::ffi::OsString]) -> i32 {
    let [entrada, salida, nivel, hilos, prioridad] = args else {
        return codigo::FALLO;
    };
    let Some(perfil) = nivel.to_str().and_then(perfil_de) else {
        return codigo::FALLO;
    };
    let hilos = hilos.to_str().and_then(|h| h.parse().ok()).unwrap_or(1);
    if prioridad.to_str() == Some("baja") {
        let _ = pixpin_shell::prioridad::proceso_por_debajo_de_lo_normal();
    }
    vigilar_al_padre();
    let fuera = std::io::stdout();
    let ultimo = AtomicU8::new(u8::MAX);
    let hecho = comprimir(
        Path::new(entrada),
        Path::new(salida),
        perfil,
        hilos,
        &|por| {
            // Solo cuando cambia: cada foto avisa, y un escaneo tiene cientos.
            if ultimo.swap(por, Ordering::Relaxed) != por {
                let mut f = fuera.lock();
                let _ = writeln!(f, "p {por}");
                let _ = f.flush();
            }
            true
        },
    );
    if let Some(pico) = pixpin_shell::prioridad::memoria_pico() {
        let mut f = fuera.lock();
        let _ = writeln!(f, "memoria {pico}");
        let _ = f.flush();
    }
    hecho
}

/// Un hilo que lee la entrada estandar hasta que se cierra, y entonces sale.
fn vigilar_al_padre() {
    let _ = std::thread::Builder::new()
        .name("vigilar-al-padre".into())
        .spawn(|| {
            let mut entrada = std::io::stdin();
            let mut b = [0u8; 64];
            loop {
                match entrada.read(&mut b) {
                    Ok(0) | Err(_) => std::process::exit(codigo::SIN_PADRE),
                    Ok(_) => {}
                }
            }
        });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_nivel_de_pixpin_es_su_perfil_de_pdfsqueeze() {
        use pdfsqueeze_core::Profile;
        assert_eq!(perfil_de("sin-perdida"), Some(Profile::Lossless));
        assert_eq!(perfil_de("equilibrado"), Some(Profile::Balanced));
        assert_eq!(perfil_de("pequeno"), Some(Profile::Small));
        assert_eq!(perfil_de("extremo"), Some(Profile::Extreme));
        // Caso negativo: un nombre que no es de PixPin no se adivina.
        assert_eq!(perfil_de("balanced"), None);
    }
}
