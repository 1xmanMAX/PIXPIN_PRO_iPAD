//! **La foto del chat, lo que no se pinta** (25-oct-2026, el usuario:
//! «permitir poner un fondo en las esferas de los chats para que parezca mas
//! una app de chat: dame la opcion de poner emojis como su perfil, o poner
//! una imagen yo»).
//!
//! Tres piezas del editor de la foto, sin Win32 ni Direct2D:
//!
//! - [`FONDOS`]: los doce degradados del circulo, los de Telegram y unos
//!   pocos mas. Se guardan como `#RRGGBB` (`pixpin_proyecto::avatar`), asi
//!   que otro aparato los pinta aunque no tenga esta lista.
//! - [`EMOJIS`] y [`buscar`]: una rejilla de emojis de uso comun con sus
//!   nombres en castellano e ingles, para encontrarlos escribiendo.
//! - [`Recorte`]: que cuadrado de una foto entra en el circulo. Al abrirla,
//!   el del centro; la rueda acerca hacia el raton y arrastrar la lleva, sin
//!   que el cuadrado se salga nunca de la foto.

/// Un fondo del circulo: un degradado de arriba (`arriba`) abajo (`abajo`),
/// en `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fondo {
    pub arriba: u32,
    pub abajo: u32,
}

/// Los fondos que se ofrecen, en el orden de la paleta. Los siete primeros
/// son los degradados de los avatares de Telegram (rojo, naranja, violeta,
/// verde, turquesa, azul y rosa); detras, amarillo, marron, indigo, gris y
/// grafito.
pub const FONDOS: [Fondo; 12] = [
    Fondo {
        arriba: 0xFF885E,
        abajo: 0xFF516A,
    },
    Fondo {
        arriba: 0xFFCD6A,
        abajo: 0xFFA85C,
    },
    Fondo {
        arriba: 0x82B1FF,
        abajo: 0x665FFF,
    },
    Fondo {
        arriba: 0xA0DE7E,
        abajo: 0x54CB68,
    },
    Fondo {
        arriba: 0x53EDD6,
        abajo: 0x28C9B7,
    },
    Fondo {
        arriba: 0x72D5FD,
        abajo: 0x2A9EF1,
    },
    Fondo {
        arriba: 0xE0A2F3,
        abajo: 0xD669ED,
    },
    Fondo {
        arriba: 0xFFE27A,
        abajo: 0xF5C33B,
    },
    Fondo {
        arriba: 0xD9A77A,
        abajo: 0xB07B4F,
    },
    Fondo {
        arriba: 0x9FA8DA,
        abajo: 0x5C6BC0,
    },
    Fondo {
        arriba: 0xB8C2CC,
        abajo: 0x8A96A3,
    },
    Fondo {
        arriba: 0x5A6270,
        abajo: 0x2F3540,
    },
];

/// El fondo que se propone al abrir el editor sin ninguno elegido: uno fijo
/// por chat, que sale de su codigo (FNV-1a, estable entre aparatos).
pub fn fondo_propuesto(codigo: &str) -> usize {
    let mut h: u32 = 0x811c_9dc5;
    for b in codigo.trim().as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h as usize % FONDOS.len()
}

/// En que lugar de la paleta esta un degradado, si es uno de ella.
pub fn lugar_del_fondo(arriba: u32, abajo: u32) -> Option<usize> {
    FONDOS
        .iter()
        .position(|f| f.arriba == arriba && f.abajo == abajo)
}

// --- Los emojis ---------------------------------------------------------------

/// Un emoji de la rejilla y las palabras con que se encuentra (castellano e
/// ingles, en minusculas y sin tildes).
pub type Emoji = (&'static str, &'static str);

