//! **La hoja editable del pin**, sin ventana: lo que se escribe, lo que se
//! elige, el formato, las pestanas de las hojas y la pila de deshacer. La
//! pinta y le pasa el raton y el teclado el visor de hojas (`visor_hoja`,
//! que es el MISMO para ver y para editar); aqui todo se prueba sin GPU.
//!
//! Es lo que hacia la hoja de calculo pintada dentro del chat
//! (`HojaAbierta` de `ventana_chat`, que ya no existe), con lo que le
//! faltaba y tiene la de Android (`tabla/TablaActivity.kt`): deshacer y
//! rehacer, elegir un rango, negrita, alineacion (que pasa de una a otra
//! como en el movil: ninguna, izquierda, centro, derecha), el fondo con la
//! misma paleta, y las hojas de un libro en pestanas. Y lo que pidio el
//! usuario el 12-oct-2026 para el PC: cursiva, subrayado, tachado, color de
//! letra, formato de numero, ajustar texto y borrar formato (ver
//! `EstiloDeCelda` para como viaja al movil).
//!
//! **Deshacer es una pila de cambios de celda**: cada cambio guarda solo
//! las celdas que toco, como estaban y como quedaron. Barato aunque la tabla
//! tenga cuarenta mil celdas: escribir en una guarda una.
//!
//! En el Mac (D5), copia de `apps/pixpin/src/hoja_pin/editor.rs` de Windows,
//! igual salvo lo marcado «(Mac)»: aqui lo pinta y le pasa el teclado
//! `HojaCalculo.swift`, por [`crate::hojas::HojaDeCalculo`]. Lo de teclear
//! letra a letra (`teclear`, `retroceso`, `en_la_barra`…) no se usa: en el
//! Mac se escribe en la barra de formulas de AppKit y llega entero.
#![allow(dead_code)]

use std::cell::OnceCell;
use std::collections::BTreeMap;

use pixpin_proyecto::formula::{self, Valor};
use pixpin_proyecto::importar_hojas::{MAX_COLS, MAX_FILAS};
use pixpin_proyecto::tabla::{EstiloDeCelda, FormatoNumero, Ref, Tabla, ref_a, ref_de};

/// Cuantos cambios se pueden deshacer.
const MAXIMO_DESHACER: usize = 200;
/// Mas celdas que esto de una vez (negrita, borrar) no se tocan: un rango
/// de columnas enteras seria millones.
pub const MAXIMO_RANGO: u64 = 200_000;

/// Los fondos de «Resaltar»: la paleta del movil (`COLORES_DE_FONDO` de
/// `TablaActivity.kt`), en su orden, con su nombre para el menu.
pub const FONDOS: [(&str, &str); 8] = [
    ("#fff3bf", "hoja-color-amarillo"),
    ("#d3f9d8", "hoja-color-verde"),
    ("#d0ebff", "hoja-color-azul"),
    ("#ffe3e3", "hoja-color-rosa"),
    ("#f3d9fa", "hoja-color-morado"),
    ("#ffe8cc", "hoja-color-naranja"),
    ("#e9ecef", "hoja-color-gris"),
    ("#c5f6fa", "hoja-color-celeste"),
];

/// Los colores de letra: los fuertes de la paleta de Excel (el movil no
/// tiene color de letra).
pub const LETRAS: [(&str, &str); 6] = [
    ("#c00000", "hoja-color-rojo"),
    ("#0070c0", "hoja-color-azul"),
    ("#00b050", "hoja-color-verde"),
    ("#c65911", "hoja-color-naranja"),
    ("#7030a0", "hoja-color-morado"),
    ("#7f7f7f", "hoja-color-gris"),
];

/// Un rango de celdas elegido: donde se empezo (`ancla`) y la celda activa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seleccion {
    pub ancla: Ref,
    pub activa: Ref,
}

impl Seleccion {
    pub fn en(r: Ref) -> Seleccion {
        Seleccion { ancla: r, activa: r }
    }

    /// Las esquinas: (columna0, fila0, columna1, fila1), incluidas.
    pub fn esquinas(&self) -> (u32, u32, u32, u32) {
        let (a, b) = (self.ancla, self.activa);
        (a.columna.min(b.columna), a.fila.min(b.fila), a.columna.max(b.columna), a.fila.max(b.fila))
    }

