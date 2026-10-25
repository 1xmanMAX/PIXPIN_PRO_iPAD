//! **Buscar dentro de todos los chats** desde el buscador de la lista, como
//! en Telegram: debajo de los chats que casan por su nombre sale una seccion
//! «Mensajes» con los mensajes de CUALQUIER chat que llevan lo escrito, del
//! mas nuevo al mas viejo, con su trozo de texto y lo encontrado resaltado.
//!
//! Aqui solo la parte pura, la que se prueba sin ventana: el indice ya
//! aplanado (minusculas y sin tildes, con las mismas reglas que
//! `resaltado`), la busqueda con su tope, el trozo que se ensena de cada
//! mensaje, la espera entre tecla y tecla y como se reparte la seccion en
//! la lista. Quien lee los cuadernos del disco y pinta es la app.
//!
//! Decisiones que no son evidentes:
//!
//! - **Se aplana una vez, al indexar**, y no en cada busqueda: con cien mil
//!   mensajes, normalizar todo en cada tecla se notaria. Buscar es entonces
//!   un `contains` sobre texto ya aplanado, que es lo mas rapido que hay sin
//!   montar un indice invertido.
//! - **El aplanado es de un caracter a uno** (como `resaltado`): asi «casa»
//!   en lo aplanado es «casa» en el original, y los tramos que se resaltan
//!   salen de `resaltado::coincidencias` sobre el texto de verdad.
//! - **Los blancos cuentan como espacio**: buscar «hola mundo» encuentra
//!   «hola⏎mundo», que en una linea de resultado se lee igual.
//! - **Se cuenta todo, se devuelve hasta el tope**: la seccion dice cuantos
//!   hay de verdad y «Ver más» pide el siguiente tramo.

use crate::resaltado::{self, Tramo};

/// Un trozo de texto de un mensaje (su texto, su transcripcion, el nombre
/// de su fichero), tal cual y aplanado para buscar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pieza {
    pub texto: String,
    plano: String,
}

impl Pieza {
    pub fn nueva(texto: &str) -> Pieza {
        Pieza {
            texto: texto.to_string(),
            plano: aplanar(texto),
        }
    }
}

/// Lo que se sabe de un mensaje para buscarlo y llevar a el.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// El chat donde esta (el `id` de su ficha).
    pub chat: String,
    /// Por lo que se le encuentra al abrir el chat (su codigo unico).
    pub codigo: String,
    /// Milisegundos desde 1970: ordena del mas nuevo al mas viejo.
    pub cuando: i64,
    pub piezas: Vec<Pieza>,
}

impl Entrada {
    /// Una entrada con los textos que se le den, sin los vacios ni los
    /// repetidos (la transcripcion suele estar tambien en el texto).
    pub fn nueva<'a>(
        chat: &str,
        codigo: &str,
        cuando: i64,
        textos: impl IntoIterator<Item = &'a str>,
    ) -> Entrada {
        let mut piezas: Vec<Pieza> = Vec::new();
        for t in textos {
            let t = t.trim();
            if t.is_empty() || piezas.iter().any(|p| p.texto == t) {
                continue;
            }
            piezas.push(Pieza::nueva(t));
        }
        Entrada {
            chat: chat.to_string(),
            codigo: codigo.to_string(),
            cuando,
            piezas,
        }
    }

    /// La primera pieza donde esta lo buscado (ya aplanado).
    fn pieza_con(&self, aguja_plana: &str) -> Option<usize> {
        self.piezas
            .iter()
            .position(|p| p.plano.contains(aguja_plana))
    }
}

/// El texto en minusculas y sin tildes, con cada blanco hecho espacio: de un
/// caracter a uno, para que lo encontrado aqui caiga en el mismo sitio del
/// original.
pub fn aplanar(texto: &str) -> String {
    texto
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                ' '
            } else {
                resaltado::normalizar(c)
            }
        })
        .collect()
}

