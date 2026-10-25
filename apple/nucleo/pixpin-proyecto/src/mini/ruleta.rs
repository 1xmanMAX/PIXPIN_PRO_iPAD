//! La ruleta: una lista de nombres y un sorteo. Puerto de `mini/Ruleta.kt` y
//! del `RuletaDoc` de `mini/Contador.kt`.
//!
//! El documento es **un nombre por linea, a pelo**: ni vinetas ni claves. Es
//! el unico de las siete mini-apps que no lleva marca ninguna, y esta bien
//! asi: una lista de nombres se entiende sola abierta con cualquier cosa.
//!
//! El azar entra **por fuera**, como un numero. Asi una prueba puede decir
//! «que salga el tercero» y comprobar de verdad lo que pasa despues, en vez de
//! girar mil veces y confiar. Ademas, el sorteo tiene que ser el mismo se abra
//! desde donde se abra: una ruleta que reparte distinto segun la ventana no
//! seria la misma ruleta.

use super::{Resumen, cuerpo, en_una_linea, linea_de_titulo};

/// Cuantos nombres caben. Mas alla, ni se leen ni se distinguen al girar.
pub const MAXIMO: usize = 40;

/// Las vueltas de adorno que da la rueda antes de parar. No cambian a quien le
/// toca —ya esta decidido—, pero sin ellas la rueda saltaria a la respuesta.
pub const VUELTAS_DE_ADORNO: f32 = 4.0;

/// Los nombres de un texto: **uno por linea, o separados por comas**.
///
/// Aceptar la coma y el punto y coma no es un capricho. Escribiendo «ana,
/// luis, marta» salia **un solo nombre**, y con uno solo no hay sorteo: el
/// boton de girar se quedaba apagado y no pasaba nada al tocarlo, sin decir
/// por que. Uno escribe una lista como le sale, y las tres formas son la misma
/// lista.
///
/// Se quitan las lineas en blanco y los espacios de los lados, que es lo que
/// sobra al pegar una lista de cualquier sitio. Los repetidos **se quedan**:
/// si alguien pone dos veces a la misma persona es porque quiere que tenga el
/// doble de posibilidades.
pub fn nombres(texto: &str) -> Vec<String> {
    texto
        .split(['\n', ',', ';'])
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .take(MAXIMO)
        .collect()
}

/// Los nombres de un documento, sin su titulo.
pub fn leer(documento: &str) -> Vec<String> {
    nombres(cuerpo(documento))
}

/// El documento entero. Cada nombre en una linea, y nada mas.
pub fn escribir(titulo: &str, nombres: &[String]) -> String {
    let cuerpo = nombres
        .iter()
        .map(|n| en_una_linea(n))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}{cuerpo}", linea_de_titulo(titulo))
}

/// Anade un nombre. Lo vacio no entra y el tope es el de la ruleta.
///
/// Pasado ese tope los nombres ni se leen ni se distinguen al girar, asi que
/// aceptar el cuarenta y uno seria aceptar algo que no se va a poder usar.
pub fn anadir(nombres: &[String], texto: &str) -> Vec<String> {
    let limpio = en_una_linea(texto);
    if limpio.is_empty() || nombres.len() >= MAXIMO {
        return nombres.to_vec();
    }
    let mut lista = nombres.to_vec();
    lista.push(limpio);
    lista
}

pub fn quitar(nombres: &[String], cual: usize) -> Vec<String> {
    if cual >= nombres.len() {
        return nombres.to_vec();
    }
    let mut lista = nombres.to_vec();
    lista.remove(cual);
    lista
}

/// ¿Hay con que sortear? Con uno solo no hay sorteo que valga.
pub fn se_puede_girar(texto: &str) -> bool {
    nombres(texto).len() >= 2
}

/// A quien le toca. `azar` es un numero de 0 (incluido) a 1 (excluido) que
/// **entra por parametro**: aqui dentro no hay ni reloj ni azar.
///
/// `None` si no hay a quien elegir.
pub fn elegir(nombres: &[String], azar: f64) -> Option<usize> {
    if nombres.is_empty() {
        return None;
    }
    let cuantos = nombres.len();
    // Se acota porque `azar` viene de fuera: un 1.0 justo, o un numero raro,
    // senalaria a un hueco que no existe y el sorteo se caeria en vez de
    // elegir a alguien.
    let n = (azar * cuantos as f64) as i64;
    Some(n.clamp(0, cuantos as i64 - 1) as usize)
}

/// Cuanto ocupa cada porcion, en grados.
pub fn angulo_de_porcion(cuantos: usize) -> f32 {
    if cuantos == 0 {
        360.0
    } else {
        360.0 / cuantos as f32
    }
}

/// Donde tiene que quedarse la rueda para que la aguja senale al elegido.
///
/// La aguja esta arriba y quieta; lo que gira es la rueda. Asi que el angulo
/// es el que lleva el **centro de esa porcion** hasta arriba: el negativo de
/// donde esta, mas las vueltas enteras que se dan para que se vea girar.
pub fn angulo_final(elegido: usize, cuantos: usize, vueltas: f32) -> f32 {
    if cuantos == 0 {
        return 0.0;
    }
    let porcion = angulo_de_porcion(cuantos);
    let centro = porcion * elegido as f32 + porcion / 2.0;
    vueltas * 360.0 - centro
}