    pub fn celdas(&self) -> u64 {
        let (c0, f0, c1, f1) = self.esquinas();
        (c1 - c0 + 1) as u64 * (f1 - f0 + 1) as u64
    }

    /// La misma, sin salirse de lo que cabe en una tabla (la columna `ZZ`
    /// y cien mil filas, como el movil).
    pub fn acotada(self) -> Seleccion {
        let a = |r: Ref| Ref { columna: r.columna.min(MAX_COLS - 1), fila: r.fila.min(MAX_FILAS - 1) };
        Seleccion { ancla: a(self.ancla), activa: a(self.activa) }
    }
}

/// Lo que habia y lo que hay en una celda: su contenido y su estilo.
type Foto = (Option<String>, Option<EstiloDeCelda>);

/// Un cambio que se puede deshacer: en que hoja, las celdas que toco (antes
/// y despues) y donde estaba la seleccion.
#[derive(Debug, Clone, PartialEq)]
struct Cambio {
    hoja: usize,
    celdas: Vec<(String, Foto, Foto)>,
    sel: Seleccion,
}

/// Una hoja del libro abierta para escribir.
pub struct HojaEditable {
    /// El nombre de la pestana.
    pub nombre: String,
    pub tabla: Tabla,
    /// El mensaje del chat que la guarda (la tabla ES su texto). `None`: un
    /// libro leido para editar que aun no se guardo en el chat.
    pub id_mensaje: Option<String>,
    /// Si cambio algo desde que se guardo.
    pub tocada: bool,
    pub sel: Seleccion,
    /// Lo que vale cada celda, calculado de una vez y olvidado al cambiar
    /// una.
    valores: OnceCell<BTreeMap<String, Valor>>,
}

impl HojaEditable {
    pub fn nueva(nombre: String, tabla: Tabla, id_mensaje: Option<String>) -> HojaEditable {
        HojaEditable { nombre, tabla, id_mensaje, tocada: false, sel: Seleccion::en(Ref { columna: 0, fila: 0 }), valores: OnceCell::new() }
    }

    /// Lo que vale cada celda (se calcula una vez).
    pub fn valores(&self) -> &BTreeMap<String, Valor> {
        self.valores.get_or_init(|| formula::evaluar_todo(&self.tabla))
    }

    /// Lo que se ve en la celda: el resultado de su formula, con el
    /// separador decimal del usuario.
    #[cfg(test)]
    pub fn mostrado(&self, r: Ref, decimal: char) -> String {
        let clave = ref_a(r);
        if !self.tabla.celdas.contains_key(&clave) {
            return String::new();
        }
        self.valores().get(&clave).map(|v| v.mostrar(decimal)).unwrap_or_default()
    }

    /// El estilo de la celda activa (para encender los botones).
    pub fn estilo_activo(&self) -> Option<&EstiloDeCelda> {
        self.tabla.estilos.get(&ref_a(self.sel.activa))
    }

    fn olvidar_valores(&mut self) {
        self.valores = OnceCell::new();
    }
}

/// Las celdas de `t` dentro de las esquinas, con su contenido y su estilo.
fn fotografiar(t: &Tabla, (c0, f0, c1, f1): (u32, u32, u32, u32)) -> BTreeMap<String, Foto> {
    let dentro = |k: &String| ref_de(k).is_some_and(|r| (c0..=c1).contains(&r.columna) && (f0..=f1).contains(&r.fila));
    let mut m: BTreeMap<String, Foto> = BTreeMap::new();
    for (k, v) in t.celdas.iter().filter(|(k, _)| dentro(k)) {
        m.entry(k.clone()).or_default().0 = Some(v.clone());
    }
    for (k, e) in t.estilos.iter().filter(|(k, _)| dentro(k)) {
        m.entry(k.clone()).or_default().1 = Some(e.clone());
    }
    m
}

fn poner_foto(t: &mut Tabla, clave: &str, (celda, estilo): &Foto) {
    match celda {
        Some(c) => {
            t.celdas.insert(clave.to_string(), c.clone());
        }
        None => {
            t.celdas.remove(clave);
        }
    }
    match estilo {
        Some(e) => {
            t.estilos.insert(clave.to_string(), e.clone());
        }
        None => {
            t.estilos.remove(clave);
        }
    }
}

