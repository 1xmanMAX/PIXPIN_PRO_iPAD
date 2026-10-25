//! **La semana de la ventana del horario**: lo puro de [`super::ventana`]
//! (18-oct-2026). Sin pintar nada, para poder probarlo sin ventana.
//!
//! - [`Semana`]: las asignaturas de la lista lateral y los **bloques** de la
//!   rejilla, uno por clase y dia. En el texto una clase puede ser de varios
//!   dias («Lun Mie 8:00-10:00 Geometría»); en la rejilla cada dia es un
//!   bloque suelto que se mueve y se alarga solo. Al escribir, los bloques
//!   iguales (misma asignatura, horas, aula, nota y aviso) vuelven a ir en
//!   una linea.
//! - [`Rejilla`]: donde cae cada dia y cada franja de 15 min, y al reves:
//!   que dia y que hora hay bajo el raton.
//! - [`reescribir`]: el `horario.md` con lo de la ventana, cambiando SOLO
//!   las lineas de clases y de asignaturas (ver
//!   [`super::leer_con_sitios`]): los titulos, los comentarios, las
//!   evaluaciones y el «avisar 10 min antes» del usuario se quedan.

#![forbid(unsafe_code)]

use pixpin_ia::acciones::normalizar;
use pixpin_ia::fechas::Momento;

/// Una caja en puntos (la `RectF` de `pixpin-render` de Windows).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

use super::{Asignatura, Clase, Horario, TITULO_ASIGNATURAS, TITULO_CLASES};

/// La primera y la ultima hora de la rejilla, en minutos del dia.
pub const DESDE: u32 = 7 * 60;
pub const HASTA: u32 = 22 * 60;
/// Una franja de la rejilla: todo se pega a esto.
pub const PASO: u32 = 15;
/// Lo que dura una clase recien soltada.
pub const DURACION: u32 = 60;
/// Cuantas franjas tiene la rejilla.
pub const FRANJAS: u32 = (HASTA - DESDE) / PASO;

/// Los dias como se escriben en el horario (y como se leen en la rejilla).
pub const DIAS: [&str; 7] = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];

/// Los colores de las asignaturas: vivos, para leer letra blanca encima.
pub const PALETA: [u32; 10] = [
    0x0A84FF, 0x30D158, 0xFF9F0A, 0xFF453A, 0xBF5AF2, 0x64D2FF, 0xFF375F, 0xAC8E68, 0x5E5CE6,
    0x66D4CF,
];

/// Una clase en un dia: lo que se ve como un bloque en la rejilla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloque {
    /// 0 lunes … 6 domingo.
    pub dia: u8,
    pub inicio: u32,
    pub fin: u32,
    pub nombre: String,
    pub aula: Option<String>,
    /// Lo que lee la voz al contestar la llamada (las lineas `>`).
    pub notas: Vec<String>,
    /// Minutos de antes del aviso; `None` es la del horario.
    pub antelacion: Option<u32>,
}

