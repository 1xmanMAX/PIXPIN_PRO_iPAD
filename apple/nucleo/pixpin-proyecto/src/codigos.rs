//! Los tres codigos que dicen que es cada cosa, este donde este.
//!
//! Puerto de `sincro/Codigos.kt` de PixPin Android (v0.50.1, 15-sep-2026).
//! Todo lo que entra al chat —un lienzo, una hoja, un proyecto, un mensaje—
//! nace con tres codigos que no cambian nunca, se comparta las veces que se
//! comparta:
//!
//! 1. **El codigo unico** (`uid`): diez signos al azar de un alfabeto de 31.
//!    Lo de antes, que no lo tenia, lo saca de su id con SHA-256: asi dos
//!    aparatos que ya tenian lo mismo le ponen el mismo codigo sin hablarse.
//! 2. **El codigo de chat**: el numero del mensaje y el codigo del aparato
//!    donde nacio, `47·K7Q2` (antes una letra: `47a`).
//! 3. **La fecha de creacion**.
//!
//! Solo se pone al dia algo si coinciden los tres. Si no, se crea aparte:
//! ante la duda, duplicar y no pisar. Copiar a proposito da codigos nuevos.
//!
//! Tiene que dar EXACTAMENTE los mismos codigos que Android: las pruebas
//! comparan con valores calculados con su algoritmo.

/// Sin 0/O ni 1/I/L: se dicta en voz alta y se teclea sin dudar.
pub const SIGNOS: &[u8; 31] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
pub const LARGO: usize = 10;
/// El separador del codigo de chat: «cuarenta y siete de K7Q2».
pub const PUNTO: char = '·';

/// SHA-256 (FIPS 180-4). Propio y no una dependencia: son sesenta lineas y
/// solo se usa para sacar codigos, que no es criptografia de seguridad.
pub fn sha256(datos: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut mensaje = datos.to_vec();
    let bits = (datos.len() as u64).wrapping_mul(8);
    mensaje.push(0x80);
    while mensaje.len() % 64 != 56 {
        mensaje.push(0);
    }
    mensaje.extend_from_slice(&bits.to_be_bytes());

    for bloque in mensaje.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, palabra) in bloque.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([palabra[0], palabra[1], palabra[2], palabra[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(sigma1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = sigma0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (i, v) in [a, b, c, d, e, f, g, hh].into_iter().enumerate() {
            h[i] = h[i].wrapping_add(v);
        }
    }
    let mut salida = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        salida[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    salida
}

fn signos_de(hash: &[u8], cuantos: usize) -> String {
    hash.iter()
        .take(cuantos)
        .map(|b| SIGNOS[*b as usize % SIGNOS.len()] as char)
        .collect()
}

/// Un codigo fijo sacado de `semilla`: el mismo en todos los aparatos
/// (`Codigos.de` de Android).
pub fn de(semilla: &str) -> String {
    signos_de(&sha256(semilla.as_bytes()), LARGO)
}

/// Un codigo nuevo al azar. La entropia sale del sistema a traves de las
/// claves aleatorias de `RandomState` mezcladas con la hora y un contador,
/// y pasadas por SHA-256: suficiente para que no se repitan, que es lo
/// unico que se les pide.
pub fn nuevo() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(CONTADOR.fetch_add(1, Ordering::Relaxed));
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    h.write_u128(ahora);
    let mut semilla = h.finish().to_le_bytes().to_vec();
    semilla.extend_from_slice(&ahora.to_le_bytes());
    signos_de(&sha256(&semilla), LARGO)
}

/// El codigo fijo de un aparato: cuatro signos sacados de su id
/// (`Aparato.codigo` de Android).
pub fn de_aparato(id_aparato: &str) -> String {
    signos_de(&sha256(id_aparato.as_bytes()), 4)
}

/// El codigo de chat: `47·K7Q2`, el de antes `47a`, o `None` sin numero.
pub fn de_chat(numero: i64, aparato: Option<&str>, letra: Option<&str>) -> Option<String> {
    if numero <= 0 {
        return None;
    }
    match (aparato, letra) {
        (Some(a), _) => Some(format!("{numero}{PUNTO}{a}")),
        (None, Some(l)) => Some(format!("{numero}{l}")),
        _ => None,
    }
}

/// El codigo unico de algo: el suyo, o el que sale de su id si es de antes.
/// `prefijo` es `"m:"` para mensajes, `"h:"` para hojas y `"p:"` para
/// proyectos, como en Android.
pub fn unico(uid: Option<&str>, prefijo: &str, id: &str) -> String {
    match uid {
        Some(u) => u.to_string(),
        None => de(&format!("{prefijo}{id}")),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn sha256_da_los_vectores_del_nist() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // 56 bytes obligan a un segundo bloque de relleno.
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn los_codigos_derivados_son_los_mismos_que_en_android() {
        // Calculados con el algoritmo de `Codigos.de` y `Aparato.codigo`.
        assert_eq!(de("p:proyecto-1"), "VVT587BFCA");
        assert_eq!(de("h:hoja-7"), "2C8C39HXZ9");
        assert_eq!(de("m:1757939357123"), "RVK5YHKCX7");
        // UTF-8 de verdad, como `String.toByteArray()` en Kotlin.
        assert_eq!(de("m:ñandú"), "YWZ9C4UHND");
        assert_eq!(de_aparato("aparato-demo"), "9FMQ");
        assert_eq!(unico(None, "p:", "proyecto-1"), "VVT587BFCA");
        assert_eq!(unico(Some("ABCDEFGHJK"), "p:", "proyecto-1"), "ABCDEFGHJK");
    }

    #[test]
    fn los_codigos_nuevos_tienen_diez_signos_del_alfabeto_y_no_se_repiten() {
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..2000 {
            let c = nuevo();
            assert_eq!(c.len(), LARGO);
            assert!(c.bytes().all(|b| SIGNOS.contains(&b)), "{c}");
            assert!(vistos.insert(c), "repetido");
        }
    }

    #[test]
    fn el_codigo_de_chat_prefiere_el_aparato_y_sin_numero_no_hay() {
        assert_eq!(
            de_chat(47, Some("K7Q2"), Some("a")).as_deref(),
            Some("47·K7Q2")
        );
        assert_eq!(de_chat(47, None, Some("a")).as_deref(), Some("47a"));
        assert_eq!(de_chat(0, Some("K7Q2"), None), None);
        assert_eq!(de_chat(5, None, None), None);
    }
}
