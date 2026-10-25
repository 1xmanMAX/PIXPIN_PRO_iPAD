//! Base64 estandar (con relleno), solo para mandar una imagen al modelo como
//! `data:image/png;base64,...`. Son veinte lineas: no merece un crate.

const TABLA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn codificar(datos: &[u8]) -> String {
    let mut s = String::with_capacity(datos.len().div_ceil(3) * 4);
    for trozo in datos.chunks(3) {
        let b = [
            trozo[0],
            *trozo.get(1).unwrap_or(&0),
            *trozo.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        s.push(TABLA[(n >> 18) as usize & 63] as char);
        s.push(TABLA[(n >> 12) as usize & 63] as char);
        s.push(if trozo.len() > 1 { TABLA[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if trozo.len() > 2 { TABLA[n as usize & 63] as char } else { '=' });
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::codificar;

    #[test]
    fn vectores_del_rfc_4648() {
        assert_eq!(codificar(b""), "");
        assert_eq!(codificar(b"f"), "Zg==");
        assert_eq!(codificar(b"fo"), "Zm8=");
        assert_eq!(codificar(b"foo"), "Zm9v");
        assert_eq!(codificar(b"foob"), "Zm9vYg==");
        assert_eq!(codificar(b"fooba"), "Zm9vYmE=");
        assert_eq!(codificar(b"foobar"), "Zm9vYmFy");
    }
}
