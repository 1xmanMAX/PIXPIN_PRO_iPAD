//! El grupo, sus miembros y la sena de cada mensaje.
//!
//! Puerto de la parte pura de `sincro/Identidad.kt` (`Grupo`, el codigo
//! fijo de un `Aparato`) y de `sincro/Sena.kt`. Lo que ya estaba en
//! `codigo` (limpiar, la clave PBKDF2, la etiqueta) no se repite: se llama
//! a lo que hay.
//!
//! Nada de aqui mira el reloj ni tira dados: el azar entra por parametro,
//! que es lo que deja probar el reparto de codigos y lo que evita que un
//! generador flojo se cuele sin que nadie lo vea.

use std::collections::HashSet;

use crate::codigo::{LARGO, SIGNOS};
use crate::mensajes::Aparato;

/// Un codigo de grupo nuevo: diez signos al azar. `azar(n)` tiene que
/// devolver un numero por debajo de `n`, uniforme (como `SecureRandom
/// .nextInt(n)` en Android); de ahi sale toda la entropia del codigo.
pub fn nuevo_codigo(mut azar: impl FnMut(usize) -> usize) -> String {
    let signos: Vec<char> = SIGNOS.chars().collect();
    (0..LARGO)
        .map(|_| signos[azar(signos.len()).min(signos.len() - 1)])
        .collect()
}

/// Si tiene la forma de un codigo: diez signos del alfabeto, ya limpio.
pub fn valido(codigo: &str) -> bool {
    codigo.chars().count() == LARGO && codigo.chars().all(|c| SIGNOS.contains(c))
}

/// `K7Q2M-9XMPA`: en dos mitades, que es como se lee y se copia sin
/// perderse.
pub fn legible(codigo: &str) -> String {
    if codigo.chars().count() <= 5 {
        return codigo.to_string();
    }
    let (a, b): (String, String) = (
        codigo.chars().take(5).collect(),
        codigo.chars().skip(5).collect(),
    );
    format!("{a}-{b}")
}

/// El codigo fijo del aparato (`Aparato.codigo` en Android): cuatro signos
/// sacados del SHA-256 de su id, los mismos siempre, este o no en un grupo.
/// Es lo que se ve junto al nombre («Max phone · K7Q2») y lo que lleva cada
/// mensaje como su aparato de origen, asi que tiene que salir igual aqui.
pub fn codigo_de_aparato(id: &str) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(id.as_bytes());
    let signos: Vec<char> = SIGNOS.chars().collect();
    h[..4]
        .iter()
        .map(|b| signos[*b as usize % signos.len()])
        .collect()
}

/// Los miembros de dos aparatos, juntos. Por id; de uno repetido gana el
/// nombre del que lo trae de primera mano (el propio aparato habla por si
/// mismo). El orden es el de un `LinkedHashMap`: los mios, luego los suyos
/// que faltaban, y quien habla en el sitio que ya tuviera.
pub fn juntar(mios: &[Aparato], suyos: &[Aparato], quien_habla: Option<&Aparato>) -> Vec<Aparato> {
    let mut salida: Vec<Aparato> = Vec::with_capacity(mios.len() + suyos.len());
    let mut poner =
        |a: &Aparato, solo_si_falta: bool| match salida.iter_mut().find(|x| x.id == a.id) {
            Some(x) => {
                if !solo_si_falta {
                    *x = a.clone();
                }
            }
            None => salida.push(a.clone()),
        };
    for a in mios {
        poner(a, false);
    }
    for a in suyos {
        poner(a, true);
    }
    if let Some(q) = quien_habla {
        poner(q, false);
    }
    salida
}

/// Letras repetidas entre aparatos distintos: no deberia pasar, pero si
/// pasa hay que decirlo. Cada letra con los aparatos que la llevan, en el
/// orden en que aparecen.
pub fn choques(miembros: &[Aparato]) -> Vec<(String, Vec<Aparato>)> {
    let mut grupos: Vec<(String, Vec<Aparato>)> = Vec::new();
    for a in miembros {
        let Some(letra) = &a.letra else { continue };
        match grupos.iter_mut().find(|(l, _)| l == letra) {
            Some((_, lista)) => lista.push(a.clone()),
            None => grupos.push((letra.clone(), vec![a.clone()])),
        }
    }
    grupos.retain(|(_, lista)| lista.len() > 1);
    grupos
}