impl Bloque {
    fn clave(&self) -> (&str, u32, u32, &Option<String>, &Vec<String>, Option<u32>) {
        (
            &self.nombre,
            self.inicio,
            self.fin,
            &self.aula,
            &self.notas,
            self.antelacion,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Semana {
    pub asignaturas: Vec<Asignatura>,
    pub bloques: Vec<Bloque>,
}

/// Lo mismo sin mirar mayusculas ni tildes.
fn mismo(a: &str, b: &str) -> bool {
    normalizar(a.trim()) == normalizar(b.trim())
}

/// Al multiplo de [`PASO`] de abajo.
pub fn redondear_abajo(m: u32) -> u32 {
    m / PASO * PASO
}

/// Al multiplo de [`PASO`] mas cercano.
pub fn redondear(m: u32) -> u32 {
    (m + PASO / 2) / PASO * PASO
}

/// El color de una asignatura sin color propio: uno de la [`PALETA`] por
/// una huella de su nombre (FNV-1a, como `v2::color::de_clave`): el mismo
/// nombre, el mismo color siempre.
pub fn color_por_nombre(nombre: &str) -> u32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in normalizar(nombre.trim()).as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    PALETA[(h % PALETA.len() as u64) as usize]
}

impl Semana {
    /// La semana de un horario leido: sus asignaturas (tambien las que solo
    /// nombra una clase) y un bloque por clase y dia, ordenados por dia y
    /// hora.
    pub fn de_horario(h: &Horario) -> Semana {
        let mut bloques: Vec<Bloque> = h
            .clases
            .iter()
            .flat_map(|c| {
                c.dias.iter().map(move |&dia| Bloque {
                    dia,
                    inicio: c.inicio,
                    fin: c.fin,
                    nombre: c.nombre.clone(),
                    aula: c.aula.clone(),
                    notas: c.notas.clone(),
                    antelacion: c.antelacion,
                })
            })
            .collect();
        bloques.sort_by(|a, b| {
            (a.dia, a.inicio, &a.nombre).cmp(&(b.dia, b.inicio, &b.nombre))
        });
        Semana {
            asignaturas: h.todas_las_asignaturas(),
            bloques,
        }
    }

    /// Las clases para escribir: los bloques iguales en una sola, con todos
    /// sus dias. En el orden de su primer dia y su hora.
    pub fn clases(&self) -> Vec<Clase> {
        let mut clases: Vec<Clase> = Vec::new();
        let mut ordenados: Vec<&Bloque> = self.bloques.iter().collect();
        ordenados.sort_by(|a, b| (a.dia, a.inicio, &a.nombre).cmp(&(b.dia, b.inicio, &b.nombre)));
        let mut claves: Vec<(&str, u32, u32, &Option<String>, &Vec<String>, Option<u32>)> =
            Vec::new();
        for b in ordenados {
            let k = b.clave();
            match claves.iter().position(|x| *x == k) {
                Some(i) => {
                    if !clases[i].dias.contains(&b.dia) {
                        clases[i].dias.push(b.dia);
                        clases[i].dias.sort_unstable();
                    }
                }
                None => {
                    claves.push(k);
                    clases.push(Clase {
                        dias: vec![b.dia],
                        inicio: b.inicio,
                        fin: b.fin,
                        nombre: b.nombre.clone(),
                        aula: b.aula.clone(),
                        notas: b.notas.clone(),
                        antelacion: b.antelacion,
                    });
                }
            }
        }
        clases
    }

    /// La asignatura de ese nombre (sin mirar mayusculas ni tildes).
    pub fn asignatura(&self, nombre: &str) -> Option<usize> {
        self.asignaturas.iter().position(|a| mismo(&a.nombre, nombre))
    }

    /// El color de una asignatura (`0xRRGGBB`): el suyo o el de su nombre.
    pub fn color_de(&self, nombre: &str) -> u32 {
        self.asignatura(nombre)
            .and_then(|i| self.asignaturas[i].color)
            .unwrap_or_else(|| color_por_nombre(nombre))
    }

    /// **«Nueva asignatura»**: lo escrito, con el aula detras si se pone
    /// (`Geometría @A-12`). `Err` con la que ya estaba si se repite, o
    /// `Ok(None)` si no hay nombre.
    pub fn anadir_asignatura(&mut self, escrito: &str) -> Result<Option<usize>, usize> {
        let palabras: Vec<&str> = escrito.split_whitespace().collect();
        let (nombre, aula) = super::nombre_y_aula(&palabras);
        if nombre.is_empty() {
            return Ok(None);
        }
        if let Some(i) = self.asignatura(&nombre) {
            return Err(i);
        }
        self.asignaturas.push(Asignatura {
            nombre,
            aula,
            color: None,
        });
        Ok(Some(self.asignaturas.len() - 1))
    }

    /// Quita la asignatura `i` de la lista, con todas sus clases.
    pub fn borrar_asignatura(&mut self, i: usize) -> Option<Asignatura> {
        if i >= self.asignaturas.len() {
            return None;
        }
        let a = self.asignaturas.remove(i);
        self.bloques.retain(|b| !mismo(&b.nombre, &a.nombre));
        Some(a)
    }

    /// El color siguiente de la paleta para la asignatura `i` (pulsar su
    /// circulo).
    pub fn cambiar_color(&mut self, i: usize) {
        let Some(nombre) = self.asignaturas.get(i).map(|a| a.nombre.clone()) else {
            return;
        };
        let ahora = self.color_de(&nombre);
        let k = PALETA.iter().position(|&c| c == ahora).map_or(0, |k| (k + 1) % PALETA.len());
        self.asignaturas[i].color = Some(PALETA[k]);
    }

    /// Cambia el aula de la asignatura `i` (y de sus clases que tenian la de
    /// antes).
    pub fn poner_aula_de_asignatura(&mut self, i: usize, aula: Option<String>) {
        let Some(a) = self.asignaturas.get_mut(i) else {
            return;
        };
        let antes = std::mem::replace(&mut a.aula, aula.clone());
        let nombre = a.nombre.clone();
        for b in &mut self.bloques {
            if mismo(&b.nombre, &nombre) && b.aula == antes {
                b.aula = aula.clone();
            }
        }
    }

    /// **Soltar una asignatura en la rejilla**: un bloque de [`DURACION`]
    /// que empieza en la franja del `minuto` soltado, sin salirse de la
    /// rejilla. Devuelve su numero.
    pub fn soltar(&mut self, asignatura: usize, dia: u8, minuto: u32) -> Option<usize> {
        let a = self.asignaturas.get(asignatura)?;
        let inicio = redondear_abajo(minuto).clamp(DESDE, HASTA - PASO);
        let fin = (inicio + DURACION).min(HASTA);
        self.bloques.push(Bloque {
            dia: dia.min(6),
            inicio,
            fin,
            nombre: a.nombre.clone(),
            aula: a.aula.clone(),
            notas: Vec::new(),
            antelacion: None,
        });
        Some(self.bloques.len() - 1)
    }

    /// **Mover un bloque** al dia `dia` empezando en `inicio` (pegado a la
    /// franja mas cercana), con lo que duraba y sin salirse de la rejilla.
    pub fn mover(&mut self, i: usize, dia: u8, inicio: i64) {
        let Some(b) = self.bloques.get_mut(i) else {
            return;
        };
        let dura = b.fin.saturating_sub(b.inicio).clamp(PASO, HASTA - DESDE);
        let inicio = redondear(inicio.max(0) as u32).clamp(DESDE, HASTA - dura);
        b.dia = dia.min(6);
        b.inicio = inicio;
        b.fin = inicio + dura;
    }

    /// **Alargar o acortar un bloque** tirando de su borde de abajo: acaba
    /// en la franja mas cercana a `fin`, con un cuarto de hora como poco.
    pub fn alargar(&mut self, i: usize, fin: u32) {
        let Some(b) = self.bloques.get_mut(i) else {
            return;
        };
        b.fin = redondear(fin).clamp(b.inicio + PASO, HASTA.max(b.inicio + PASO));
    }

    pub fn borrar(&mut self, i: usize) -> Option<Bloque> {
        (i < self.bloques.len()).then(|| self.bloques.remove(i))
    }
}

// ------------------------------------------------------------- la rejilla

/// Donde esta la rejilla en la ventana: la esquina de arriba a la izquierda
/// de la primera franja del lunes, el ancho de cada dia y el alto de cada
/// franja de [`PASO`] minutos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rejilla {
    pub x: f32,
    pub y: f32,
    pub ancho_dia: f32,
    pub alto_franja: f32,
    /// Cuantos dias se ven (6: lunes a sabado; 7 con el domingo).
    pub dias: u8,
}

impl Rejilla {
    /// El minuto del dia a la altura `y` (sin pegar a la franja; puede
    /// salirse de la rejilla).
    pub fn minuto_en(&self, y: f32) -> i64 {
        DESDE as i64 + ((y - self.y) / self.alto_franja * PASO as f32).floor() as i64
    }