/// Lo que se busca, ya aplanado; `None` si no hay nada que buscar (la aguja
/// en blanco no encuentra nada, como en `resaltado`).
pub fn aguja_plana(aguja: &str) -> Option<String> {
    let a = aguja.trim();
    (!a.is_empty()).then(|| aplanar(a))
}

/// Un mensaje encontrado: cual (por su posicion en lo buscado) y en que
/// pieza esta lo escrito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Acierto {
    pub entrada: usize,
    pub pieza: usize,
}

/// Lo que dio una busqueda: los primeros `tope` y cuantos hay en total.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hallado {
    pub aciertos: Vec<Acierto>,
    pub total: usize,
}

impl Hallado {
    /// Si quedan por ensenar («Ver más»).
    pub fn hay_mas(&self) -> bool {
        self.total > self.aciertos.len()
    }
}

/// **Busca** `aguja` en las entradas, que tienen que venir ya del mas nuevo
/// al mas viejo (es el orden en que se ensenan). Devuelve los `tope`
/// primeros y cuenta todos.
pub fn buscar<'a>(
    entradas: impl IntoIterator<Item = &'a Entrada>,
    aguja: &str,
    tope: usize,
) -> Hallado {
    let Some(plana) = aguja_plana(aguja) else {
        return Hallado::default();
    };
    let mut hallado = Hallado::default();
    for (i, e) in entradas.into_iter().enumerate() {
        if let Some(pieza) = e.pieza_con(&plana) {
            hallado.total += 1;
            if hallado.aciertos.len() < tope {
                hallado.aciertos.push(Acierto { entrada: i, pieza });
            }
        }
    }
    hallado
}

/// Ordena las entradas como se buscan: lo mas nuevo primero y, a la par,
/// por chat y codigo, para que dos busquedas iguales den lo mismo.
pub fn ordenar(entradas: &mut [&Entrada]) {
    entradas.sort_by(|a, b| comparar(a, b));
}

/// El orden de `ordenar`, para quien ordena sus propios indices.
pub fn comparar(a: &Entrada, b: &Entrada) -> std::cmp::Ordering {
    b.cuando
        .cmp(&a.cuando)
        .then_with(|| a.chat.cmp(&b.chat))
        .then_with(|| a.codigo.cmp(&b.codigo))
}

/// El trozo de un mensaje que se ensena en su fila, en una linea, y donde
/// resaltar lo encontrado (bytes de `texto`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fragmento {
    pub texto: String,
    pub tramos: Vec<Tramo>,
}

/// Los puntos que dicen «aqui hay mas».
const PUNTOS: &str = "…";

/// **El trozo de `texto` que se ensena** con lo encontrado: en una linea,
/// empezando como mucho `antes` caracteres antes de la primera
/// coincidencia (por el principio de una palabra, con «…» delante si se
/// corta) y de `largo` caracteres como mucho (con «…» detras si se corta).
/// Sin coincidencia, el principio del texto.
pub fn fragmento(texto: &str, aguja: &str, antes: usize, largo: usize) -> Fragmento {
    let linea: String = texto
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let tramos = resaltado::coincidencias(&linea, aguja);
    // Donde empieza el trozo, en bytes de `linea`.
    let mut desde = 0;
    if let Some(primero) = tramos.first() {
        let previas: Vec<(usize, char)> = linea[..primero.desde].char_indices().collect();
        if previas.len() > antes {
            let corte = previas.len() - antes;
            // Hacia delante hasta el principio de una palabra, sin pasar de
            // la coincidencia; si no hay espacio, por donde caiga.
            desde = previas[corte..]
                .iter()
                .position(|(_, c)| *c == ' ')
                .map(|k| previas[corte + k].0 + 1)
                .unwrap_or(previas[corte].0);
        }
    }
    let resto = &linea[desde..];
    let hasta = resto
        .char_indices()
        .nth(largo)
        .map_or(linea.len(), |(k, _)| desde + k);
    let delante = if desde > 0 { PUNTOS } else { "" };
    let detras = if hasta < linea.len() { PUNTOS } else { "" };
    let cuerpo = &linea[desde..hasta];
    let corrido = delante.len();
    let tramos = tramos
        .into_iter()
        .filter(|t| t.desde >= desde && t.desde < hasta)
        .map(|t| Tramo {
            desde: t.desde - desde + corrido,
            hasta: t.hasta.min(hasta) - desde + corrido,
        })
        .collect();
    Fragmento {
        texto: format!("{delante}{cuerpo}{detras}"),
        tramos,
    }
}

