//! **Lo que mas se repitio** en un mes (5-oct-2026): el «Mostly "Studying"»
//! de la pestana «Estado» de WeChat que el usuario mando como ejemplo, aqui
//! «Sobre todo «X»».
//!
//! Primero mandan los emoticonos: son lo que el usuario pone a proposito
//! para decir como estuvo. Si el mes no tiene ninguno, la palabra mas
//! repetida en los titulos, de cuatro letras o mas y sin las de relleno
//! («para», «como», «pero») ni las que pone la app sola en los titulos sin
//! texto («Nota de voz», «Foto»): esas saldrian siempre y no dicen nada.

use std::collections::HashMap;

use crate::Momento;
use crate::buscar::normalizar;
use crate::emoticonos;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoMas {
    Emoticono(String),
    Palabra(String),
}

impl LoMas {
    pub fn texto(&self) -> &str {
        match self {
            LoMas::Emoticono(e) | LoMas::Palabra(e) => e,
        }
    }
}

/// Las de relleno, ya sin tildes. Solo las de cuatro letras o mas: las
/// cortas ya se quedan fuera por largo.
const VACIAS: &[&str] = &[
    // castellano
    "para", "como", "pero", "esta", "este", "esto", "estos", "estas", "estaba", "todo", "toda",
    "todos", "todas", "cuando", "donde", "porque", "sobre", "entre", "hasta", "desde", "tiene",
    "tengo", "hace", "hacer", "algo", "otra", "otro", "otros", "otras", "mucho", "mucha", "poco",
    "bien", "solo", "luego", "despues", "antes", "ahora", "tambien", "aunque", "mismo", "misma",
    "cada", "nada", "muy", "sido", "estoy", "estar", "fuimos", "fueron", "eran", "habia", "hubo",
    "nota", "foto", "fotos", "audio", // ingles
    "that", "this", "with", "from", "have", "there", "what", "when", "were", "been", "just",
    "about", "into", "then", "them", "they", "your", "will", "would", "some", "more", "note",
    "voice", "photo",
];

/// Lo que mas se repite en `momentos`, o `None` si no hay nada que contar.
/// A igualdad, gana lo que salio antes (en el orden que llegan).
pub fn lo_mas_repetido(momentos: &[&Momento]) -> Option<LoMas> {
    lo_mas_con_cuenta(momentos).map(|(l, _)| l)
}

/// Como [`lo_mas_repetido`], con cuantas veces salio: la chapa «🍕 ×6» de la
/// pestana «Estado».
pub fn lo_mas_con_cuenta(momentos: &[&Momento]) -> Option<(LoMas, usize)> {
    // (cuantas, orden de aparicion, como se ensena)
    let mut emos: HashMap<String, (usize, usize, String)> = HashMap::new();
    let mut n = 0;
    for m in momentos {
        for t in [&m.titulo, &m.descripcion] {
            for e in emoticonos::emoticonos(t) {
                n += 1;
                let entrada = emos
                    .entry(emoticonos::clave(e))
                    .or_insert((0, n, e.to_string()));
                entrada.0 += 1;
            }
        }
    }
    if let Some((e, n)) = ganador(emos) {
        return Some((LoMas::Emoticono(e), n));
    }
    let mut palabras: HashMap<String, (usize, usize, String)> = HashMap::new();
    for m in momentos {
        for p in m.titulo.split(|c: char| !c.is_alphanumeric()) {
            if p.chars().count() < 4 || p.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let k = normalizar(p);
            if VACIAS.contains(&k.as_str()) {
                continue;
            }
            n += 1;
            let entrada = palabras.entry(k).or_insert((0, n, p.to_lowercase()));
            entrada.0 += 1;
        }
    }
    ganador(palabras).map(|(p, n)| (LoMas::Palabra(p), n))
}

fn ganador(m: HashMap<String, (usize, usize, String)>) -> Option<(String, usize)> {
    m.into_values()
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(n, _, t)| (t, n))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(titulo: &str, desc: &str) -> Momento {
        Momento::nuevo("x".into(), 0, titulo.into(), desc.into())
    }

    #[test]
    fn manda_el_emoticono_mas_repetido() {
        let v = [
            m("Estudiando 📚", ""),
            m("Cena", "rica 😋"),
            m("Mas estudio", "📚📚"),
        ];
        let r: Vec<&Momento> = v.iter().collect();
        assert_eq!(lo_mas_repetido(&r), Some(LoMas::Emoticono("📚".into())));
        assert_eq!(lo_mas_con_cuenta(&r), Some((LoMas::Emoticono("📚".into()), 3)));
    }

    #[test]
    fn sin_emoticonos_la_palabra_mas_repetida_de_los_titulos() {
        let v = [
            m("Estudiando para el examen", ""),
            m("Sigo estudiando", ""),
            m("Estudiando cálculo", "estudiar estudiar estudiar"),
        ];
        let r: Vec<&Momento> = v.iter().collect();
        assert_eq!(
            lo_mas_repetido(&r),
            Some(LoMas::Palabra("estudiando".into()))
        );
    }

    #[test]
    fn caso_negativo_las_de_relleno_y_las_cortas_no_cuentan() {
        let v = [
            m("Para la casa", ""),
            m("Para el gym", ""),
            m("Nota de voz", ""),
        ];
        let r: Vec<&Momento> = v.iter().collect();
        // «para» y «nota» no valen; «casa» sale una vez, igual que nada mas:
        // gana la primera palabra que vale.
        assert_eq!(lo_mas_repetido(&r), Some(LoMas::Palabra("casa".into())));
        assert_eq!(lo_mas_repetido(&[]), None);
        let solo_relleno = [m("Nota de voz", ""), m("Foto", "")];
        let r: Vec<&Momento> = solo_relleno.iter().collect();
        assert_eq!(lo_mas_repetido(&r), None);
    }

    #[test]
    fn el_corazon_con_y_sin_selector_cuenta_junto() {
        let v = [m("❤", ""), m("❤️", ""), m("😀", "")];
        let r: Vec<&Momento> = v.iter().collect();
        assert_eq!(lo_mas_repetido(&r), Some(LoMas::Emoticono("❤".into())));
    }
}