    /// **El dia y el minuto bajo el punto** `(x, y)`, o `None` fuera de la
    /// rejilla.
    pub fn en(&self, (x, y): (f32, f32)) -> Option<(u8, u32)> {
        if x < self.x || y < self.y {
            return None;
        }
        let dia = ((x - self.x) / self.ancho_dia) as u32;
        let m = self.minuto_en(y);
        (dia < u32::from(self.dias) && m < HASTA as i64).then_some((dia as u8, m as u32))
    }

    /// El dia bajo `x`, sin salirse de los que se ven (arrastrando por el
    /// borde, el bloque no se pierde).
    pub fn dia_cercano(&self, x: f32) -> u8 {
        (((x - self.x) / self.ancho_dia).floor().max(0.0) as u32).min(u32::from(self.dias) - 1) as u8
    }

    /// La y de un minuto del dia.
    pub fn y_de(&self, minuto: u32) -> f32 {
        self.y + (minuto as f32 - DESDE as f32) / PASO as f32 * self.alto_franja
    }

    /// La caja de un bloque.
    pub fn caja(&self, dia: u8, inicio: u32, fin: u32) -> RectF {
        let y0 = self.y_de(inicio.max(DESDE));
        let y1 = self.y_de(fin.min(HASTA));
        RectF {
            x: self.x + f32::from(dia) * self.ancho_dia,
            y: y0,
            ancho: self.ancho_dia,
            alto: (y1 - y0).max(0.0),
        }
    }