/// Los de la rejilla, por temas: caras, trabajo y estudio, casa y obra,
/// naturaleza, comida, viajes, deporte y ocio, y simbolos.
pub const EMOJIS: &[Emoji] = &[
    ("😀", "cara sonrisa feliz alegre smile happy face"),
    ("😎", "gafas sol guay cool sunglasses"),
    ("🤓", "empollon estudio gafas nerd"),
    ("🥳", "fiesta celebrar party celebrate"),
    ("😍", "amor enamorado love heart eyes"),
    ("🤔", "pensar duda think thinking"),
    ("😴", "dormir sueno sleep"),
    ("🤖", "robot bot ia ai"),
    ("👻", "fantasma ghost"),
    ("👽", "alien extraterrestre"),
    ("🐱", "gato cat"),
    ("🐶", "perro dog"),
    ("💼", "trabajo maletin oficina work briefcase office"),
    ("📚", "libros estudio clase books study school"),
    (
        "🎓",
        "graduacion universidad tesis graduation university thesis",
    ),
    ("✏️", "lapiz escribir apuntes pencil write notes"),
    ("📝", "nota apuntes memo notes"),
    ("📐", "regla escuadra dibujo tecnico ruler drawing"),
    ("📊", "grafico datos estadistica chart data"),
    ("💡", "idea bombilla light bulb"),
    ("🔬", "microscopio ciencia laboratorio science lab"),
    ("🧪", "quimica experimento chemistry test"),
    ("💻", "ordenador portatil codigo computer laptop code"),
    ("📱", "movil telefono phone mobile"),
    ("📅", "calendario fecha agenda calendar"),
    ("📌", "chincheta fijar pin"),
    ("🗂️", "carpetas archivo fichero folders files"),
    ("💰", "dinero cuentas money"),
    ("🏠", "casa hogar home house"),
    ("🏡", "casa jardin chalet garden house"),
    ("🏢", "edificio oficina building office"),
    ("🏗️", "obra construccion grua construction building site"),
    ("🧱", "ladrillo muro obra brick wall"),
    ("🔨", "martillo herramienta hammer tool"),
    ("🔧", "llave herramienta arreglar wrench fix"),
    ("🪚", "sierra carpinteria saw"),
    ("📏", "regla medir metro ruler measure"),
    ("🛋️", "sofa salon muebles couch furniture"),
    ("🛏️", "cama dormitorio bed bedroom"),
    ("🚿", "ducha bano shower bathroom"),
    ("🍳", "cocina sarten kitchen cooking"),
    ("🔑", "llave key"),
    ("🌳", "arbol naturaleza tree nature"),
    ("🌻", "girasol flor sunflower flower"),
    ("🌸", "flor cerezo blossom flower"),
    ("🌵", "cactus desierto desert"),
    ("🍀", "trebol suerte clover luck"),
    ("🌊", "ola mar playa wave sea beach"),
    ("⛰️", "montana mountain"),
    ("🌙", "luna noche moon night"),
    ("☀️", "sol sun"),
    ("⭐", "estrella star"),
    ("🔥", "fuego fire hot"),
    ("❄️", "nieve invierno snow winter"),
    ("🌈", "arcoiris rainbow"),
    ("🌍", "mundo tierra planeta world earth globe"),
    ("🍕", "pizza comida food"),
    ("🍔", "hamburguesa burger food"),
    ("🍎", "manzana fruta apple fruit"),
    ("🥑", "aguacate avocado"),
    ("☕", "cafe coffee"),
    ("🍷", "vino wine"),
    ("🍰", "tarta pastel cake"),
    ("🥗", "ensalada dieta salad"),
    ("✈️", "avion viaje plane travel"),
    ("🚗", "coche car"),
    ("🚲", "bici bicicleta bike"),
    ("🚀", "cohete lanzamiento proyecto rocket launch"),
    ("🏖️", "playa vacaciones beach holiday"),
    ("🗺️", "mapa viaje map travel"),
    ("🧳", "maleta equipaje luggage"),
    ("⚽", "futbol balon soccer football"),
    ("🏀", "baloncesto basket basketball"),
    ("🎾", "tenis tennis"),
    ("🏋️", "gimnasio pesas gym"),
    ("🧘", "yoga meditar relax"),
    ("🎮", "videojuego juego game"),
    ("🎵", "musica nota music"),
    ("🎸", "guitarra guitar"),
    ("🎨", "arte pintura paleta art paint"),
    ("📷", "camara foto camera photo"),
    ("🎬", "cine pelicula video movie film"),
    ("🎁", "regalo gift present"),
    ("🎉", "fiesta confeti party"),
    ("❤️", "corazon rojo amor heart love red"),
    ("💙", "corazon azul heart blue"),
    ("💚", "corazon verde heart green"),
    ("💛", "corazon amarillo heart yellow"),
    ("💜", "corazon morado heart purple"),
    ("✅", "hecho listo check done"),
    ("⚠️", "aviso cuidado warning"),
    ("❓", "pregunta duda question"),
    ("💬", "chat conversacion mensaje message"),
    ("🔔", "campana aviso bell notification"),
    ("🔒", "candado privado lock private"),
    ("⚡", "rayo energia lightning energy"),
    ("💎", "diamante joya diamond gem"),
    ("👑", "corona rey crown king"),
    ("🏆", "trofeo premio trophy"),
    ("🎯", "diana objetivo target goal"),
];