/// El texto sin el nombre `cual`, para seguir sorteando entre los que quedan.
///
/// Se rehace desde los nombres limpios en vez de tocar el texto original:
/// quitar una linea a mano dejaria los espacios y los renglones vacios que
/// hubiera alrededor, y a la vuelta la lista ya no cuadraria con lo que se ve.
pub fn sin_el_nombre(texto: &str, cual: usize) -> String {
    let quedan = nombres(texto);
    if cual >= quedan.len() {
        return texto.to_string();
    }
    quitar(&quedan, cual).join("\n")
}

/// Cuantos entran en el sorteo. El texto va vacio a proposito: una ruleta no
/// tiene un numero que ensenar hasta que se gira.
pub fn resumen(documento: &str) -> Resumen {
    let lista = leer(documento);
    Resumen {
        texto: String::new(),
        de: lista.len(),
        vacia: lista.is_empty(),
        ..Resumen::default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lista(nombres: &[&str]) -> Vec<String> {
        nombres.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn el_documento_de_la_ruleta_es_un_nombre_por_linea() {
        let d = escribir("Quién friega", &lista(&["Ana", "Luis", "Marta"]));
        assert_eq!(d, "# Quién friega\n\nAna\nLuis\nMarta");
        assert_eq!(leer(&d), lista(&["Ana", "Luis", "Marta"]));
    }

    #[test]
    fn nace_vacia_y_con_su_titulo() {
        let d = escribir("Sorteo", &[]);
        assert_eq!(d, "# Sorteo\n\n");
        assert!(resumen(&d).vacia);
        assert!(!se_puede_girar(cuerpo(&d)));
    }

    #[test]
    fn una_lista_pegada_con_comas_sigue_siendo_una_lista() {
        let leidos = nombres("ana, luis; marta\n\n  pepe  ");
        assert_eq!(leidos, lista(&["ana", "luis", "marta", "pepe"]));
        assert!(se_puede_girar("ana, luis"));
        assert!(!se_puede_girar("  ana  "), "con uno solo no hay sorteo");
        assert!(!se_puede_girar(""));
    }

    #[test]
    fn los_repetidos_se_quedan_y_el_tope_corta() {
        assert_eq!(nombres("ana\nana").len(), 2, "el doble de posibilidades");
        let muchos = (0..60)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(nombres(&muchos).len(), MAXIMO);
        let llena = nombres(&muchos);
        assert_eq!(anadir(&llena, "uno mas").len(), MAXIMO, "no cabe");
    }

    #[test]
    fn lo_vacio_no_entra_y_un_parrafo_pegado_no_parte_el_documento() {
        assert_eq!(anadir(&lista(&["ana"]), "   "), lista(&["ana"]));
        let con = anadir(&[], "una cosa\ny otra");
        assert_eq!(con, lista(&["una cosa y otra"]), "una linea, un nombre");
        assert_eq!(leer(&escribir("T", &con)).len(), 1);
    }

    #[test]
    fn el_sorteo_es_el_numero_que_le_dan() {
        let l = lista(&["ana", "luis", "marta"]);
        assert_eq!(elegir(&l, 0.0), Some(0));
        assert_eq!(elegir(&l, 0.5), Some(1));
        assert_eq!(elegir(&l, 0.99), Some(2));
        // Casos negativos: un azar fuera de sitio no puede caerse.
        assert_eq!(elegir(&l, 1.0), Some(2), "el 1.0 justo senala un hueco");
        assert_eq!(elegir(&l, -3.0), Some(0));
        assert_eq!(elegir(&l, f64::NAN), Some(0));
        assert_eq!(elegir(&[], 0.5), None, "no hay a quien elegir");
    }

    #[test]
    fn quitar_al_elegido_deja_la_lista_limpia() {
        assert_eq!(sin_el_nombre("ana\n\n  luis , marta", 1), "ana\nmarta");
        // Uno que no existe deja el texto tal cual.
        assert_eq!(sin_el_nombre("ana\nluis", 7), "ana\nluis");
        assert_eq!(quitar(&lista(&["ana"]), 7), lista(&["ana"]));
    }

    #[test]
    fn la_rueda_para_con_el_elegido_arriba() {
        assert_eq!(angulo_de_porcion(4), 90.0);
        assert_eq!(angulo_de_porcion(0), 360.0, "sin nadie, una porcion entera");
        // El primero de cuatro: su centro esta a 45 grados, asi que la rueda
        // retrocede eso despues de las vueltas de adorno.
        assert_eq!(angulo_final(0, 4, VUELTAS_DE_ADORNO), 4.0 * 360.0 - 45.0);
        assert_eq!(angulo_final(0, 0, VUELTAS_DE_ADORNO), 0.0);
    }
}