// --- Esperar a que se deje de escribir -----------------------------------

/// Lo que se espera desde la ultima tecla antes de buscar en todos los
/// chats: escribiendo seguido no se busca en cada letra.
pub const ESPERA_MS: u64 = 180;

/// **La espera entre tecla y tecla** (el «debounce»): cada cambio la
/// rearma, y toca buscar cuando pasa `ESPERA_MS` sin cambios.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Retardo {
    /// Cuando fue el ultimo cambio sin buscar, en ms de cualquier reloj.
    desde: Option<u64>,
}

impl Retardo {
    /// Lo escrito cambio a los `ahora` ms.
    pub fn cambio(&mut self, ahora: u64) {
        self.desde = Some(ahora);
    }

    /// Si toca buscar ya. Si toca, se desarma: una vez por cambio.
    pub fn toca(&mut self, ahora: u64) -> bool {
        match self.desde {
            Some(d) if ahora.saturating_sub(d) >= ESPERA_MS => {
                self.desde = None;
                true
            }
            _ => false,
        }
    }

    /// Cuanto falta para que toque, si hay algo esperando.
    pub fn ms_hasta(&self, ahora: u64) -> Option<u32> {
        self.desde
            .map(|d| ESPERA_MS.saturating_sub(ahora.saturating_sub(d)) as u32)
    }
}

// --- La seccion en la lista ----------------------------------------------

/// La cabecera «Mensajes» de la seccion, mas baja que una fila (como la de
/// Telegram).
pub const CABECERA_SECCION: u32 = 30;
/// La fila de «Ver más».
pub const VER_MAS: u32 = 40;
/// Cuantos mensajes se ensenan de primeras, y cuantos mas con cada «Ver más».
pub const DE_PRIMERAS: usize = 20;

/// Lo que hay en un punto de la seccion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elemento {
    Cabecera,
    /// El mensaje encontrado `n` (de los que se ensenan).
    Mensaje(usize),
    VerMas,
}

/// **La seccion «Mensajes»** debajo de los chats: su cabecera, una fila de
/// lista (`chat::FILA`) por mensaje y, si quedan, «Ver más». Todo en
/// pixeles desde donde empieza la seccion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Seccion {
    pub mensajes: usize,
    pub ver_mas: bool,
}

impl Seccion {
    /// Si hay seccion: sin mensajes no se pinta ni la cabecera.
    pub fn hay(&self) -> bool {
        self.mensajes > 0
    }

    /// Donde empieza y lo alto que es cada elemento.
    pub fn caja(&self, el: Elemento, escala_por_cien: u32) -> (i32, u32) {
        let e = |v: u32| v * escala_por_cien / 100;
        let fila = e(crate::chat::FILA);
        let cab = e(CABECERA_SECCION) as i32;
        match el {
            Elemento::Cabecera => (0, e(CABECERA_SECCION)),
            Elemento::Mensaje(n) => (cab + n as i32 * fila as i32, fila),
            Elemento::VerMas => (cab + self.mensajes as i32 * fila as i32, e(VER_MAS)),
        }
    }

    /// Lo alto que es entera.
    pub fn alto(&self, escala_por_cien: u32) -> u32 {
        if !self.hay() {
            return 0;
        }
        let ultimo = if self.ver_mas {
            Elemento::VerMas
        } else {
            Elemento::Mensaje(self.mensajes - 1)
        };
        let (y, h) = self.caja(ultimo, escala_por_cien);
        y as u32 + h
    }