/// La alineacion de una celda (el `a` del movil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineacion {
    Izquierda,
    Centro,
    Derecha,
}

impl Alineacion {
    pub fn letra(self) -> &'static str {
        match self {
            Alineacion::Izquierda => "i",
            Alineacion::Centro => "c",
            Alineacion::Derecha => "d",
        }
    }

    pub fn de(letra: &str) -> Option<Alineacion> {
        match letra {
            "i" => Some(Alineacion::Izquierda),
            "c" => Some(Alineacion::Centro),
            "d" => Some(Alineacion::Derecha),
            _ => None,
        }
    }

    /// La siguiente al pulsar «Alinear», como el movil: ninguna, izquierda,
    /// centro, derecha y otra vez ninguna.
    pub fn siguiente(a: Option<Alineacion>) -> Option<Alineacion> {
        match a {
            None => Some(Alineacion::Izquierda),
            Some(Alineacion::Izquierda) => Some(Alineacion::Centro),
            Some(Alineacion::Centro) => Some(Alineacion::Derecha),
            Some(Alineacion::Derecha) => None,
        }
    }
}

/// Un interruptor de formato: se pone si alguna celda de lo elegido no lo
/// tiene y se quita si ya lo tienen todas (como Excel y el movil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interruptor {
    Negrita,
    Cursiva,
    Subrayado,
    Tachado,
    Ajustar,
}

impl Interruptor {
    pub fn en(self, e: &EstiloDeCelda) -> bool {
        match self {
            Interruptor::Negrita => e.n,
            Interruptor::Cursiva => e.k,
            Interruptor::Subrayado => e.s,
            Interruptor::Tachado => e.t,
            Interruptor::Ajustar => e.w,
        }
    }

    fn poner(self, e: &mut EstiloDeCelda, si: bool) {
        match self {
            Interruptor::Negrita => e.n = si,
            Interruptor::Cursiva => e.k = si,
            Interruptor::Subrayado => e.s = si,
            Interruptor::Tachado => e.t = si,
            Interruptor::Ajustar => e.w = si,
        }
    }
}

/// **Un cambio de formato** en lo elegido (los botones y atajos de la barra
/// de formato del pin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formato {
    Alternar(Interruptor),
    /// El fondo (`#rrggbb`); `None`, sin color.
    Fondo(Option<String>),
    /// El color de la letra; `None`, el automatico.
    Letra(Option<String>),
    /// La alineacion; la misma otra vez la quita.
    Alinear(Alineacion),
    /// La siguiente alineacion (el boton, como el movil).
    AlinearSiguiente,
    /// El formato de numero; `None`, General.
    Numero(Option<FormatoNumero>),
    /// Quitar todo el formato.
    Borrar,
}

/// **El editor**: las hojas, la activa, lo que se esta tecleando y las
/// pilas de deshacer y rehacer.
pub struct Editor {
    pub hojas: Vec<HojaEditable>,
    pub activa: usize,
    /// Lo que se teclea en la celda activa. `None`: elegida, sin escribir
    /// (las flechas andan por la hoja).
    pub edicion: Option<String>,
    deshacer: Vec<Cambio>,
    rehacer: Vec<Cambio>,
    /// Sube con cada cambio (y con deshacer y rehacer): quien ensena la hoja
    /// sabe asi si lo que ve esta al dia.
    cambios: u64,
}

impl Editor {
    /// Un editor con estas hojas. Sin ninguna no hay nada que editar.
    pub fn nuevo(hojas: Vec<HojaEditable>, activa: usize) -> Option<Editor> {
        if hojas.is_empty() {
            return None;
        }
        let activa = activa.min(hojas.len() - 1);
        Some(Editor { hojas, activa, edicion: None, deshacer: Vec::new(), rehacer: Vec::new(), cambios: 0 })
    }

    pub fn hoja(&self) -> &HojaEditable {
        &self.hojas[self.activa]
    }

    pub fn hoja_mut(&mut self) -> &mut HojaEditable {
        &mut self.hojas[self.activa]
    }