    /// Lo que mide toda la rejilla de alto.
    pub fn alto(&self) -> f32 {
        FRANJAS as f32 * self.alto_franja
    }
}

// ------------------------------------------------------------- escribir

/// «8:00».
fn hora(m: u32) -> String {
    format!("{}:{:02}", m / 60, m % 60)
}

/// El aula como se escribe: `@A-12`, o `@(Lab de física)` si lleva blancos.
fn aula_escrita(a: &str) -> String {
    if a.split_whitespace().count() > 1 {
        format!("@({})", a.trim())
    } else {
        format!("@{}", a.trim())
    }
}

/// La linea de una clase: `Lun Mie 8:00-10:00 Geometría @A-12 avisar 15`.
pub fn linea_de_clase(c: &Clase) -> String {
    let dias: Vec<&str> = c.dias.iter().map(|&d| DIAS[usize::from(d.min(6))]).collect();
    let mut s = format!("{} {}-{} {}", dias.join(" "), hora(c.inicio), hora(c.fin), c.nombre.trim());
    if let Some(a) = c.aula.as_deref().filter(|a| !a.trim().is_empty()) {
        s.push(' ');
        s.push_str(&aula_escrita(a));
    }
    if let Some(n) = c.antelacion {
        s.push_str(&format!(" avisar {n}"));
    }
    s
}

/// La linea de una asignatura: `Asignatura Geometría @A-12 #4F8EF7`.
pub fn linea_de_asignatura(a: &Asignatura) -> String {
    let mut s = format!("Asignatura {}", a.nombre.trim());
    if let Some(aula) = a.aula.as_deref().filter(|a| !a.trim().is_empty()) {
        s.push(' ');
        s.push_str(&aula_escrita(aula));
    }
    if let Some(c) = a.color {
        s.push_str(&format!(" #{c:06X}"));
    }
    s
}

/// Lo que escribe la ventana: la lista de asignaturas y las clases, cada
/// cosa con su titulo.
fn lo_de_la_ventana(s: &Semana) -> Vec<String> {
    let mut v = Vec::new();
    if !s.asignaturas.is_empty() {
        v.push(TITULO_ASIGNATURAS.to_string());
        v.extend(s.asignaturas.iter().map(linea_de_asignatura));
        v.push(String::new());
    }
    let clases = s.clases();
    if !clases.is_empty() {
        v.push(TITULO_CLASES.to_string());
        for c in &clases {
            v.push(linea_de_clase(c));
            v.extend(c.notas.iter().map(|n| format!("> {n}")));
        }
        v.push(String::new());
    }
    v
}

/// **El `horario.md` con lo de la ventana.** Las lineas de clases y de
/// asignaturas de `original` salen, y lo de la ventana entra donde estaba
/// la primera de ellas (o al final, si no habia ninguna). Lo demas, tal
/// cual. Sin dejar mas de un renglon en blanco seguido.
pub fn reescribir(original: &str, s: &Semana) -> String {
    let (_, suyas) = super::leer_con_sitios(original, Momento::de_ms_locales(0));
    let lineas: Vec<&str> = original.lines().collect();
    let nuevas = lo_de_la_ventana(s);
    let mut fuera: Vec<String> = Vec::new();
    let mut puesto = false;
    for (i, l) in lineas.iter().enumerate() {
        if suyas[i] {
            if !puesto {
                if fuera.last().is_some_and(|x| !x.trim().is_empty()) {
                    fuera.push(String::new());
                }
                fuera.extend(nuevas.iter().cloned());
                puesto = true;
            }
            continue;
        }
        fuera.push((*l).to_string());
    }
    if !puesto {
        if fuera.last().is_some_and(|x| !x.trim().is_empty()) {
            fuera.push(String::new());
        }
        fuera.extend(nuevas);
    }
    // Sin dos renglones en blanco seguidos (quitar lineas los junta).
    let mut limpio: Vec<String> = Vec::with_capacity(fuera.len());
    for l in fuera {
        if l.trim().is_empty() && limpio.last().is_none_or(|x: &String| x.trim().is_empty()) {
            continue;
        }
        limpio.push(l);
    }
    while limpio.last().is_some_and(|x| x.trim().is_empty()) {
        limpio.pop();
    }
    let mut t = limpio.join("\n");
    t.push('\n');
    t
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::horario::{PLANTILLA, leer};
    use pixpin_ia::fechas;

    fn ahora() -> Momento {
        Momento {
            dia: fechas::dias_de(2026, 10, 14),
            minuto: 10 * 60 + 30,
        }
    }

    /// Lunes a sabado, de 100 en 100 de ancho y 10 de alto por franja,
    /// empezando en (60, 100).
    fn rejilla() -> Rejilla {
        Rejilla {
            x: 60.0,
            y: 100.0,
            ancho_dia: 100.0,
            alto_franja: 10.0,
            dias: 6,
        }
    }

    fn semana_de(t: &str) -> Semana {
        Semana::de_horario(&leer(t, ahora()))
    }

    #[test]
    fn soltar_en_una_franja_da_el_dia_y_la_hora_correctos() {
        let r = rejilla();
        // Miercoles (tercera columna), a 4 franjas y media de las 7:00:
        // 8:07 → la franja de las 8:00.
        let punto = (60.0 + 2.0 * 100.0 + 50.0, 100.0 + 4.5 * 10.0);
        let (dia, minuto) = r.en(punto).unwrap();
        assert_eq!(dia, 2);
        assert_eq!(minuto, 8 * 60 + 7, "7:00 + 4,5 franjas = 8:07");
        let mut s = Semana::default();
        s.anadir_asignatura("Geometría @A-12").unwrap();
        let i = s.soltar(0, dia, minuto).unwrap();
        let b = &s.bloques[i];
        assert_eq!((b.dia, b.inicio, b.fin), (2, 7 * 60 + 60, 7 * 60 + 120), "8:00-9:00");
        assert_eq!((b.nombre.as_str(), b.aula.as_deref()), ("Geometría", Some("A-12")));
        // La caja del bloque cae donde se solto.
        let c = r.caja(b.dia, b.inicio, b.fin);
        assert_eq!((c.x, c.y, c.alto), (260.0, 140.0, 40.0));
        // Al final del dia no se sale: a las 21:50 dura lo que queda.
        let i = s.soltar(0, 5, 21 * 60 + 50).unwrap();
        assert_eq!((s.bloques[i].inicio, s.bloques[i].fin), (21 * 60 + 45, HASTA));
    }

    #[test]
    fn caso_negativo_fuera_de_la_rejilla_no_se_suelta() {
        let r = rejilla();
        assert_eq!(r.en((59.0, 150.0)), None, "en la columna de las horas");
        assert_eq!(r.en((100.0, 99.0)), None, "en la cabecera de los dias");
        assert_eq!(r.en((60.0 + 6.0 * 100.0 + 1.0, 150.0)), None, "el domingo no se ve");
        assert_eq!(r.en((100.0, 100.0 + r.alto() + 1.0)), None, "pasadas las 22:00");
        let con_domingo = Rejilla { dias: 7, ..r };
        assert_eq!(con_domingo.en((60.0 + 6.5 * 100.0, 100.0)), Some((6, DESDE)));
        let mut s = Semana::default();
        assert_eq!(s.soltar(3, 0, 600), None, "no hay asignatura 3");
        assert_eq!(s.anadir_asignatura("   "), Ok(None));
        s.anadir_asignatura("Física").unwrap();
        assert_eq!(s.anadir_asignatura("fisica"), Err(0), "repetida, sin tildes ni mayusculas");
    }

    #[test]
    fn mover_y_alargar_se_pegan_a_las_franjas_y_no_se_salen() {
        let mut s = semana_de("Lun 8:00-10:00 Geometría\n");
        s.mover(0, 3, 10 * 60 + 7);
        let b = &s.bloques[0];
        assert_eq!((b.dia, b.inicio, b.fin), (3, 10 * 60, 12 * 60), "jueves 10:00, sigue durando 2 h");
        s.mover(0, 9, 21 * 60);
        let b = &s.bloques[0];
        assert_eq!((b.dia, b.inicio, b.fin), (6, 20 * 60, HASTA), "no pasa de las 22:00 ni del domingo");
        s.mover(0, 0, -30);
        assert_eq!(s.bloques[0].inicio, DESDE, "ni antes de las 7:00");
        s.alargar(0, 9 * 60 + 53);
        assert_eq!(s.bloques[0].fin, 10 * 60, "9:53 se pega a las 10:00 (9:52 iria a las 9:45)");
        s.alargar(0, 6 * 60);
        assert_eq!(s.bloques[0].fin, DESDE + PASO, "un cuarto de hora como poco");
        s.alargar(0, 23 * 60 + 59);
        assert_eq!(s.bloques[0].fin, HASTA);
        // Arrastrar fuera por la derecha deja el bloque en el ultimo dia.
        let r = rejilla();
        assert_eq!(r.dia_cercano(5000.0), 5);
        assert_eq!(r.dia_cercano(-50.0), 0);
        assert!(s.borrar(0).is_some());
        assert_eq!(s.borrar(0), None);
    }

    const A_MANO: &str = "# Mi horario

Algo que escribí yo.

Lun Mie 8:00-10:00 Geometría @A-12
> traer el compás
Martes y Jueves 10:15 a 12:00 Física (Aula 7)
- Evaluación Geometría 23/10
avisar 15 min antes
<!-- un comentario
Lun 8-9 No soy clase
-->
Fin de la nota.
";

    #[test]
    fn guardar_y_volver_a_leer_no_pierde_nada() {
        let h = leer(A_MANO, ahora());
        let mut s = Semana::de_horario(&h);
        // Lo hecho en la ventana: un color, una asignatura nueva con su
        // clase de 2 h con aviso propio, y el aula de varias palabras.
        s.cambiar_color(0);
        s.anadir_asignatura("Dibujo técnico").unwrap();
        let i = s.soltar(2, 4, 16 * 60).unwrap();
        s.alargar(i, 18 * 60);
        s.bloques[i].antelacion = Some(30);
        s.bloques[i].aula = Some("Lab de dibujo".into());
        s.bloques[i].notas = vec!["llevar la escuadra".into()];
        let texto = reescribir(A_MANO, &s);
        // Lo del usuario sigue ahi, tal cual.
        for l in [
            "# Mi horario",
            "Algo que escribí yo.",
            "- Evaluación Geometría 23/10",
            "avisar 15 min antes",
            "<!-- un comentario",
            "Lun 8-9 No soy clase",
            "Fin de la nota.",
        ] {
            assert!(texto.lines().any(|x| x == l), "falta «{l}»:\n{texto}");
        }
        // Legible: una linea por clase, los dias juntos.
        assert!(texto.contains("\nLun Mié 8:00-10:00 Geometría @A-12\n> traer el compás\n"), "{texto}");
        assert!(texto.contains("Vie 16:00-18:00 Dibujo técnico @(Lab de dibujo) avisar 30"), "{texto}");
        // Y se lee igual que lo que habia en la ventana.
        let h2 = leer(&texto, ahora());
        let s2 = Semana::de_horario(&h2);
        let mut esperados = s.bloques.clone();
        esperados.sort_by(|a, b| (a.dia, a.inicio, &a.nombre).cmp(&(b.dia, b.inicio, &b.nombre)));
        assert_eq!(s2.bloques, esperados);
        assert_eq!(s2.asignaturas, s.asignaturas);
        assert_eq!(h2.antelacion, 15);
        assert_eq!(h2.evaluaciones.len(), 1);
        assert_eq!(h2.evaluaciones[0].clase, Some(0), "la evaluacion sigue con su clase");
        // Guardar otra vez lo mismo no cambia nada (ni repite titulos).
        assert_eq!(reescribir(&texto, &s2), texto);
        // El aviso propio de la clase manda sobre el del horario.
        let c = h2.clases.iter().find(|c| c.nombre == "Dibujo técnico").unwrap();
        assert_eq!(c.antelacion, Some(30));
    }

    #[test]
    fn la_plantilla_recibe_las_clases_y_conserva_el_ejemplo_comentado() {
        let mut s = semana_de(PLANTILLA);
        assert!(s.bloques.is_empty());
        s.anadir_asignatura("Inglés").unwrap();
        s.soltar(0, 0, 9 * 60);
        let t = reescribir(PLANTILLA, &s);
        assert!(t.contains("<!-- Ejemplo"), "{t}");
        assert!(t.contains("avisar 10 min antes"));
        assert!(t.contains("## Asignaturas\nAsignatura Inglés\n"), "{t}");
        assert!(t.contains("## Clases\nLun 9:00-10:00 Inglés\n"), "{t}");
        let h = leer(&t, ahora());
        assert_eq!(h.clases.len(), 1, "el ejemplo comentado sigue sin llamar");
        // Borrar la asignatura se lleva sus clases.
        s.borrar_asignatura(0);
        let t2 = reescribir(&t, &s);
        assert!(!t2.contains("Inglés"), "{t2}");
        assert!(t2.contains("<!-- Ejemplo"));
    }

    #[test]
    fn una_tabla_pegada_pasa_a_lineas_al_guardar_desde_la_ventana() {
        let t = "Hora\tLunes\tMartes\n8:00-10:00\tGeometría\t\nnota final\n";
        let s = semana_de(t);
        assert_eq!(s.bloques.len(), 1);
        let n = reescribir(t, &s);
        assert!(!n.contains('\t'), "{n}");
        assert!(n.contains("Lun 8:00-10:00 Geometría"));
        assert!(n.contains("nota final"));
    }
}