    /// Cuantas filas de lista ocupa, redondeando hacia arriba: es lo que se
    /// suma a los chats para saber hasta donde se puede bajar la lista.
    pub fn filas(&self, escala_por_cien: u32) -> usize {
        let fila = (crate::chat::FILA * escala_por_cien / 100).max(1);
        self.alto(escala_por_cien).div_ceil(fila) as usize
    }

    /// Que hay a `y` pixeles del principio de la seccion.
    pub fn elemento_en(&self, y: i32, escala_por_cien: u32) -> Option<Elemento> {
        if !self.hay() || y < 0 || y >= self.alto(escala_por_cien) as i32 {
            return None;
        }
        let (cab_y, cab_h) = self.caja(Elemento::Cabecera, escala_por_cien);
        if y < cab_y + cab_h as i32 {
            return Some(Elemento::Cabecera);
        }
        let (fila_y, fila_h) = self.caja(Elemento::Mensaje(0), escala_por_cien);
        let n = ((y - fila_y) / fila_h.max(1) as i32) as usize;
        Some(if n < self.mensajes {
            Elemento::Mensaje(n)
        } else {
            Elemento::VerMas
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada(chat: &str, codigo: &str, cuando: i64, textos: &[&str]) -> Entrada {
        Entrada::nueva(chat, codigo, cuando, textos.iter().copied())
    }

    #[test]
    fn busca_sin_mirar_mayusculas_ni_tildes_y_en_cualquier_pieza() {
        let es = [
            entrada("a", "1", 30, &["La ARAÑA del tejado"]),
            entrada("b", "2", 20, &["", "nota de voz: cemento listo"]),
            entrada("c", "3", 10, &["", "", "Plano_Cimentación.pdf"]),
            entrada("c", "4", 5, &["nada que ver"]),
        ];
        let h = buscar(&es, "arana", 10);
        assert_eq!(
            h.aciertos,
            vec![Acierto {
                entrada: 0,
                pieza: 0
            }]
        );
        let h = buscar(&es, "CEMENTO", 10);
        assert_eq!(
            h.aciertos,
            vec![Acierto {
                entrada: 1,
                pieza: 0
            }]
        );
        let h = buscar(&es, "cimentacion", 10);
        assert_eq!(
            h.aciertos,
            vec![Acierto {
                entrada: 2,
                pieza: 0
            }]
        );
        assert_eq!(buscar(&es, "   ", 10), Hallado::default());
    }

    #[test]
    fn los_vacios_y_repetidos_no_son_piezas() {
        let e = entrada("a", "1", 0, &["  ", "hola", "hola", "hola.png"]);
        let textos: Vec<&str> = e.piezas.iter().map(|p| p.texto.as_str()).collect();
        assert_eq!(textos, ["hola", "hola.png"]);
    }

    #[test]
    fn el_tope_corta_lo_que_se_devuelve_pero_no_la_cuenta() {
        let es: Vec<Entrada> = (0..50)
            .map(|i| entrada("a", &i.to_string(), 100 - i, &["pan y vino"]))
            .collect();
        let h = buscar(&es, "vino", 20);
        assert_eq!(h.aciertos.len(), 20);
        assert_eq!(h.total, 50);
        assert!(h.hay_mas());
        assert!(!buscar(&es, "vino", 50).hay_mas());
    }

    #[test]
    fn los_blancos_cuentan_como_espacio() {
        let es = [entrada("a", "1", 0, &["hola\nmundo"])];
        assert_eq!(buscar(&es, "hola mundo", 5).total, 1);
        let f = fragmento("hola\nmundo", "hola mundo", 10, 50);
        assert_eq!(f.texto, "hola mundo");
        assert_eq!(
            f.tramos,
            vec![Tramo {
                desde: 0,
                hasta: 10
            }]
        );
    }

    #[test]
    fn se_ordena_de_lo_mas_nuevo_a_lo_mas_viejo() {
        let a = entrada("a", "1", 10, &["x"]);
        let b = entrada("b", "2", 30, &["x"]);
        let c = entrada("a", "3", 20, &["x"]);
        let mut v = vec![&a, &b, &c];
        ordenar(&mut v);
        let codigos: Vec<&str> = v.iter().map(|e| e.codigo.as_str()).collect();
        assert_eq!(codigos, ["2", "3", "1"]);
    }

    #[test]
    fn el_fragmento_empieza_cerca_de_lo_encontrado_por_una_palabra() {
        let texto = "uno dos tres cuatro cinco seis siete ocho nueve diez once";
        let f = fragmento(texto, "nueve", 12, 100);
        assert_eq!(f.texto, "…siete ocho nueve diez once");
        let t = f.tramos[0];
        assert_eq!(&f.texto[t.desde..t.hasta], "nueve");
    }

    #[test]
    fn el_fragmento_corto_no_lleva_puntos_y_resalta_todas() {
        let f = fragmento("  Casa y casa  ", "casa", 20, 100);
        assert_eq!(f.texto, "Casa y casa");
        let recortes: Vec<&str> = f
            .tramos
            .iter()
            .map(|t| &f.texto[t.desde..t.hasta])
            .collect();
        assert_eq!(recortes, ["Casa", "casa"]);
    }

    #[test]
    fn el_fragmento_largo_se_corta_con_puntos_y_respeta_las_tildes() {
        let f = fragmento("Árbol ñandú camión árbol más texto aquí", "arbol", 5, 12);
        assert_eq!(f.texto, "Árbol ñandú …");
        let t = f.tramos[0];
        assert_eq!(&f.texto[t.desde..t.hasta], "Árbol");
        // La segunda coincidencia queda fuera del trozo: no se resalta.
        assert_eq!(f.tramos.len(), 1);
    }

    #[test]
    fn el_fragmento_sin_coincidencia_es_el_principio() {
        let f = fragmento("abc def", "zzz", 3, 3);
        assert_eq!(f.texto, "abc…");
        assert!(f.tramos.is_empty());
    }

    #[test]
    fn la_espera_busca_una_vez_cuando_se_deja_de_escribir() {
        let mut r = Retardo::default();
        assert!(!r.toca(0));
        assert_eq!(r.ms_hasta(0), None);
        r.cambio(1000);
        assert_eq!(r.ms_hasta(1000), Some(ESPERA_MS as u32));
        assert!(!r.toca(1100));
        r.cambio(1100);
        assert!(!r.toca(1100 + ESPERA_MS - 1));
        assert!(r.toca(1100 + ESPERA_MS));
        assert!(!r.toca(5000), "una vez por cambio");
    }

    #[test]
    fn la_seccion_reparte_cabecera_mensajes_y_ver_mas() {
        let s = Seccion {
            mensajes: 2,
            ver_mas: true,
        };
        let fila = crate::chat::FILA as i32;
        let cab = CABECERA_SECCION as i32;
        assert_eq!(s.alto(100), (cab + 2 * fila) as u32 + VER_MAS);
        assert_eq!(s.elemento_en(0, 100), Some(Elemento::Cabecera));
        assert_eq!(s.elemento_en(cab, 100), Some(Elemento::Mensaje(0)));
        assert_eq!(s.elemento_en(cab + fila, 100), Some(Elemento::Mensaje(1)));
        assert_eq!(s.elemento_en(cab + 2 * fila, 100), Some(Elemento::VerMas));
        assert_eq!(s.elemento_en(s.alto(100) as i32, 100), None);
        assert_eq!(s.elemento_en(-1, 100), None);
        // 30 + 2 x 62 + 40 = 194 px: algo mas de tres filas, cuatro.
        assert_eq!(s.filas(100), 4);
        // A doble escala, todo el doble.
        assert_eq!(s.elemento_en(2 * cab, 200), Some(Elemento::Mensaje(0)));
        // Sin mensajes no hay seccion.
        let vacia = Seccion::default();
        assert_eq!((vacia.alto(100), vacia.filas(100)), (0, 0));
        assert_eq!(vacia.elemento_en(0, 100), None);
    }
}