    /// Cuantos cambios lleva (ver [`Editor::cambios`]).
    pub fn cambios(&self) -> u64 {
        self.cambios
    }

    pub fn puede_deshacer(&self) -> bool {
        !self.deshacer.is_empty()
    }

    pub fn puede_rehacer(&self) -> bool {
        !self.rehacer.is_empty()
    }

    /// Si alguna hoja cambio desde que se guardo.
    pub fn tocado(&self) -> bool {
        self.hojas.iter().any(|h| h.tocada)
    }

    /// Lo que se ve en la barra de formulas: lo que se teclea o lo escrito
    /// tal cual (la FORMULA, no su resultado).
    pub fn en_la_barra(&self) -> &str {
        match &self.edicion {
            Some(t) => t,
            None => self.hoja().tabla.celda(self.hoja().sel.activa),
        }
    }

    /// **Hace un cambio en las celdas de `esquinas`** de la hoja activa y lo
    /// apunta para deshacerlo. `false` si no cambio nada.
    fn cambiar(&mut self, esquinas: (u32, u32, u32, u32), f: impl FnOnce(&mut Tabla)) -> bool {
        let activa = self.activa;
        let h = &mut self.hojas[activa];
        let antes = fotografiar(&h.tabla, esquinas);
        let sel = h.sel;
        f(&mut h.tabla);
        let despues = fotografiar(&h.tabla, esquinas);
        let mut celdas = Vec::new();
        for k in antes.keys().chain(despues.keys().filter(|k| !antes.contains_key(*k))) {
            let a = antes.get(k).cloned().unwrap_or_default();
            let d = despues.get(k).cloned().unwrap_or_default();
            if a != d {
                celdas.push((k.clone(), a, d));
            }
        }
        if celdas.is_empty() {
            return false;
        }
        h.tocada = true;
        h.olvidar_valores();
        self.cambios += 1;
        self.deshacer.push(Cambio { hoja: activa, celdas, sel });
        if self.deshacer.len() > MAXIMO_DESHACER {
            self.deshacer.remove(0);
        }
        self.rehacer.clear();
        true
    }

    /// Deja escrito lo que se tecleaba. Escribir lo mismo que habia no
    /// cuenta como cambio (no se guarda ni se apunta).
    pub fn confirmar(&mut self) -> bool {
        let Some(texto) = self.edicion.take() else { return false };
        let texto = texto.trim().to_string();
        let r = self.hoja().sel.activa;
        self.cambiar((r.columna, r.fila, r.columna, r.fila), |t| t.poner(r, &texto))
    }

    /// Escapar con algo tecleado deja la celda como estaba.
    pub fn cancelar(&mut self) {
        self.edicion = None;
    }

    /// Empieza a escribir en la celda activa: con lo que tiene (F2, doble
    /// clic) o de cero (teclear encima, como en cualquier hoja).
    pub fn empezar(&mut self, con_lo_que_hay: bool) {
        if self.edicion.is_none() {
            let r = self.hoja().sel.activa;
            self.edicion = Some(if con_lo_que_hay { self.hoja().tabla.celda(r).to_string() } else { String::new() });
        }
    }

    pub fn teclear(&mut self, c: char) {
        self.edicion.get_or_insert_with(String::new).push(c);
    }

    /// Retroceso: borra una letra de lo tecleado; sin nada tecleado, vacia
    /// la celda para escribir de cero (como el chat).
    pub fn retroceso(&mut self) {
        match self.edicion.as_mut() {
            Some(t) => {
                t.pop();
            }
            None => self.edicion = Some(String::new()),
        }
    }

    /// Suprimir: vacia lo elegido (contenido; el estilo se queda, como en
    /// Excel). `false` si no habia nada o es demasiado.
    pub fn borrar_seleccion(&mut self) -> bool {
        self.edicion = None;
        let sel = self.hoja().sel;
        if sel.celdas() > MAXIMO_RANGO {
            return false;
        }
        let esquinas = sel.esquinas();
        self.cambiar(esquinas, |t| {
            let (c0, f0, c1, f1) = esquinas;
            t.celdas.retain(|k, _| !ref_de(k).is_some_and(|r| (c0..=c1).contains(&r.columna) && (f0..=f1).contains(&r.fila)));
        })
    }

