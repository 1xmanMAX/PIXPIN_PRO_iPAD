//! Lo poco de URL que hace falta para OAuth y las API de archivos: escapar
//! un valor, leer una consulta `a=b&c=d` y montar un formulario
//! `application/x-www-form-urlencoded`. Son treinta lineas: no merece un
//! crate.

/// Escapa un valor para una URL o un formulario (RFC 3986: solo quedan tal
/// cual las letras ASCII, las cifras y `-._~`).
pub fn escapar(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3 / 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Deshace [`escapar`] (y el `+` de los formularios, que es un espacio).
/// Un `%` mal formado se deja tal cual.
pub fn desescapar(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
                {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Los pares de una consulta (`code=4%2F0A&state=xyz`), ya desescapados.
pub fn consulta(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (desescapar(k), desescapar(v)),
            None => (desescapar(p), String::new()),
        })
        .collect()
}

/// El valor de `clave` en una consulta.
pub fn valor<'a>(pares: &'a [(String, String)], clave: &str) -> Option<&'a str> {
    pares
        .iter()
        .find(|(k, _)| k == clave)
        .map(|(_, v)| v.as_str())
}

/// Monta `a=b&c=d` con cada valor escapado (la consulta de una URL o el
/// cuerpo de un formulario).
pub fn formulario(pares: &[(&str, &str)]) -> String {
    pares
        .iter()
        .map(|(k, v)| format!("{}={}", escapar(k), escapar(v)))
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn escapar_y_desescapar_van_y_vuelven() {
        assert_eq!(escapar("a b/c?d=é"), "a%20b%2Fc%3Fd%3D%C3%A9");
        assert_eq!(desescapar("a%20b%2Fc%3Fd%3D%C3%A9"), "a b/c?d=é");
        assert_eq!(desescapar("4%2F0Aa+b"), "4/0Aa b");
        // Un % roto no se come nada.
        assert_eq!(desescapar("100%"), "100%");
        assert_eq!(desescapar("%zz1"), "%zz1");
        let q = consulta("code=4%2F0AX&state=abc&scope=x+y&vacio");
        assert_eq!(valor(&q, "code"), Some("4/0AX"));
        assert_eq!(valor(&q, "scope"), Some("x y"));
        assert_eq!(valor(&q, "vacio"), Some(""));
        assert_eq!(valor(&q, "otro"), None);
        assert_eq!(formulario(&[("a", "1 2"), ("b", "x&y")]), "a=1%202&b=x%26y");
    }
}