/// La sena de un mensaje: su numero y la letra del aparato donde nacio.
///
/// `47a` es «el cuarenta y siete del telefono». Con una letra por aparato
/// dos aparatos no pueden chocar al repartir numeros, y la sena no cambia
/// al viajar: es lo que permite reconocer despues que son el mismo mensaje.
pub mod sena {
    /// Las letras que se reparten, en orden. Ver [libre].
    pub const LETRAS: &str = "abcdefghijklmnopqrstuvwxyz";

    /// La sena de un mensaje: su numero pegado a la letra del aparato.
    ///
    /// El numero cero es un mensaje de antes de que esto existiera y no
    /// tiene sena: se devuelve None para que quien llame decida que hace
    /// con el en vez de inventarse un `0a` que chocaria con todos los ceros
    /// de todos los aparatos.
    pub fn de(numero: i32, letra: char) -> Option<String> {
        if numero <= 0 {
            None
        } else {
            Some(format!("{numero}{letra}"))
        }
    }

    /// El numero de una sena, o None si no lo es.
    pub fn numero_de(sena: &str) -> Option<i32> {
        let n = sin_ultima(sena).parse::<i32>().ok()?;
        if valida(sena) { Some(n) } else { None }
    }

    /// La letra de una sena, o None si no lo es.
    pub fn letra_de(sena: &str) -> Option<char> {
        if valida(sena) {
            sena.chars().last()
        } else {
            None
        }
    }

    /// Si esto tiene forma de sena: digitos y una letra al final, y el
    /// numero mayor que cero.
    pub fn valida(sena: &str) -> bool {
        if sena.chars().count() < 2 {
            return false;
        }
        let Some(ultima) = sena.chars().last() else {
            return false;
        };
        if !LETRAS.contains(ultima) {
            return false;
        }
        sin_ultima(sena).parse::<i32>().is_ok_and(|n| n > 0)
    }

    fn sin_ultima(s: &str) -> &str {
        match s.char_indices().last() {
            Some((i, _)) => &s[..i],
            None => s,
        }
    }

    /// Una letra libre para un aparato que se une al grupo. Se reparte
    /// sola (lo decidio el usuario el 9-sep-2026): elegirla a mano se
    /// agota, choca, y obliga a resolver el choque justo al emparejar. None
    /// si el grupo ya tiene veintiseis aparatos.
    pub fn libre(ocupadas: &[char]) -> Option<char> {
        LETRAS.chars().find(|c| !ocupadas.contains(c))
    }
}

