//! El codigo del grupo: de lo que se teclea a la clave que cifra.
//!
//! Byte a byte lo mismo que PixPin Android (`sincro/Identidad.kt`), porque
//! si no el movil y el ordenador no se entienden: la clave NUNCA viaja, y
//! que los dos lados la deriven igual es toda la autenticacion que hay. Si
//! una sola vuelta de PBKDF2 baila, el otro aparato solo dira «tiene otro
//! codigo de grupo» y no habra forma de saber por que.
//!
//! Por eso los numeros de aqui no se tocan sin tocar tambien el movil.

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Los signos del codigo. Sin 0/O ni 1/I/L: un codigo se dicta por telefono.
pub const SIGNOS: &str = "23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// Cuantos signos tiene un codigo de grupo.
pub const LARGO: usize = 10;
/// La sal de PBKDF2, tal cual en Android.
pub const SAL: &[u8] = b"pixpin-sincro-grupo";
/// Vueltas de PBKDF2. Son muchas a proposito: lo unico que protege un codigo
/// de diez signos de quien lo pruebe a lo bruto es el coste de cada intento.
pub const VUELTAS: u32 = 60_000;

/// Deja el codigo como lo espera la derivacion: mayusculas y solo signos del
/// alfabeto. Asi `k7q2m-9xmpa` y `K7Q2M9XMPA` dan la misma clave, que es lo
/// que hace que valga dictarlo con guion.
pub fn limpiar(codigo: &str) -> String {
    codigo
        .chars()
        .flat_map(|c| c.to_uppercase())
        .filter(|c| SIGNOS.contains(*c))
        .collect()
}

/// La clave de 32 bytes de un grupo.
pub fn clave_de_grupo(codigo: &str) -> [u8; 32] {
    // Se recuerda por codigo: son 60 000 vueltas de PBKDF2 y cada conexion
    // las pagaba dos veces (una por lado), segundos enteros sin optimizar.
    static HECHAS: std::sync::Mutex<Vec<(String, [u8; 32])>> = std::sync::Mutex::new(Vec::new());
    let limpio = limpiar(codigo);
    if let Some(c) = HECHAS
        .lock()
        .ok()
        .and_then(|h| h.iter().find(|(k, _)| *k == limpio).map(|(_, c)| *c))
    {
        return c;
    }
    let mut clave = [0u8; 32];
    // Android pasa el codigo como `char[]` a PBEKeySpec y Java lo codifica en
    // UTF-8; todos los signos son ASCII, asi que son los mismos bytes.
    pbkdf2::pbkdf2_hmac::<Sha256>(limpio.as_bytes(), SAL, VUELTAS, &mut clave);
    if let Ok(mut h) = HECHAS.lock() {
        h.push((limpio, clave));
    }
    clave
}

/// La clave de 32 bytes de un texto TAL CUAL, sin limpiarlo.
///
/// Android deriva asi (`Grupo.clave`): limpiar el codigo es cosa de quien lo
/// teclea, no de la derivacion. Hace falta cruda para el envio puntual, cuyo
/// texto es `envio-482913` —con guion, ceros y unos, que el alfabeto del
/// grupo no admite y `limpiar` tiraria—.
pub fn clave_cruda(texto: &str) -> [u8; 32] {
    let mut clave = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(texto.as_bytes(), SAL, VUELTAS, &mut clave);
    clave
}

/// La etiqueta publica del grupo: los ocho primeros bytes de
/// `HMAC(clave, "etiqueta")`, en hex minuscula (16 caracteres).
///
/// Es lo que se anuncia en la red (el par TXT `g`) para que dos aparatos del
/// mismo grupo se reconozcan **sin publicar el codigo**: de la etiqueta no se
/// vuelve atras.
pub fn etiqueta(clave: &[u8; 32]) -> String {
    let marca = hmac(clave, b"etiqueta");
    marca[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// `HMAC-SHA256`, la pieza con la que se deriva todo lo demas.
pub fn hmac(clave: &[u8], datos: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(clave)
        .expect("HMAC admite claves de cualquier largo");
    mac.update(datos);
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn la_clave_es_la_misma_que_saca_el_movil() {
        // Vectores calculados aparte con otra implementacion de
        // PBKDF2-HMAC-SHA256, no con esta: una prueba que use el mismo
        // codigo que prueba no demuestra nada.
        let clave = clave_de_grupo("K7Q2M9XMPA");
        assert_eq!(
            hex(&clave),
            "99e570f7d4ff8055c6f7af090312e76f13937fae52928187714661e29b6010c4"
        );
        assert_eq!(etiqueta(&clave), "09df6bf6b40e77f0");
        assert_eq!(etiqueta(&clave).len(), 16);
    }

    #[test]
    fn el_codigo_se_puede_dictar_con_guion_y_en_minusculas() {
        let esperada = clave_de_grupo("K7Q2M9XMPA");
        for escrito in ["K7Q2M-9XMPA", "k7q2m-9xmpa", " K7Q2M 9XMPA ", "K7Q2M.9XMPA"] {
            assert_eq!(clave_de_grupo(escrito), esperada, "{escrito}");
        }
        // Caso negativo: otro codigo NO da la misma clave, ni de lejos.
        assert_ne!(clave_de_grupo("K7Q2M9XMPB"), esperada);
    }

    #[test]
    fn limpiar_tira_lo_que_no_es_del_alfabeto() {
        assert_eq!(limpiar("k7q2m-9xmpa"), "K7Q2M9XMPA");
        // El 0, la O, el 1, la I y la L no estan en el alfabeto: se caen.
        assert_eq!(limpiar("O0I1L"), "");
        assert_eq!(limpiar(""), "");
        assert_eq!(limpiar("K7Q2M9XMPA").len(), LARGO);
    }

    #[test]
    fn el_hmac_es_el_de_libro() {
        // Vector de prueba 1 del RFC 4231 para HMAC-SHA256.
        let clave = [0x0bu8; 20];
        assert_eq!(
            hex(&hmac(&clave, b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
}