/// Minusculas y sin tildes, para buscar sin pensar en acentos.
pub fn normalizar(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            otra => otra,
        })
        .collect()
}

/// **Los emojis que encuentra `consulta`**, por su lugar en [`EMOJIS`]. Sin
/// nada escrito, todos. Cada palabra escrita tiene que empezar alguna de sus
/// palabras («cas» encuentra la casa; «casa jar», la del jardin); un emoji
/// pegado se encuentra a si mismo.
pub fn buscar(consulta: &str) -> Vec<usize> {
    let q = normalizar(consulta);
    if q.is_empty() {
        return (0..EMOJIS.len()).collect();
    }
    let trozos: Vec<&str> = q.split_whitespace().collect();
    EMOJIS
        .iter()
        .enumerate()
        .filter(|(_, (e, nombres))| {
            *e == consulta.trim()
                || trozos
                    .iter()
                    .all(|t| nombres.split(' ').any(|n| n.starts_with(t)))
        })
        .map(|(i, _)| i)
        .collect()
}

// --- El recorte -------------------------------------------------------------

/// Lo mas que se acerca, veces «el cuadrado mas grande».
pub const AUMENTO_MAXIMO: f32 = 8.0;
/// Lo que acerca un paso de rueda (120).
pub const PASO: f32 = 1.2;

/// **Que cuadrado de una foto `w x h` entra en el circulo**: lo acercado
/// (1 = el cuadrado mas grande que cabe, el lado corto entero) y su centro
/// en pixeles de la foto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recorte {
    pub aumento: f32,
    pub cx: f32,
    pub cy: f32,
}

impl Recorte {
    /// El cuadrado mas grande, en el centro: como se abre.
    pub fn centrado(w: u32, h: u32) -> Recorte {
        Recorte {
            aumento: 1.0,
            cx: w as f32 / 2.0,
            cy: h as f32 / 2.0,
        }
    }

    /// El lado del cuadrado, en pixeles de la foto.
    pub fn lado(&self, w: u32, h: u32) -> f32 {
        w.min(h) as f32 / self.aumento.clamp(1.0, AUMENTO_MAXIMO)
    }

    /// Lo deja dentro de la foto: el aumento entre 1 y el maximo y el
    /// cuadrado sin salirse por ningun lado.
    pub fn acotar(&mut self, w: u32, h: u32) {
        self.aumento = self.aumento.clamp(1.0, AUMENTO_MAXIMO);
        let mitad = self.lado(w, h) / 2.0;
        self.cx = self.cx.clamp(mitad, (w as f32 - mitad).max(mitad));
        self.cy = self.cy.clamp(mitad, (h as f32 - mitad).max(mitad));
    }

