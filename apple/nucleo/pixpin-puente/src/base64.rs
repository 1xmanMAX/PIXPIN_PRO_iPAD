//! Base64 estandar (RFC 4648, con o sin `=`), solo para leer las imagenes
//! que manda la extension. Treinta lineas no justifican un crate.

const MALO: u8 = 0xFF;
const BLANCO: u8 = 0xFE;
const RELLENO: u8 = 0xFD;

/// El valor de cada byte: 0–63 del alfabeto, o que es (blanco, `=`, malo).
const TABLA: [u8; 256] = {
    let mut t = [MALO; 256];
    let mut i = 0;
    while i < 26 {
        t[b'A' as usize + i] = i as u8;
        t[b'a' as usize + i] = 26 + i as u8;
        i += 1;
    }
    let mut d = 0;
    while d < 10 {
        t[b'0' as usize + d] = 52 + d as u8;
        d += 1;
    }
    t[b'+' as usize] = 62;
    t[b'/' as usize] = 63;
    t[b'=' as usize] = RELLENO;
    t[b' ' as usize] = BLANCO;
    t[b'\n' as usize] = BLANCO;
    t[b'\r' as usize] = BLANCO;
    t[b'\t' as usize] = BLANCO;
    t
};

/// Decodifica. Los blancos se saltan; cualquier otro caracter fuera del
/// alfabeto, o un relleno en mitad, es un error. Con una tabla y de cuatro
/// en cuatro: una captura de pagina son megas y el puente tiene que
/// contestar en milisegundos.
pub fn decodificar(texto: &str) -> Result<Vec<u8>, String> {
    let mut fuera = Vec::with_capacity(texto.len() / 4 * 3 + 3);
    let mut acumulado: u32 = 0;
    let mut cuantos = 0u32;
    let mut relleno = 0usize;
    for (i, &c) in texto.as_bytes().iter().enumerate() {
        let v = TABLA[c as usize];
        if v < 64 {
            if relleno > 0 {
                return Err("relleno «=» en mitad del base64".into());
            }
            acumulado = (acumulado << 6) | u32::from(v);
            cuantos += 1;
            if cuantos == 4 {
                fuera.extend_from_slice(&[
                    (acumulado >> 16) as u8,
                    (acumulado >> 8) as u8,
                    acumulado as u8,
                ]);
                acumulado = 0;
                cuantos = 0;
            }
        } else if v == RELLENO {
            relleno += 1;
        } else if v == MALO {
            return Err(format!("caracter no valido en base64 en la posicion {i}"));
        }
    }
    match cuantos {
        0 => {}
        2 => fuera.push((acumulado >> 4) as u8),
        3 => fuera.extend_from_slice(&[(acumulado >> 10) as u8, (acumulado >> 2) as u8]),
        _ => return Err("base64 cortado".into()),
    }
    if relleno > 2 {
        return Err("base64 cortado".into());
    }
    Ok(fuera)
}

#[cfg(test)]
mod pruebas {
    use super::decodificar;

    #[test]
    fn los_casos_de_la_rfc() {
        for (b64, claro) in [
            ("", ""),
            ("Zg==", "f"),
            ("Zm8=", "fo"),
            ("Zm9v", "foo"),
            ("Zm9vYg==", "foob"),
            ("Zm9vYmE=", "fooba"),
            ("Zm9vYmFy", "foobar"),
            ("Zm9vYmE", "fooba"),
            ("Zm9v\r\nYmFy", "foobar"),
        ] {
            assert_eq!(decodificar(b64).unwrap(), claro.as_bytes(), "{b64}");
        }
        assert_eq!(decodificar("/+8=").unwrap(), [0xFF, 0xEF]);
    }

    #[test]
    fn caso_negativo_lo_que_no_es_base64() {
        assert!(decodificar("Zm9v!").is_err());
        assert!(decodificar("Zg==Zg==").is_err());
        assert!(decodificar("Z").is_err());
        assert!(decodificar("data:image/png;base64,AAAA").is_err());
    }
}