/// Las letras que ya llevan unos miembros, para pedir una [sena::libre].
pub fn letras_ocupadas(miembros: &[Aparato]) -> Vec<char> {
    let mut vistas = HashSet::new();
    miembros
        .iter()
        .filter_map(|a| a.letra.as_ref()?.chars().next())
        .filter(|c| vistas.insert(*c))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::canonico::sha256_hex;

    fn aparato(id: &str, nombre: &str, letra: Option<&str>) -> Aparato {
        Aparato {
            id: id.into(),
            nombre: nombre.into(),
            letra: letra.map(str::to_string),
            desde: 0,
        }
    }

    #[test]
    fn el_codigo_se_limpia_al_teclearlo() {
        assert_eq!(crate::codigo::limpiar("abcde-23456 "), "ABCDE23456");
        assert_eq!(legible("ABCDE23456"), "ABCDE-23456");
        assert_eq!(legible("ABC"), "ABC");
        assert!(valido("ABCDE23456"));
        // Casos negativos: corto, con un signo fuera del alfabeto, en
        // minusculas (valido espera lo ya limpio).
        assert!(!valido("ABCDE2345"));
        assert!(!valido("ABCDE2345O"));
        assert!(!valido("abcde23456"));
    }

    #[test]
    fn un_codigo_nuevo_sale_del_azar_que_se_le_da_y_es_valido() {
        // Con un azar que siempre da cero, todo son doses: se ve que la
        // entropia entra por el parametro y no de dentro.
        assert_eq!(nuevo_codigo(|_| 0), "2222222222");
        let mut i = 0usize;
        let codigo = nuevo_codigo(|n| {
            i += 7;
            i % n
        });
        assert!(valido(&codigo), "{codigo}");
        // 7, 14, 21, 28, 35, 42, 49, 56, 63, 70 modulo 31, sobre el alfabeto.
        assert_eq!(codigo, "9GQX6DMU3A");
        // Un azar que se pasa de rosca no saca nada fuera del alfabeto.
        assert!(valido(&nuevo_codigo(|_| usize::MAX)));
    }

    #[test]
    fn el_codigo_del_aparato_sale_de_su_id_y_no_cambia() {
        let c = codigo_de_aparato("id-tel");
        assert_eq!(c.chars().count(), 4);
        assert!(c.chars().all(|x| SIGNOS.contains(x)));
        assert_eq!(c, codigo_de_aparato("id-tel"));
        assert_ne!(c, codigo_de_aparato("id-tab"));
        // La formula de Android: byte a byte del SHA-256, modulo 31.
        let h = sha256_hex("id-tel".as_bytes());
        let esperado: String = (0..4)
            .map(|i| {
                let b = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap();
                SIGNOS.chars().nth(b as usize % 31).unwrap()
            })
            .collect();
        assert_eq!(c, esperado);
    }

    #[test]
    fn juntar_miembros_va_por_id_y_quien_habla_manda_sobre_si_mismo() {
        let mios = [
            aparato("t", "Telefono", Some("a")),
            aparato("x", "Viejo nombre", Some("c")),
        ];
        let suyos = [
            aparato("x", "Otro nombre", Some("c")),
            aparato("b", "Tableta", Some("b")),
        ];
        let juntos = juntar(&mios, &suyos, None);
        let ids: Vec<&str> = juntos.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["t", "x", "b"]);
        // Del repetido se queda lo mio.
        assert_eq!(juntos[1].nombre, "Viejo nombre");
        // Salvo que sea quien habla: entonces manda su version, en su sitio.
        let habla = aparato("x", "Nombre nuevo", Some("c"));
        let juntos = juntar(&mios, &suyos, Some(&habla));
        assert_eq!(juntos[1].nombre, "Nombre nuevo");
        assert_eq!(juntos.len(), 3);
        // Quien habla y no estaba entra al final.
        let nuevo = aparato("n", "Nuevo", None);
        assert_eq!(juntar(&mios, &[], Some(&nuevo)).last().unwrap().id, "n");
    }

    #[test]
    fn los_choques_de_letra_se_dicen_y_sin_letra_no_hay_choque() {
        let miembros = [
            aparato("1", "Uno", Some("a")),
            aparato("2", "Dos", Some("b")),
            aparato("3", "Tres", Some("a")),
            aparato("4", "Cuatro", None),
            aparato("5", "Cinco", None),
        ];
        let ch = choques(&miembros);
        assert_eq!(ch.len(), 1);
        assert_eq!(ch[0].0, "a");
        let ids: Vec<&str> = ch[0].1.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "3"]);
        assert!(choques(&miembros[..2]).is_empty());
        assert_eq!(letras_ocupadas(&miembros), vec!['a', 'b']);
    }

    #[test]
    fn la_sena_junta_el_numero_con_la_letra_del_aparato() {
        assert_eq!(sena::de(47, 'a').as_deref(), Some("47a"));
        assert_eq!(sena::numero_de("47a"), Some(47));
        assert_eq!(sena::letra_de("47a"), Some('a'));
    }

    #[test]
    fn un_mensaje_sin_numero_no_tiene_sena() {
        // Cero es de antes de que los numeros existieran: inventarle un
        // `0a` lo haria chocar con todos los ceros de todos los aparatos.
        assert_eq!(sena::de(0, 'a'), None);
        assert_eq!(sena::numero_de("0a"), None);
        assert_eq!(sena::numero_de("hola"), None);
        assert_eq!(sena::letra_de("47"), None);
        assert_eq!(sena::letra_de("47A"), None);
        assert_eq!(sena::numero_de("-3a"), None);
        assert!(!sena::valida("a"));
    }

    #[test]
    fn dos_aparatos_nunca_reparten_la_misma_sena() {
        let del_telefono: HashSet<String> = (1..=50).filter_map(|n| sena::de(n, 'a')).collect();
        let de_la_tableta: HashSet<String> = (1..=50).filter_map(|n| sena::de(n, 't')).collect();
        assert!(del_telefono.is_disjoint(&de_la_tableta));
    }

    #[test]
    fn la_letra_se_reparte_sola_y_salta_las_ocupadas() {
        assert_eq!(sena::libre(&[]), Some('a'));
        assert_eq!(sena::libre(&['a', 'b']), Some('c'));
        let todas: Vec<char> = sena::LETRAS.chars().collect();
        assert_eq!(sena::libre(&todas), None, "veintiseis aparatos son todos");
        assert_eq!(todas.len(), crate::APARATOS_POR_GRUPO);
    }
}