    /// El cuadrado `(x, y, lado)` en pixeles enteros de la foto, el que se
    /// recorta al guardar. Siempre dentro de ella y de un pixel o mas.
    pub fn en_pixeles(&self, w: u32, h: u32) -> (u32, u32, u32) {
        let mut r = *self;
        r.acotar(w, h);
        let lado = (r.lado(w, h).round() as u32).clamp(1, w.min(h).max(1));
        let x = ((r.cx - lado as f32 / 2.0).round().max(0.0) as u32).min(w.saturating_sub(lado));
        let y = ((r.cy - lado as f32 / 2.0).round().max(0.0) as u32).min(h.saturating_sub(lado));
        (x, y, lado)
    }

    /// Donde se pinta la foto ENTERA para que su cuadrado caiga en `caja`
    /// (`x, y, lado` de la ventana): `(x, y, ancho, alto)`. Quien pinta la
    /// recorta al circulo.
    pub fn destino(&self, caja: (f32, f32, f32), w: u32, h: u32) -> (f32, f32, f32, f32) {
        let k = caja.2 / self.lado(w, h);
        (
            caja.0 + caja.2 / 2.0 - self.cx * k,
            caja.1 + caja.2 / 2.0 - self.cy * k,
            w as f32 * k,
            h as f32 * k,
        )
    }

    /// Arrastrar la foto `(dx, dy)` pixeles de la ventana en una caja de
    /// `lado_caja`: la foto va con el raton, asi que el centro va al reves.
    pub fn mover(&mut self, w: u32, h: u32, lado_caja: f32, dx: f32, dy: f32) {
        if lado_caja <= 0.0 {
            return;
        }
        let k = self.lado(w, h) / lado_caja;
        self.cx -= dx * k;
        self.cy -= dy * k;
        self.acotar(w, h);
    }

    /// Acerca (`factor` > 1) o aleja hacia `punto`, en fraccion de la caja
    /// (0..1 en cada eje): lo que hay bajo el raton se queda bajo el.
    pub fn acercar(&mut self, w: u32, h: u32, factor: f32, punto: (f32, f32)) {
        let antes = self.lado(w, h);
        // El punto de la foto bajo el raton.
        let px = self.cx + (punto.0 - 0.5) * antes;
        let py = self.cy + (punto.1 - 0.5) * antes;
        self.aumento = (self.aumento * factor).clamp(1.0, AUMENTO_MAXIMO);
        let despues = self.lado(w, h);
        self.cx = px - (punto.0 - 0.5) * despues;
        self.cy = py - (punto.1 - 0.5) * despues;
        self.acotar(w, h);
    }

