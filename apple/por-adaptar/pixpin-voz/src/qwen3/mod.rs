//! **Qwen3-ASR-0.6B**: el motor opcional para pasar grabaciones a texto,
//! sobre el mismo ONNX Runtime que Whisper e Indra.
//!
//! Whisper *base* sigue siendo el motor de fabrica y el de reserva. Qwen3-ASR
//! se baja solo si el usuario lo elige en Ajustes (unos 988 MB, Apache-2.0)
//! y a cambio entiende mucho mejor el castellano real: en el banco de
//! `docs/investigacion/2026-10-25-voz-a-texto-en-espanol.md` (FLEURS es_419)
//! el WER pasa de 11,8 a 5,8 % en limpio, de 22,5 a 8,3 % en una reunion y
//! de 94,6 a 37,7 % en un aula. Los numeros de esta exportacion y del
//! equipo de referencia estan en `docs/voz-qwen3.md`.
//!
//! ## Que corre
//!
//! La exportacion int8 de sherpa-onnx ([`catalogo`]): tres modelos ONNX.
//!
//! 1. El audio se parte en tandas de hasta 20 s por los silencios
//!    ([`guion::tandas`], los mismos cortes que Whisper); cada tanda sabe en
//!    que milisegundo empieza (`[m:ss]`).
//! 2. Log-mel de 128 bandas sin rellenar ([`mel`]) → **frente
//!    convolucional** → **encoder** de audio: ~13 vectores por segundo.
//! 3. El **decoder** (el modelo de lenguaje de 0,6B) lee el guion de chat
//!    con esos vectores en el sitio de los `<|audio_pad|>` ([`guion`]) y
//!    escribe token a token, voraz, con una cache de claves y valores que
//!    se rellena en su sitio paso a paso.
//! 4. Los tokens pasan a texto ([`fichas`]) y el texto, por
//!    [`creible`](crate::credibilidad::creible), como con Whisper.
//!
//! Lo que es aritmetica o texto (mel, tokens, guion, cortes, catalogo) no
//! toca Windows y se prueba en cualquier sistema. Lo que habla con ONNX
//! Runtime esta en [`motor`], que tambien compila en Linux para probarlo
//! con el modelo de verdad; leer el `.m4a` y bajar el modelo son solo de
//! Windows.

pub mod catalogo;
pub mod fichas;
pub mod guion;
pub mod mel;
/// Compila en todos los sistemas (como `ort`); lo que lee audio o baja de
/// la red, solo en Windows.
pub mod motor;

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub use guion::sabe;

/// `<datos>/qwen3-asr/`.
pub fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("qwen3-asr")
}

/// Si un fichero esta entero (con su tamano exacto; el sha256 se comprobo
/// al bajarlo).
fn esta_entero(p: &Path, f: &catalogo::Fichero) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.len() == f.bytes)
}

/// Si los seis ficheros estan bajados y enteros.
pub fn modelo_listo(raiz: &Path) -> bool {
    let c = carpeta(raiz);
    catalogo::FICHEROS
        .iter()
        .all(|f| esta_entero(&c.join(f.nombre), f))
}

/// Si hay algo bajado (aunque sea a medias): para ofrecer «Borrar».
pub fn algo_bajado(raiz: &Path) -> bool {
    std::fs::read_dir(carpeta(raiz)).is_ok_and(|mut d| d.next().is_some())
}

/// Lo que falta por bajar, en MB.
pub fn megas_a_bajar(raiz: &Path) -> u64 {
    let c = carpeta(raiz);
    catalogo::FICHEROS
        .iter()
        .filter(|f| !esta_entero(&c.join(f.nombre), f))
        .map(|f| f.bytes)
        .sum::<u64>()
        .div_ceil(1_000_000)
}

/// Borra lo bajado (Ajustes › «Borrar Qwen3-ASR»). El runtime se queda:
/// lo usan Whisper e Indra.
pub fn borrar(raiz: &Path) -> std::io::Result<()> {
    let c = carpeta(raiz);
    if c.exists() {
        std::fs::remove_dir_all(c)?;
    }
    Ok(())
}

/// sha256 de un fichero, en hexadecimal.
pub fn sha256_de(p: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|x| format!("{x:02x}")).collect())
}

/// **Si se ofrece en este equipo**: con al menos ~3,5 GB de memoria
/// fisica (los «4 GB» del equipo de referencia lo son). Por debajo, Windows
/// y la app no dejan sitio a los ~1,5 GB que pide mientras transcribe.
pub fn se_ofrece(memoria_total_mb: u64) -> bool {
    memoria_total_mb >= catalogo::RAM_MINIMA_MB
}

/// Como corre ONNX Runtime con Qwen3-ASR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reparto {
    pub hilos: usize,
    /// Preempaquetar los pesos int8 al cargar: ~1,5× mas rapido y ~200 MB
    /// mas de memoria (medido, ver `docs/voz-qwen3.md`).
    pub preempaquetar: bool,
}

/// **El reparto segun el equipo.** Sin arena nunca (no gana velocidad y
/// cuesta ~300 MB de pico). En modo cuidadoso (4 GB o nivel Ligero) sin
/// preempaquetar, y dejando un hilo libre para la interfaz; si no, hasta
/// ocho hilos y preempaquetado.
pub fn reparto(cuidadoso: bool, nucleos: usize) -> Reparto {
    if cuidadoso {
        Reparto {
            hilos: nucleos.saturating_sub(1).clamp(1, 3),
            preempaquetar: false,
        }
    } else {
        Reparto {
            hilos: nucleos.clamp(1, 8),
            preempaquetar: true,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn listo_solo_con_los_seis_ficheros_enteros() {
        let raiz = std::env::temp_dir().join(format!("pixpin-voz-qwen3-{}", std::process::id()));
        let c = carpeta(&raiz);
        std::fs::create_dir_all(&c).unwrap();
        assert!(!modelo_listo(&raiz));
        assert!(!algo_bajado(&raiz));
        assert_eq!(megas_a_bajar(&raiz), catalogo::megas());
        // Un fichero pequeno con el tamano bueno cuenta; uno corto no.
        let f = &catalogo::FICHEROS[catalogo::VOCAB];
        std::fs::write(c.join(f.nombre), vec![b' '; f.bytes as usize]).unwrap();
        assert!(algo_bajado(&raiz));
        assert!(megas_a_bajar(&raiz) < catalogo::megas());
        std::fs::write(c.join(f.nombre), b"{}").unwrap();
        assert_eq!(megas_a_bajar(&raiz), catalogo::megas());
        assert!(!modelo_listo(&raiz));
        borrar(&raiz).unwrap();
        assert!(!c.exists());
        borrar(&raiz).unwrap();
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_sha_de_un_fichero_es_el_de_sha256sum() {
        let p = std::env::temp_dir().join(format!("pixpin-voz-qwen3-sha-{}", std::process::id()));
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_de(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn se_ofrece_desde_los_cuatro_gigas() {
        assert!(se_ofrece(4_096));
        assert!(se_ofrece(3_900), "4 GB con la grafica integrada comiendo");
        assert!(!se_ofrece(2_048));
    }

    #[test]
    fn en_modo_cuidadoso_sin_preempaquetar_y_con_un_hilo_libre() {
        assert_eq!(
            reparto(true, 4),
            Reparto {
                hilos: 3,
                preempaquetar: false
            }
        );
        assert_eq!(reparto(true, 1).hilos, 1);
        assert_eq!(
            reparto(false, 4),
            Reparto {
                hilos: 4,
                preempaquetar: true
            }
        );
        assert_eq!(reparto(false, 32).hilos, 8);
    }
}