    /// Mueve la celda activa (confirmando lo tecleado); con `extender`,
    /// alarga el rango desde el ancla. (En el pin mueve el visor; esto es
    /// para las pruebas.)
    #[cfg(test)]
    pub fn mover(&mut self, dx: i32, dy: i32, extender: bool) {
        self.confirmar();
        let h = self.hoja_mut();
        let a = h.sel.activa;
        let r = Ref {
            columna: (a.columna as i64 + dx as i64).clamp(0, MAX_COLS as i64 - 1) as u32,
            fila: (a.fila as i64 + dy as i64).clamp(0, MAX_FILAS as i64 - 1) as u32,
        };
        h.sel = if extender { Seleccion { ancla: h.sel.ancla, activa: r } } else { Seleccion::en(r) };
    }

    /// Elige una celda (un clic); con `extender` (Mayus), alarga el rango.
    #[cfg(test)]
    pub fn elegir(&mut self, r: Ref, extender: bool) {
        self.confirmar();
        let h = self.hoja_mut();
        h.sel = if extender { Seleccion { ancla: h.sel.ancla, activa: r } } else { Seleccion::en(r) };
    }

    /// Pone la seleccion que viene del visor (sin confirmar: quien la pone
    /// ya confirmo si hacia falta).
    pub fn poner_seleccion(&mut self, s: Seleccion) {
        self.hoja_mut().sel = s.acotada();
    }

    /// Cambia de hoja (confirmando lo tecleado en la que se deja).
    pub fn cambiar_hoja(&mut self, i: usize) {
        if i < self.hojas.len() && i != self.activa {
            self.confirmar();
            self.activa = i;
        }
    }

    /// **Un cambio de formato en lo elegido**, como un solo paso de
    /// deshacer. `false` si no cambio nada o el rango es demasiado.
    pub fn formato(&mut self, f: Formato) -> bool {
        self.confirmar();
        let sel = self.hoja().sel;
        if sel.celdas() > MAXIMO_RANGO {
            return false;
        }
        let (c0, f0, c1, f1) = sel.esquinas();
        let activo = self.hoja().estilo_activo().cloned().unwrap_or_default();
        match f {
            Formato::Alternar(i) => {
                let t = &self.hoja().tabla;
                let todas = (f0..=f1).all(|f| (c0..=c1).all(|c| t.estilos.get(&ref_a(Ref { columna: c, fila: f })).is_some_and(|e| i.en(e))));
                self.estilo_en_rango(|e| i.poner(e, !todas))
            }
            Formato::Fondo(color) => self.estilo_en_rango(|e| e.f = color.clone()),
            Formato::Letra(color) => self.estilo_en_rango(|e| e.c = color.clone()),
            Formato::Alinear(a) => {
                let ya = activo.a.as_deref() == Some(a.letra());
                self.estilo_en_rango(|e| e.a = if ya { None } else { Some(a.letra().to_string()) })
            }
            Formato::AlinearSiguiente => {
                let siguiente = Alineacion::siguiente(activo.a.as_deref().and_then(Alineacion::de));
                self.estilo_en_rango(|e| e.a = siguiente.map(|a| a.letra().to_string()))
            }
            Formato::Numero(n) => self.estilo_en_rango(|e| e.nf = n.map(|n| n.palabra().to_string())),
            Formato::Borrar => self.estilo_en_rango(EstiloDeCelda::borrar_formato),
        }
    }

    /// (Mac) Cambia el estilo de UNA celda como un paso de deshacer (el
    /// `poner_estilo` de antes del puente, que sigue para quien lo usa).
    pub fn estilo_de_celda(&mut self, r: Ref, f: impl FnOnce(&mut EstiloDeCelda)) -> bool {
        self.cambiar((r.columna, r.fila, r.columna, r.fila), |t| {
            let k = ref_a(r);
            let e = t.estilos.entry(k.clone()).or_default();
            f(e);
            if e.vacio() {
                t.estilos.remove(&k);
            }
        })
    }