    /// Un giro de rueda (`delta` en unidades de 120, positivo acerca).
    pub fn rueda(&mut self, w: u32, h: u32, delta: i32, punto: (f32, f32)) {
        let pasos = delta as f32 / 120.0;
        self.acercar(w, h, PASO.powf(pasos), punto);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_paleta_tiene_doce_fondos_distintos_y_cada_chat_su_propuesto() {
        for (i, a) in FONDOS.iter().enumerate() {
            assert!(FONDOS[i + 1..].iter().all(|b| b != a), "{i} repetido");
            assert_eq!(lugar_del_fondo(a.arriba, a.abajo), Some(i));
        }
        assert_eq!(FONDOS.len(), 12);
        assert!(fondo_propuesto("AGJ36629KT") < FONDOS.len());
        assert_eq!(
            fondo_propuesto("AGJ36629KT"),
            fondo_propuesto(" AGJ36629KT ")
        );
        // Caso negativo: un degradado que no es de la paleta no tiene lugar.
        assert_eq!(lugar_del_fondo(0x123456, 0x654321), None);
    }

    #[test]
    fn buscar_encuentra_por_el_principio_de_las_palabras_sin_tildes() {
        let de = |q: &str| -> Vec<&str> { buscar(q).iter().map(|i| EMOJIS[*i].0).collect() };
        assert_eq!(buscar("").len(), EMOJIS.len());
        assert!(de("casa").contains(&"🏠"));
        assert!(de("Construcción").contains(&"🏗️"));
        assert!(de("house").contains(&"🏠"));
        // Dos palabras: tienen que estar las dos.
        assert_eq!(de("casa jardin"), vec!["🏡"]);
        // Un emoji pegado se encuentra a si mismo.
        assert_eq!(de("🍕"), vec!["🍕"]);
        // Caso negativo: lo que no esta no encuentra nada, y una letra en
        // medio de una palabra no cuenta.
        assert!(buscar("zzzz").is_empty());
        assert!(!de("asa").contains(&"🏠"));
    }

    #[test]
    fn los_emojis_no_se_repiten_y_todos_tienen_nombre() {
        for (i, (e, n)) in EMOJIS.iter().enumerate() {
            assert!(!n.trim().is_empty(), "{e} sin nombre");
            assert_eq!(normalizar(n), *n, "{e}: nombres sin normalizar");
            assert!(EMOJIS[i + 1..].iter().all(|(o, _)| o != e), "{e} repetido");
        }
    }

    #[test]
    fn al_abrir_es_el_cuadrado_mas_grande_del_centro() {
        assert_eq!(
            Recorte::centrado(400, 300).en_pixeles(400, 300),
            (50, 0, 300)
        );
        assert_eq!(
            Recorte::centrado(300, 400).en_pixeles(300, 400),
            (0, 50, 300)
        );
        assert_eq!(Recorte::centrado(10, 10).en_pixeles(10, 10), (0, 0, 10));
    }

    #[test]
    fn mover_no_saca_el_cuadrado_de_la_foto() {
        let (w, h) = (400, 300);
        let mut r = Recorte::centrado(w, h);
        // A la izquierda del todo: la foto va a la derecha.
        r.mover(w, h, 150.0, 1000.0, 0.0);
        assert_eq!(r.en_pixeles(w, h), (0, 0, 300));
        r.mover(w, h, 150.0, -1000.0, -1000.0);
        assert_eq!(r.en_pixeles(w, h), (100, 0, 300));
        // En el eje en que no sobra nada no se mueve.
        assert_eq!(r.cy, 150.0);
    }

    #[test]
    fn acercar_deja_quieto_lo_que_hay_bajo_el_raton() {
        let (w, h) = (1000, 1000);
        let mut r = Recorte::centrado(w, h);
        // En el centro: el cuadrado se encoge alrededor del centro.
        r.acercar(w, h, 2.0, (0.5, 0.5));
        assert_eq!(r.en_pixeles(w, h), (250, 250, 500));
        // Hacia la esquina de arriba a la izquierda: esa esquina se queda.
        let mut r = Recorte::centrado(w, h);
        r.acercar(w, h, 4.0, (0.0, 0.0));
        assert_eq!(r.en_pixeles(w, h), (0, 0, 250));
        // Caso negativo: alejar de mas no da un cuadrado mayor que la foto,
        // ni acercar de mas uno mas pequeno que el tope.
        r.rueda(w, h, -120 * 50, (0.5, 0.5));
        assert_eq!(r.en_pixeles(w, h), (0, 0, 1000));
        r.rueda(w, h, 120 * 50, (0.5, 0.5));
        assert_eq!(r.aumento, AUMENTO_MAXIMO);
        assert_eq!(r.en_pixeles(w, h).2, 125);
    }

    #[test]
    fn el_destino_pone_el_cuadrado_en_la_caja() {
        let (w, h) = (400, 300);
        let r = Recorte::centrado(w, h);
        // El cuadrado (50,0,300) en una caja de 150 en (10,20): la foto se
        // pinta a la mitad y movida 25 a la izquierda.
        let (x, y, dw, dh) = r.destino((10.0, 20.0, 150.0), w, h);
        assert_eq!((x, y, dw, dh), (-15.0, 20.0, 200.0, 150.0));
    }
}