    fn estilo_en_rango(&mut self, f: impl Fn(&mut EstiloDeCelda)) -> bool {
        let esquinas = self.hoja().sel.esquinas();
        self.cambiar(esquinas, |t| {
            let (c0, f0, c1, f1) = esquinas;
            for fila in f0..=f1 {
                for columna in c0..=c1 {
                    let k = ref_a(Ref { columna, fila });
                    let e = t.estilos.entry(k.clone()).or_default();
                    f(e);
                    if e.vacio() {
                        t.estilos.remove(&k);
                    }
                }
            }
        })
    }

    /// **Pega** unas filas de celdas con la esquina en la celda activa
    /// (`portapapeles_tabla::pegar`), como un solo cambio. Escribiendo, el
    /// texto va dentro de lo que se escribe.
    pub fn pegar(&mut self, filas: &[Vec<pixpin_proyecto::portapapeles_tabla::Pegada>]) -> bool {
        let en = self.hoja().sel.activa;
        let alto = filas.len() as u32;
        let ancho = filas.iter().map(|f| f.len()).max().unwrap_or(0) as u32;
        if alto == 0 || ancho == 0 {
            return false;
        }
        let esquinas = (en.columna, en.fila, (en.columna + ancho - 1).min(MAX_COLS - 1), (en.fila + alto - 1).min(MAX_FILAS - 1));
        let hecho = self.cambiar(esquinas, |t| {
            pixpin_proyecto::portapapeles_tabla::pegar(t, en, filas);
        });
        // Lo pegado queda elegido, como en Excel.
        let h = self.hoja_mut();
        h.sel = Seleccion { ancla: en, activa: Ref { columna: esquinas.2, fila: esquinas.3 } };
        hecho
    }

    /// **Deshace** el ultimo cambio: vuelve a su hoja y a su seleccion.
    pub fn deshacer(&mut self) -> bool {
        self.edicion = None;
        let Some(c) = self.deshacer.pop() else { return false };
        self.aplicar(&c, false);
        self.rehacer.push(c);
        true
    }

    pub fn rehacer(&mut self) -> bool {
        self.edicion = None;
        let Some(c) = self.rehacer.pop() else { return false };
        self.aplicar(&c, true);
        self.deshacer.push(c);
        true
    }

    fn aplicar(&mut self, c: &Cambio, adelante: bool) {
        let Some(h) = self.hojas.get_mut(c.hoja) else { return };
        for (k, antes, despues) in &c.celdas {
            poner_foto(&mut h.tabla, k, if adelante { despues } else { antes });
        }
        h.tocada = true;
        h.olvidar_valores();
        h.sel = c.sel;
        self.activa = c.hoja;
        self.cambios += 1;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(t: &str) -> Ref {
        ref_de(t).unwrap()
    }

    fn editor() -> Editor {
        let mut t = Tabla::default();
        t.poner(r("A1"), "Concepto");
        t.poner(r("B2"), "5");
        t.poner(r("B3"), "=B2*2");
        let otra = Tabla::default();
        Editor::nuevo(
            vec![HojaEditable::nueva("Gastos".into(), t, Some("1".into())), HojaEditable::nueva("Resumen".into(), otra, Some("2".into()))],
            0,
        )
        .unwrap()
    }

    #[test]
    fn escribir_deshacer_y_rehacer_celda_a_celda() {
        let mut e = editor();
        e.elegir(r("B2"), false);
        e.teclear('7');
        e.mover(0, 1, false);
        assert_eq!(e.hoja().tabla.celda(r("B2")), "7");
        assert_eq!(e.hoja().mostrado(r("B3"), '.'), "14", "la formula se recalcula");
        assert!(e.hoja().tocada);
        e.elegir(r("C1"), false);
        e.teclear('x');
        e.confirmar();
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celda(r("C1")), "", "deshacer quita lo ultimo");
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celda(r("B2")), "5");
        assert_eq!(e.hoja().mostrado(r("B3"), '.'), "10");
        assert_eq!(e.hoja().sel.activa, r("B2"), "deshacer vuelve a donde se escribio");
        // Caso negativo: no hay nada mas que deshacer.
        assert!(!e.deshacer());
        assert!(e.rehacer());
        assert_eq!(e.hoja().tabla.celda(r("B2")), "7");
        assert!(e.rehacer());
        assert_eq!(e.hoja().tabla.celda(r("C1")), "x");
        assert!(!e.rehacer());
    }

    #[test]
    fn escribir_lo_mismo_no_es_un_cambio_y_uno_nuevo_borra_lo_rehecho() {
        let mut e = editor();
        e.elegir(r("A1"), false);
        e.empezar(true);
        assert_eq!(e.en_la_barra(), "Concepto");
        assert!(!e.confirmar(), "lo mismo que habia no cuenta");
        assert!(!e.puede_deshacer());
        assert!(!e.hoja().tocada);
        assert_eq!(e.cambios(), 0);
        e.teclear('a');
        e.confirmar();
        assert_eq!(e.cambios(), 1, "cada cambio cuenta para quien lo ensena");
        e.deshacer();
        assert_eq!(e.cambios(), 2, "y deshacer tambien");
        assert!(e.puede_rehacer());
        e.teclear('b');
        e.confirmar();
        assert!(!e.puede_rehacer(), "un cambio nuevo tira lo que se podia rehacer");
        // Escapar no escribe nada.
        e.teclear('z');
        e.cancelar();
        assert_eq!(e.hoja().tabla.celda(r("A1")), "b");
    }

    #[test]
    fn deshacer_lleva_a_la_hoja_del_cambio() {
        let mut e = editor();
        e.cambiar_hoja(1);
        e.elegir(r("D4"), false);
        e.teclear('1');
        e.confirmar();
        e.cambiar_hoja(0);
        assert!(e.deshacer());
        assert_eq!(e.activa, 1);
        assert_eq!(e.hojas[1].tabla.celda(r("D4")), "");
        assert_eq!(e.hojas[0].tabla.celda(r("A1")), "Concepto", "la otra hoja no se toca");
    }

    #[test]
    fn negrita_alinear_y_borrar_un_rango_son_un_solo_cambio() {
        let mut e = editor();
        e.elegir(r("A1"), false);
        e.mover(1, 2, true);
        assert_eq!(e.hoja().sel.esquinas(), (0, 0, 1, 2));
        assert!(e.formato(Formato::Alternar(Interruptor::Negrita)));
        assert!(e.hoja().tabla.estilos["B3"].n);
        assert_eq!(e.hoja().tabla.estilos.len(), 6);
        assert!(e.formato(Formato::Alternar(Interruptor::Negrita)), "todo en negrita: se quita");
        assert!(e.hoja().tabla.estilos.is_empty(), "un estilo vacio no se guarda");
        assert!(e.formato(Formato::Alinear(Alineacion::Centro)));
        assert_eq!(e.hoja().tabla.estilos["A1"].a.as_deref(), Some("c"));
        assert!(e.borrar_seleccion());
        assert!(e.hoja().tabla.celdas.is_empty());
        assert_eq!(e.hoja().tabla.estilos.len(), 6, "borrar deja el formato");
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celdas.len(), 3, "un deshacer devuelve el rango entero");
        // Caso negativo: un rango gigante no se toca.
        e.elegir(r("A1"), false);
        e.elegir(r("ZZ100000"), true);
        assert!(!e.borrar_seleccion());
        assert!(!e.formato(Formato::Alternar(Interruptor::Negrita)));
    }

    #[test]
    fn cada_formato_se_pone_se_quita_y_se_deshace() {
        let mut e = editor();
        e.elegir(r("A1"), false);
        e.elegir(r("B2"), true);
        let a1 = |e: &Editor| e.hoja().tabla.estilos.get("A1").cloned().unwrap_or_default();
        for i in [Interruptor::Negrita, Interruptor::Cursiva, Interruptor::Subrayado, Interruptor::Tachado, Interruptor::Ajustar] {
            assert!(e.formato(Formato::Alternar(i)), "{i:?}");
            assert!(i.en(&a1(&e)), "{i:?} puesto");
            assert!(i.en(&e.hoja().tabla.estilos["B2"]), "{i:?} en todo el rango");
        }
        let e5 = e.cambios();
        // Si una celda del rango no lo tiene, se pone en todas (no se quita).
        e.hoja_mut().tabla.estilos.get_mut("B1").unwrap().k = false;
        assert!(e.formato(Formato::Alternar(Interruptor::Cursiva)));
        assert!(e.hoja().tabla.estilos["B1"].k && a1(&e).k);
        assert!(e.formato(Formato::Fondo(Some("#fff3bf".into()))));
        assert!(e.formato(Formato::Letra(Some("#c00000".into()))));
        assert!(e.formato(Formato::Numero(Some(FormatoNumero::Porcentaje))));
        let b2 = e.hoja().tabla.estilos["B2"].clone();
        assert_eq!((b2.f.as_deref(), b2.c.as_deref(), b2.nf.as_deref()), (Some("#fff3bf"), Some("#c00000"), Some("porcentaje")));
        assert!(e.cambios() > e5);
        // Alinear con el boton pasa por las tres y vuelve a ninguna.
        let mut vistas = Vec::new();
        for _ in 0..4 {
            e.formato(Formato::AlinearSiguiente);
            vistas.push(a1(&e).a);
        }
        assert_eq!(vistas, [Some("i".into()), Some("c".into()), Some("d".into()), None]);
        // Deshacer quita el ultimo formato de todo el rango de una vez.
        e.formato(Formato::Letra(None));
        assert_eq!(e.hoja().tabla.estilos["B2"].c, None, "el automatico es sin color");
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.estilos["B2"].c.as_deref(), Some("#c00000"));
        assert!(e.rehacer());
        // Borrar formato lo quita todo, y se deshace.
        assert!(e.formato(Formato::Borrar));
        assert!(e.hoja().tabla.estilos.is_empty(), "sin formato no queda estilo");
        assert!(e.deshacer());
        assert!(e.hoja().tabla.estilos["B2"].n);
        // Casos negativos: borrar el formato de lo que no tiene no es un
        // cambio, y poner el fondo que ya hay tampoco.
        e.elegir(r("H9"), false);
        assert!(!e.formato(Formato::Borrar));
        e.formato(Formato::Fondo(Some("#d3f9d8".into())));
        assert!(!e.formato(Formato::Fondo(Some("#d3f9d8".into()))));
        assert!(!e.formato(Formato::Numero(None)), "General sin formato no cambia nada");
    }

    #[test]
    fn el_formato_se_guarda_en_la_tabla_del_chat_y_vuelve() {
        let mut e = editor();
        e.elegir(r("B2"), false);
        e.formato(Formato::Alternar(Interruptor::Tachado));
        e.formato(Formato::Numero(Some(FormatoNumero::Moneda)));
        let texto = e.hoja().tabla.escribir().unwrap();
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        assert_eq!(v["estilos"]["B2"]["t"], true);
        assert_eq!(v["estilos"]["B2"]["nf"], "moneda");
        let vuelta = Tabla::leer(&texto).unwrap();
        assert_eq!(vuelta.estilos["B2"].formato_numero(), Some(FormatoNumero::Moneda));
        assert!(vuelta.estilos["B2"].t);
    }

    #[test]
    fn pegar_se_deshace_de_una_vez() {
        use pixpin_proyecto::portapapeles_tabla::Pegada;
        let mut e = editor();
        e.elegir(r("B2"), false);
        let p = |t: &str| Pegada { texto: t.into(), ..Default::default() };
        assert!(e.pegar(&[vec![p("1"), p("2")], vec![p("3")]]));
        assert_eq!(e.hoja().tabla.celda(r("C2")), "2");
        assert_eq!(e.hoja().sel.esquinas(), (1, 1, 2, 2));
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celda(r("B2")), "5");
        assert_eq!(e.hoja().tabla.celda(r("C2")), "");
        assert!(!e.pegar(&[]), "nada que pegar");
    }

    #[test]
    fn la_seleccion_del_visor_no_se_sale_de_la_tabla() {
        let mut e = editor();
        let lejos = Seleccion { ancla: r("A1"), activa: Ref { columna: 16_000, fila: 1_000_000 } };
        e.poner_seleccion(lejos);
        assert_eq!(e.hoja().sel.activa, Ref { columna: MAX_COLS - 1, fila: MAX_FILAS - 1 });
        assert!(Editor::nuevo(Vec::new(), 0).is_none(), "sin hojas no hay editor");
    }
}
