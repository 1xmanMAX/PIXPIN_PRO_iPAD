//! **La nota rica del chat**: una nota es un Markdown pequeno, y su burbuja
//! lo ensena entero —texto con formato e imagenes en su sitio—, como un
//! mensaje de Telegram con varias fotos y su texto (el usuario, 25-oct-2026:
//! «que la burbuja muestre todo completo, como si fuera un pequeno archivo
//! markdown»).
//!
//! Logica pura: aqui se parte el texto en bloques y se reparte el album de
//! fotos; quien pinta (el chat de Windows) mide los renglones con su fuente.
//! El formato es el de siempre de las notas, y el movil ya lo lee igual:
//! una `NOTA` cuyo `texto` es Markdown, con las fotos como
//! `![alt](pixpin:files/…)` (ver `docs/nota-rica-del-chat.md`).
//!
//! Decisiones que no son evidentes:
//!
//! - **Un salto de linea es un salto de linea.** En un chat nadie escribe
//!   pensando en el Markdown de los parrafos: dos renglones seguidos se
//!   ensenan en dos renglones, no unidos como haria un lector de `.md`.
//! - **Las fotos seguidas son un album.** Dos o mas `![…](…)` en renglones
//!   seguidos (o en el mismo) se reparten en filas de dos o tres, como el
//!   album de Telegram; una linea en blanco entre ellas las separa.
//! - **Nada del Markdown se ve crudo**: ni los `**`, ni las rutas de las
//!   fotos. Lo que no se entiende se ensena como texto, que es mas honrado
//!   que esconderlo.
//! - **Las posiciones van en UTF-16**, que es lo que cuenta quien pinta
//!   (DirectWrite), igual que en `resaltado`.

/// Un trozo del texto de un bloque con estilo propio, en unidades UTF-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Marca {
    pub inicio: u32,
    pub largo: u32,
    pub negrita: bool,
    pub cursiva: bool,
    pub codigo: bool,
    pub tachado: bool,
}

/// Un enlace del texto: donde esta (UTF-16) y adonde lleva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enlace {
    pub inicio: u32,
    pub largo: u32,
    pub url: String,
}

/// El texto de un bloque ya sin las marcas del Markdown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Linea {
    pub texto: String,
    pub marcas: Vec<Marca>,
    pub enlaces: Vec<Enlace>,
}

/// Que es un bloque de texto: decide su letra, su sangria y su marca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clase {
    Parrafo,
    /// `#` a `######`.
    Titulo(u8),
    /// `- x`, con su nivel de sangria (cero el de fuera).
    Vineta(u8),
    /// `1. x`, con su numero tal cual se escribio.
    Numero(u8, String),
    /// `- [ ] x` / `- [x] x`.
    Casilla(u8, bool),
    /// `> x`.
    Cita,
}

/// Una foto de la nota: lo que dice de ella y como se llega a su fichero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imagen {
    pub alt: String,
    pub enlace: String,
}

/// Un bloque de la nota, de arriba abajo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bloque {
    Texto(Clase, Linea),
    /// Un bloque de codigo (o una tabla, que en monoespaciada se lee).
    Codigo(String),
    /// `---`.
    Regla,
    /// Una o varias fotos seguidas: un album.
    Imagenes(Vec<Imagen>),
    /// Un fichero de la nota que no es una foto (un PDF, un audio): su
    /// nombre, que es lo que se reconoce, y su enlace.
    Adjunto(Imagen),
}

/// Si el enlace de un `![…](…)` es una foto, por su extension. Lo demas
/// (audio, video, documentos) lo nombra el movil igual (`claseDeMedio`).
pub fn es_foto(enlace: &str) -> bool {
    let sin_resto = enlace.split(['?', '#']).next().unwrap_or_default();
    let ext = sin_resto
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "heic" | "heif" | "tif" | "tiff"
    )
}

/// Las fotos y adjuntos `![alt](enlace)` de un renglon, con lo que queda
/// entre ellos: `Err` con cada trozo de texto y `Ok` con cada imagen. Lo
/// que va dentro de un `` `codigo` `` es texto aunque lo parezca (quien
/// explica la sintaxis escribe `` `![](foto.png)` ``).
fn partir_imagenes(linea: &str) -> Vec<Result<Imagen, &str>> {
    let codigo = tramos_de_codigo(linea);
    let mut salida = Vec::new();
    // Donde empieza el texto aun sin soltar, y desde donde se busca.
    let (mut desde, mut buscar) = (0, 0);
    while let Some(k) = linea[buscar..].find("![") {
        let i = buscar + k;
        let imagen = if codigo.iter().any(|r| r.contains(&i)) {
            None
        } else {
            imagen_en(&linea[i..])
        };
        let Some((imagen, largo)) = imagen else {
            // Un `![` que no cierra, o de un codigo: texto, y se sigue
            // buscando detras.
            buscar = i + 2;
            continue;
        };
        if i > desde {
            salida.push(Err(&linea[desde..i]));
        }
        salida.push(Ok(imagen));
        desde = i + largo;
        buscar = desde;
    }
    if desde < linea.len() {
        salida.push(Err(&linea[desde..]));
    }
    salida
}

/// Donde hay `` `codigo` `` en un renglon (en bytes, con sus comillas),
/// cerrado como lo cierra [`en_linea`].
fn tramos_de_codigo(linea: &str) -> Vec<std::ops::Range<usize>> {
    let mut tramos = Vec::new();
    let mut i = 0;
    while let Some(k) = linea[i..].find('`') {
        let inicio = i + k;
        let tiras = linea[inicio..].bytes().take_while(|b| *b == b'`').count();
        let valla = &linea[inicio..inicio + tiras];
        match linea[inicio + tiras..].find(valla) {
            Some(j) => {
                let fin = inicio + tiras + j + tiras;
                tramos.push(inicio..fin);
                i = fin;
            }
            None => i = inicio + tiras,
        }
    }
    tramos
}

/// Donde cierra el `(` de un enlace, en `tras` (lo de detras del `(`): el
/// `)` que lo empareja, contando los que se abren dentro, porque una ruta
/// de Windows (`C:\Fotos (2)\a.png`) o una direccion de la Wikipedia los
/// llevan. Entre `<…>` (`![](<ruta con ) suelto>)`), el de despues del `>`.
/// Si no se emparejan, el primero, como antes.
fn cierre_del_parentesis(tras: &str) -> Option<usize> {
    if tras.trim_start().starts_with('<')
        && let Some(g) = tras.find('>')
        && let Some(k) = tras[g..].find(')')
    {
        return Some(g + k);
    }
    let mut nivel = 0usize;
    for (k, c) in tras.char_indices() {
        match c {
            '(' => nivel += 1,
            ')' if nivel == 0 => return Some(k),
            ')' => nivel -= 1,
            '\n' => break,
            _ => {}
        }
    }
    tras.find(')')
}

/// Una `![alt](enlace)` al principio de `s`, y lo que ocupa en bytes.
/// El `|300` que el editor pone detras del alt (el ancho) no es del alt.
fn imagen_en(s: &str) -> Option<(Imagen, usize)> {
    let s2 = s.strip_prefix("![")?;
    let cierre = s2.find("](")?;
    let alt = &s2[..cierre];
    if alt.contains('\n') {
        return None;
    }
    let tras = &s2[cierre + 2..];
    let fin = cierre_del_parentesis(tras)?;
    let enlace = tras[..fin].trim();
    // Un titulo detras de la ruta (`![a](ruta "titulo")`) no es la ruta.
    let enlace = enlace.split(" \"").next().unwrap_or(enlace).trim();
    let enlace = enlace.trim_start_matches('<').trim_end_matches('>');
    if enlace.is_empty() {
        return None;
    }
    let alt = alt.split('|').next().unwrap_or_default().trim().to_string();
    Some((
        Imagen {
            alt,
            enlace: enlace.to_string(),
        },
        2 + cierre + 2 + fin + 1,
    ))
}

/// Cuantos espacios lleva delante un renglon (un tabulador cuenta cuatro).
fn sangria(linea: &str) -> (usize, &str) {
    let mut n = 0;
    for (i, c) in linea.char_indices() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => return (n, &linea[i..]),
        }
    }
    (n, "")
}

/// Si el renglon es una raya (`---`, `***`, `___`, con espacios).
fn es_regla(t: &str) -> bool {
    let sin: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    sin.len() >= 3
        && (sin.chars().all(|c| c == '-')
            || sin.chars().all(|c| c == '*')
            || sin.chars().all(|c| c == '_'))
}

/// Lo que marca un renglon de lista, si lo es: su clase y el texto de
/// despues.
fn lista(linea: &str) -> Option<(Clase, &str)> {
    let (espacios, t) = sangria(linea);
    let nivel = (espacios / 2).min(6) as u8;
    let mut c = t.chars();
    let primera = c.next()?;
    if matches!(primera, '-' | '*' | '+') {
        let tras = t[1..].strip_prefix(' ')?;
        for (marca, hecha) in [("[ ] ", false), ("[x] ", true), ("[X] ", true)] {
            if let Some(r) = tras.strip_prefix(marca) {
                return Some((Clase::Casilla(nivel, hecha), r));
            }
        }
        // Una casilla vacia al final del renglon («- [ ]») tambien lo es.
        if tras == "[ ]" || tras == "[x]" || tras == "[X]" {
            return Some((Clase::Casilla(nivel, tras != "[ ]"), ""));
        }
        return Some((Clase::Vineta(nivel), tras));
    }
    let cifras = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=9).contains(&cifras) {
        let tras = &t[cifras..];
        let sep = tras.chars().next()?;
        if matches!(sep, '.' | ')')
            && let Some(r) = tras[1..].strip_prefix(' ')
        {
            return Some((Clase::Numero(nivel, format!("{}{sep}", &t[..cifras])), r));
        }
    }
    None
}

/// Un titulo `# x`, si lo es.
fn titulo(t: &str) -> Option<(u8, &str)> {
    let almohadillas = t.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&almohadillas) {
        return None;
    }
    let tras = &t[almohadillas..];
    if tras.is_empty() {
        return Some((almohadillas as u8, ""));
    }
    let tras = tras.strip_prefix(' ')?;
    // Las almohadillas de cierre (`# Titulo #`) no son del titulo.
    Some((
        almohadillas as u8,
        tras.trim_end().trim_end_matches('#').trim_end(),
    ))
}

/// Parte una nota en bloques.
pub fn bloques(texto: &str) -> Vec<Bloque> {
    let mut salida: Vec<Bloque> = Vec::new();
    // El parrafo que se va juntando, renglon a renglon.
    let mut parrafo: Vec<&str> = Vec::new();
    let mut cita: Vec<&str> = Vec::new();
    // Las fotos que se van juntando en un album.
    let mut album: Vec<Imagen> = Vec::new();
    let renglones: Vec<&str> = texto.lines().collect();

    fn cerrar_parrafo(parrafo: &mut Vec<&str>, salida: &mut Vec<Bloque>) {
        if !parrafo.is_empty() {
            salida.push(Bloque::Texto(Clase::Parrafo, en_linea(&parrafo.join("\n"))));
            parrafo.clear();
        }
    }
    fn cerrar_cita(cita: &mut Vec<&str>, salida: &mut Vec<Bloque>) {
        if !cita.is_empty() {
            salida.push(Bloque::Texto(Clase::Cita, en_linea(&cita.join("\n"))));
            cita.clear();
        }
    }
    fn cerrar_album(album: &mut Vec<Imagen>, salida: &mut Vec<Bloque>) {
        if !album.is_empty() {
            salida.push(Bloque::Imagenes(std::mem::take(album)));
        }
    }

    let mut i = 0;
    while i < renglones.len() {
        let linea = renglones[i];
        let (espacios, t) = sangria(linea);
        let t = t.trim_end();
        i += 1;
        // Un bloque de codigo, hasta su cierre (o hasta el final).
        if espacios < 4 && (t.starts_with("```") || t.starts_with("~~~")) {
            cerrar_parrafo(&mut parrafo, &mut salida);
            cerrar_cita(&mut cita, &mut salida);
            cerrar_album(&mut album, &mut salida);
            let valla = &t[..3];
            let mut dentro = Vec::new();
            while i < renglones.len() && !renglones[i].trim_start().starts_with(valla) {
                dentro.push(renglones[i]);
                i += 1;
            }
            i += 1;
            salida.push(Bloque::Codigo(dentro.join("\n")));
            continue;
        }
        if t.is_empty() {
            cerrar_parrafo(&mut parrafo, &mut salida);
            cerrar_cita(&mut cita, &mut salida);
            cerrar_album(&mut album, &mut salida);
            continue;
        }
        if let Some(r) = t.strip_prefix('>') {
            cerrar_parrafo(&mut parrafo, &mut salida);
            cerrar_album(&mut album, &mut salida);
            cita.push(r.strip_prefix(' ').unwrap_or(r));
            continue;
        }
        cerrar_cita(&mut cita, &mut salida);
        // Una tabla (`| a | b |`): se ve alineada en monoespaciada.
        if t.starts_with('|') && t.ends_with('|') && t.len() > 1 {
            cerrar_parrafo(&mut parrafo, &mut salida);
            cerrar_album(&mut album, &mut salida);
            let mut filas = vec![t];
            while i < renglones.len() {
                let s = renglones[i].trim();
                if !(s.starts_with('|') && s.ends_with('|')) {
                    break;
                }
                filas.push(s);
                i += 1;
            }
            salida.push(Bloque::Codigo(tabla_legible(&filas)));
            continue;
        }
        if es_regla(t) && espacios < 4 {
            cerrar_parrafo(&mut parrafo, &mut salida);
            cerrar_album(&mut album, &mut salida);
            salida.push(Bloque::Regla);
            continue;
        }
        if let Some((nivel, r)) = titulo(t) {
            cerrar_parrafo(&mut parrafo, &mut salida);
            cerrar_album(&mut album, &mut salida);
            salida.push(Bloque::Texto(Clase::Titulo(nivel), en_linea(r)));
            continue;
        }
        if let Some((clase, r)) = lista(linea.trim_end()) {
            cerrar_parrafo(&mut parrafo, &mut salida);
            cerrar_album(&mut album, &mut salida);
            salida.push(Bloque::Texto(clase, en_linea(r)));
            continue;
        }
        // Un renglon con fotos: las fotos van a su album y el texto que
        // quede alrededor, a su parrafo, en el orden en que se escribio.
        if t.contains("![") {
            let trozos = partir_imagenes(t);
            if trozos.iter().any(Result::is_ok) {
                for trozo in trozos {
                    match trozo {
                        Ok(imagen) if es_foto(&imagen.enlace) => {
                            cerrar_parrafo(&mut parrafo, &mut salida);
                            album.push(imagen);
                        }
                        Ok(adjunto) => {
                            cerrar_parrafo(&mut parrafo, &mut salida);
                            cerrar_album(&mut album, &mut salida);
                            salida.push(Bloque::Adjunto(adjunto));
                        }
                        Err(texto) if texto.trim().is_empty() => {}
                        Err(texto) => {
                            cerrar_album(&mut album, &mut salida);
                            parrafo.push(texto.trim());
                        }
                    }
                }
                continue;
            }
        }
        cerrar_album(&mut album, &mut salida);
        parrafo.push(linea.trim_end());
    }
    cerrar_parrafo(&mut parrafo, &mut salida);
    cerrar_cita(&mut cita, &mut salida);
    cerrar_album(&mut album, &mut salida);
    salida
}

/// Una tabla de Markdown con sus columnas alineadas y sin la fila de las
/// rayas (`|---|---|`), para verla en monoespaciada.
fn tabla_legible(filas: &[&str]) -> String {
    let celdas: Vec<Vec<String>> = filas
        .iter()
        .filter(|f| !f.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t')))
        .map(|f| {
            f.trim_matches('|')
                .split('|')
                .map(|c| en_linea(c.trim()).texto)
                .collect()
        })
        .collect();
    let columnas = celdas.iter().map(Vec::len).max().unwrap_or(0);
    let anchos: Vec<usize> = (0..columnas)
        .map(|k| {
            celdas
                .iter()
                .filter_map(|f| f.get(k))
                .map(|c| c.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    celdas
        .iter()
        .map(|f| {
            (0..columnas)
                .map(|k| {
                    let c = f.get(k).map(String::as_str).unwrap_or_default();
                    format!("{c:<w$}", w = anchos[k])
                })
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// El estilo con que se va escribiendo dentro de un renglon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Estilo {
    negrita: bool,
    cursiva: bool,
    codigo: bool,
    tachado: bool,
}

/// Quien va escribiendo el texto limpio y sus marcas.
struct Escritor {
    linea: Linea,
    /// Lo que mide `linea.texto` en UTF-16, llevado a mano.
    largo16: u32,
}

impl Escritor {
    fn poner(&mut self, s: &str, estilo: Estilo) {
        if s.is_empty() {
            return;
        }
        let largo = s.encode_utf16().count() as u32;
        if estilo != Estilo::default() {
            // Seguido de otro trozo con el mismo estilo: una sola marca.
            match self.linea.marcas.last_mut() {
                Some(m)
                    if m.inicio + m.largo == self.largo16
                        && (m.negrita, m.cursiva, m.codigo, m.tachado)
                            == (
                                estilo.negrita,
                                estilo.cursiva,
                                estilo.codigo,
                                estilo.tachado,
                            ) =>
                {
                    m.largo += largo
                }
                _ => self.linea.marcas.push(Marca {
                    inicio: self.largo16,
                    largo,
                    negrita: estilo.negrita,
                    cursiva: estilo.cursiva,
                    codigo: estilo.codigo,
                    tachado: estilo.tachado,
                }),
            }
        }
        self.linea.texto.push_str(s);
        self.largo16 += largo;
    }
}

/// El texto de un renglon (o un parrafo) sin las marcas del Markdown en
/// linea: `**negrita**`, `*cursiva*`, `` `codigo` ``, `~~tachado~~`,
/// `[enlace](url)` y `<https://…>`.
pub fn en_linea(fuente: &str) -> Linea {
    let mut e = Escritor {
        linea: Linea::default(),
        largo16: 0,
    };
    escribir(fuente, Estilo::default(), &mut e);
    e.linea
}

/// Si `c` es una letra o cifra: un `_` entre dos de ellas es del nombre
/// (`mi_fichero_nuevo`), no una cursiva.
fn es_palabra(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Donde cierra `delim` en `s`, con algo dentro que no empiece ni acabe en
/// blanco. Para `*` y `_` solos, sin tomar la mitad de un `**`.
fn cierre(s: &str, delim: &str) -> Option<usize> {
    let mut desde = 0;
    while let Some(j) = s[desde..].find(delim) {
        let j = desde + j;
        let dentro = &s[..j];
        let doble_de_mas =
            delim.len() == 1 && (s[j + 1..].starts_with(delim) || dentro.ends_with(delim));
        let detras = s[j + delim.len()..].chars().next();
        let vale = !dentro.is_empty()
            && !dentro.starts_with(char::is_whitespace)
            && !dentro.ends_with(char::is_whitespace)
            && !doble_de_mas
            && !(delim == "_" && es_palabra(detras));
        if vale {
            return Some(j);
        }
        // Un `**` con un blanco delante y letra detras no cierra: abre otra
        // (`**abierto y **cerrado**`). Esa se queda con el cierre de mas
        // alla, y la de fuera no cierra nunca: se ve tal cual, como en
        // cualquier lector de Markdown, y no tine de negrita medio renglon.
        let abre_otra =
            dentro.ends_with(char::is_whitespace) && detras.is_some_and(|c| !c.is_whitespace());
        if abre_otra && !doble_de_mas {
            return None;
        }
        desde = j + delim.len();
    }
    None
}

fn escribir(s: &str, estilo: Estilo, e: &mut Escritor) {
    let mut i = 0;
    // El comienzo del trozo llano que se va acumulando.
    let mut llano = 0;
    let mut anterior: Option<char> = None;
    while i < s.len() {
        let resto = &s[i..];
        let c = resto.chars().next().unwrap_or_default();
        let mut hecho: Option<usize> = None;
        // Escapes: `\*` es un asterisco.
        if c == '\\'
            && let Some(sig) = resto[1..].chars().next()
            && sig.is_ascii_punctuation()
        {
            e.poner(&s[llano..i], estilo);
            e.poner(&resto[1..1 + sig.len_utf8()], estilo);
            hecho = Some(1 + sig.len_utf8());
        } else if c == '`' {
            let tiras = resto.chars().take_while(|c| *c == '`').count();
            let valla = &resto[..tiras];
            if let Some(j) = resto[tiras..].find(valla) {
                e.poner(&s[llano..i], estilo);
                let dentro = &resto[tiras..tiras + j];
                let dentro = if dentro.starts_with(' ') && dentro.ends_with(' ') && dentro.len() > 1
                {
                    &dentro[1..dentro.len() - 1]
                } else {
                    dentro
                };
                e.poner(
                    dentro,
                    Estilo {
                        codigo: true,
                        ..estilo
                    },
                );
                hecho = Some(tiras + j + tiras);
            }
        } else if c == '[' && !estilo.codigo {
            if let Some((texto, url, largo)) = enlace_en(resto) {
                e.poner(&s[llano..i], estilo);
                let inicio = e.largo16;
                escribir(texto, estilo, e);
                let largo16 = e.largo16 - inicio;
                if largo16 > 0 {
                    e.linea.enlaces.push(Enlace {
                        inicio,
                        largo: largo16,
                        url: url.to_string(),
                    });
                }
                hecho = Some(largo);
            }
        } else if c == '<'
            && let Some(j) = resto.find('>')
            && {
                let url = &resto[1..j];
                (url.starts_with("http://") || url.starts_with("https://"))
                    && !url.contains(char::is_whitespace)
            }
        {
            e.poner(&s[llano..i], estilo);
            let url = &resto[1..j];
            let inicio = e.largo16;
            e.poner(url, estilo);
            e.linea.enlaces.push(Enlace {
                inicio,
                largo: e.largo16 - inicio,
                url: url.to_string(),
            });
            hecho = Some(j + 1);
        } else {
            // Las marcas de dos: negrita y tachado.
            for (delim, cambia) in [("**", 0u8), ("__", 0), ("~~", 1), ("*", 2), ("_", 2)] {
                if !resto.starts_with(delim) {
                    continue;
                }
                // Un `_` pegado a una palabra por delante no abre nada.
                if delim.starts_with('_') && es_palabra(anterior) {
                    break;
                }
                let tras = &resto[delim.len()..];
                if let Some(j) = cierre(tras, delim) {
                    e.poner(&s[llano..i], estilo);
                    let nuevo = match cambia {
                        0 => Estilo {
                            negrita: true,
                            ..estilo
                        },
                        1 => Estilo {
                            tachado: true,
                            ..estilo
                        },
                        _ => Estilo {
                            cursiva: true,
                            ..estilo
                        },
                    };
                    escribir(&tras[..j], nuevo, e);
                    hecho = Some(delim.len() + j + delim.len());
                }
                break;
            }
        }
        match hecho {
            Some(n) => {
                anterior = s[..i + n].chars().next_back();
                i += n;
                llano = i;
            }
            None => {
                anterior = Some(c);
                i += c.len_utf8();
            }
        }
    }
    e.poner(&s[llano..], estilo);
}

/// Un `[texto](url)` al principio de `s`: su texto, su direccion y lo que
/// ocupa.
fn enlace_en(s: &str) -> Option<(&str, &str, usize)> {
    let s2 = s.strip_prefix('[')?;
    // El corchete que cierra, contando los que se abren dentro.
    let mut nivel = 0;
    let mut cierre = None;
    for (k, c) in s2.char_indices() {
        match c {
            '[' => nivel += 1,
            ']' if nivel == 0 => {
                cierre = Some(k);
                break;
            }
            ']' => nivel -= 1,
            _ => {}
        }
    }
    let cierre = cierre?;
    let tras = s2[cierre + 1..].strip_prefix('(')?;
    let fin = cierre_del_parentesis(tras)?;
    let url = tras[..fin].trim();
    if url.is_empty() || url.contains(char::is_whitespace) {
        return None;
    }
    Some((&s2[..cierre], url, 1 + cierre + 2 + fin + 1))
}

/// Si una nota tiene algo que ensenar con formato. Una sin nada de esto se
/// pinta como siempre, renglon a renglon y con sus enlaces sueltos: asi una
/// nota llana no cambia ni un pixel.
pub fn tiene_formato(bloques: &[Bloque], original: &str) -> bool {
    match bloques {
        [] => false,
        [Bloque::Texto(Clase::Parrafo, l)] => {
            // Los blancos de final de renglon y los `\r\n` no son formato:
            // el parrafo los quita, y la nota llana tiene que seguir llana.
            let llano = original
                .trim()
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n");
            !l.marcas.is_empty() || !l.enlaces.is_empty() || l.texto != llano
        }
        _ => true,
    }
}

/// Las fotos de una nota, en orden: las del album y ninguna mas.
pub fn imagenes(bloques: &[Bloque]) -> impl Iterator<Item = &Imagen> {
    bloques.iter().flat_map(|b| match b {
        Bloque::Imagenes(v) => v.as_slice(),
        _ => &[],
    })
}

/// Una caja en coordenadas del album (su esquina de arriba a la izquierda
/// es el cero).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caja {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

/// Cuantas fotos van en cada fila de un album de `n`: de tres en tres, con
/// la cuenta hecha para que ninguna fila se quede con una sola (salvo la
/// primera de tres, que es la grande, como en Telegram).
pub fn filas_del_album(n: usize) -> Vec<usize> {
    match n {
        0 => Vec::new(),
        1 => vec![1],
        2 => vec![2],
        3 => vec![1, 2],
        4 => vec![2, 2],
        _ => {
            let mut filas = vec![3; n / 3];
            match n % 3 {
                1 => {
                    // 3+3+1 se ve mal: mejor 3+2+2.
                    filas.pop();
                    filas.extend([2, 2]);
                }
                2 => filas.push(2),
                _ => {}
            }
            filas
        }
    }
}

/// **El album**: donde va cada foto, dadas sus proporciones (ancho / alto,
/// un 4:3 si aun no se sabe), el ancho que hay, el alto como mucho de una
/// foto sola y el de cada fila, y el hueco entre fotos. Devuelve las cajas
/// en el orden de las fotos y el alto total.
///
/// Una foto sola guarda su proporcion entera (como una foto del chat:
/// `tamano_de_vista`). En una fila de varias, todas toman el mismo alto, el
/// que hace que juntas llenen el ancho; si ese alto se pasa de los topes,
/// cada una se recorta por el centro al pintarla.
pub fn album(
    proporciones: &[f32],
    ancho: f32,
    alto_solo: f32,
    alto_fila: f32,
    hueco: f32,
) -> (Vec<Caja>, f32) {
    let prop = |r: f32| {
        if r.is_finite() && r > 0.0 {
            r
        } else {
            4.0 / 3.0
        }
    };
    let ancho = ancho.max(1.0);
    if proporciones.len() == 1 {
        let r = prop(proporciones[0]);
        let w = ancho.min(alto_solo * r).max(1.0);
        let h = (w / r).max(1.0);
        return (
            vec![Caja {
                x: 0.0,
                y: 0.0,
                ancho: w,
                alto: h,
            }],
            h,
        );
    }
    let mut cajas = Vec::with_capacity(proporciones.len());
    let mut y = 0.0f32;
    let mut k = 0;
    for (f, cuantas) in filas_del_album(proporciones.len()).into_iter().enumerate() {
        if f > 0 {
            y += hueco;
        }
        let fila = &proporciones[k..k + cuantas];
        let libre = (ancho - hueco * (cuantas as f32 - 1.0)).max(1.0);
        let suma: f32 = fila.iter().map(|r| prop(*r)).sum();
        // Ni tan bajas que no se vean ni tan altas que se coman el chat.
        let h = (libre / suma).clamp(ancho / 6.0, alto_fila).max(1.0);
        // El ancho de cada una sale de su proporcion; la ultima se lleva lo
        // que sobre del redondeo, para que la fila acabe justo en el borde.
        let mut x = 0.0f32;
        for (j, r) in fila.iter().enumerate() {
            let w = if j + 1 == cuantas {
                (ancho - x).max(1.0)
            } else {
                (libre * prop(*r) / suma).max(1.0)
            };
            cajas.push(Caja {
                x,
                y,
                ancho: w,
                alto: h,
            });
            x += w + hueco;
        }
        y += h;
        k += cuantas;
    }
    (cajas, y)
}

/// El texto de una nota para una sola linea (la lista de chats, la cita de
/// una respuesta): sin las marcas y con las fotos como «🖼».
pub fn resumen(texto: &str) -> String {
    let mut salida: Vec<String> = Vec::new();
    for b in bloques(texto) {
        match b {
            Bloque::Texto(_, l) => salida.push(l.texto),
            Bloque::Codigo(c) => salida.push(c),
            Bloque::Regla => {}
            Bloque::Imagenes(v) => salida.push(
                v.iter()
                    .map(|i| {
                        if i.alt.is_empty() {
                            "🖼".to_string()
                        } else {
                            format!("🖼 {}", i.alt)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            Bloque::Adjunto(i) => salida.push(format!("📎 {}", i.alt)),
        }
    }
    salida.join("\n")
}

/// Lo que hay que escribir en `antes|despues` (la rayita en medio) para que
/// `renglones` queden cada uno en su renglon: un salto delante si la rayita
/// no esta a principio de renglon, y otro detras para seguir escribiendo
/// debajo. Es lo que hace la caja de escribir al pegar una foto con texto.
pub fn en_su_renglon(antes: &str, despues: &str, renglones: &[String]) -> String {
    let mut s = String::new();
    if !antes.is_empty() && !antes.ends_with('\n') {
        s.push('\n');
    }
    s.push_str(&renglones.join("\n"));
    if !despues.starts_with('\n') {
        s.push('\n');
    }
    s
}

/// El renglon de la foto `numero` (desde 1, en el orden de la nota) que se
/// guardo en `notas/<fichero>` de la carpeta del proyecto, como lo deja la
/// caja de escribir: relativo y con su numero delante (`![foto 2](…)`),
/// que es lo que se lee mientras se escribe —el nombre del fichero es una
/// hora de trece cifras— y lo que dice el resumen («🖼 foto 2»).
pub fn renglon_de_foto(fichero: &str, numero: usize) -> String {
    format!("![foto {numero}]({NOTAS}{fichero})")
}

/// La carpeta de las fotos de las notas, dentro de la del proyecto (la
/// misma que usa el editor de notas, `notas_md::adjuntos::sitio`).
pub const NOTAS: &str = "notas/";

/// Lo escrito, con los enlaces relativos a `notas/` pasados a la ruta
/// portatil de esa carpeta (`pixpin:files/guardados/pc/<chat>/notas`): asi
/// viaja la nota con sus fotos y el movil las encuentra (la sincronizacion
/// solo lleva lo que se nombra con `pixpin:files/`). Sin portatil (un
/// proyecto fuera del indice) se queda como esta, que aqui tambien se lee.
pub fn enlaces_portatiles(texto: &str, carpeta_portatil: Option<&str>) -> String {
    match carpeta_portatil {
        Some(c) => texto.replace(
            &format!("]({NOTAS}"),
            &format!("]({}/", c.trim_end_matches('/')),
        ),
        None => texto.to_string(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn linea(t: &str) -> Linea {
        en_linea(t)
    }

    #[test]
    fn la_negrita_la_cursiva_el_codigo_y_el_tachado_se_ven_sin_sus_marcas() {
        let l = linea("hola **mundo** y *tu* con `x` y ~~no~~");
        assert_eq!(l.texto, "hola mundo y tu con x y no");
        let m = |inicio, largo| {
            l.marcas
                .iter()
                .find(|m| m.inicio == inicio && m.largo == largo)
        };
        assert!(m(5, 5).is_some_and(|m| m.negrita && !m.cursiva));
        assert!(m(13, 2).is_some_and(|m| m.cursiva));
        assert!(m(20, 1).is_some_and(|m| m.codigo));
        assert!(m(24, 2).is_some_and(|m| m.tachado));
        // Anidadas: la cursiva dentro de la negrita es las dos cosas.
        let n = linea("**a *b* c**");
        assert_eq!(n.texto, "a b c");
        assert!(
            n.marcas
                .iter()
                .any(|m| m.negrita && m.cursiva && m.largo == 1)
        );
    }

    #[test]
    fn lo_que_no_cierra_o_es_parte_de_un_nombre_se_queda_como_texto() {
        // Casos negativos: ni un asterisco suelto ni un nombre con guiones
        // bajos son formato.
        assert_eq!(linea("5 * 3 = 15").texto, "5 * 3 = 15");
        assert!(linea("5 * 3 = 15").marcas.is_empty());
        assert_eq!(linea("mi_fichero_nuevo.png").texto, "mi_fichero_nuevo.png");
        assert!(linea("mi_fichero_nuevo.png").marcas.is_empty());
        assert_eq!(linea("**abierto").texto, "**abierto");
        assert_eq!(linea(r"\*no\*").texto, "*no*");
    }

    #[test]
    fn un_enlace_ensena_su_texto_y_guarda_su_direccion_en_utf16() {
        let l = linea("ver ñ [la obra](https://x.es/a) y <https://y.es>");
        assert_eq!(l.texto, "ver ñ la obra y https://y.es");
        assert_eq!(
            l.enlaces[0],
            Enlace {
                inicio: 6,
                largo: 7,
                url: "https://x.es/a".into()
            }
        );
        assert_eq!(l.enlaces[1].url, "https://y.es");
        // Un emoji son dos unidades UTF-16: el enlace de detras se corre dos.
        let e = linea("🙂 [a](https://z.es)");
        assert_eq!(e.enlaces[0].inicio, 3);
    }

    #[test]
    fn una_nota_con_texto_y_fotos_queda_en_sus_bloques_y_las_seguidas_son_un_album() {
        let nota = "hola\n![](pixpin:files/guardados/pc/c/notas/1-a.png)\n![b|300](pixpin:files/guardados/pc/c/notas/2-b.jpg)\n\nY sigue **aqui**\n![](notas/3-c.png)";
        let b = bloques(nota);
        assert_eq!(b.len(), 4, "{b:?}");
        assert!(matches!(&b[0], Bloque::Texto(Clase::Parrafo, l) if l.texto == "hola"));
        match &b[1] {
            Bloque::Imagenes(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[1].alt, "b", "el ancho del editor no es del alt");
                assert!(v[0].enlace.ends_with("1-a.png"));
            }
            otro => panic!("{otro:?}"),
        }
        assert!(matches!(&b[2], Bloque::Texto(Clase::Parrafo, l) if l.texto == "Y sigue aqui"));
        assert!(matches!(&b[3], Bloque::Imagenes(v) if v.len() == 1));
        // Ninguna ruta se ve como texto.
        assert!(!resumen(nota).contains("pixpin:"));
        assert_eq!(imagenes(&b).count(), 3);
    }

    #[test]
    fn una_foto_en_medio_de_un_renglon_parte_el_renglon_en_su_sitio() {
        let b = bloques("antes ![](a.png) despues");
        assert_eq!(b.len(), 3);
        assert!(matches!(&b[0], Bloque::Texto(_, l) if l.texto == "antes"));
        assert!(matches!(&b[1], Bloque::Imagenes(_)));
        assert!(matches!(&b[2], Bloque::Texto(_, l) if l.texto == "despues"));
        // Un adjunto que no es foto se nombra, no se pinta como foto.
        let d = bloques("![Plano.pdf](pixpin:files/guardados/pc/c/notas/1-Plano.pdf)");
        assert!(matches!(&d[0], Bloque::Adjunto(i) if i.alt == "Plano.pdf"));
        // Caso negativo: un `![` sin cerrar es texto.
        assert!(
            matches!(&bloques("mira ![esto")[0], Bloque::Texto(_, l) if l.texto == "mira ![esto")
        );
    }

    #[test]
    fn titulos_listas_casillas_citas_codigo_y_rayas() {
        let nota = "# Obra\n- uno\n  - dos\n1. primero\n- [x] hecho\n- [ ] falta\n> dicho\n> y mas\n---\n```\nlet a = 1;\n```\n| a | b |\n|---|---|\n| 1 | 22 |";
        let b = bloques(nota);
        assert!(matches!(&b[0], Bloque::Texto(Clase::Titulo(1), l) if l.texto == "Obra"));
        assert!(matches!(&b[1], Bloque::Texto(Clase::Vineta(0), _)));
        assert!(matches!(&b[2], Bloque::Texto(Clase::Vineta(1), l) if l.texto == "dos"));
        assert!(matches!(&b[3], Bloque::Texto(Clase::Numero(0, n), _) if n == "1."));
        assert!(matches!(&b[4], Bloque::Texto(Clase::Casilla(0, true), l) if l.texto == "hecho"));
        assert!(matches!(&b[5], Bloque::Texto(Clase::Casilla(0, false), _)));
        assert!(matches!(&b[6], Bloque::Texto(Clase::Cita, l) if l.texto == "dicho\ny mas"));
        assert_eq!(b[7], Bloque::Regla);
        assert_eq!(b[8], Bloque::Codigo("let a = 1;".into()));
        assert_eq!(b[9], Bloque::Codigo("a  b\n1  22".into()));
    }

    #[test]
    fn una_nota_llana_no_tiene_formato_y_se_pinta_como_siempre() {
        let llana = "hola\nque tal https://x.es";
        assert!(!tiene_formato(&bloques(llana), llana));
        // Dos parrafos ya son algo que ordenar, y una negrita tambien.
        assert!(tiene_formato(&bloques("a\n\nb"), "a\n\nb"));
        assert!(tiene_formato(&bloques("**a**"), "**a**"));
        assert!(!tiene_formato(&bloques(""), ""));
    }

    #[test]
    fn el_album_reparte_las_filas_como_telegram() {
        assert_eq!(filas_del_album(2), vec![2]);
        assert_eq!(filas_del_album(3), vec![1, 2]);
        assert_eq!(filas_del_album(5), vec![3, 2]);
        assert_eq!(filas_del_album(7), vec![3, 2, 2]);
        assert_eq!(filas_del_album(9), vec![3, 3, 3]);
        assert_eq!(filas_del_album(7).iter().sum::<usize>(), 7);
    }

    #[test]
    fn una_foto_sola_guarda_su_proporcion_y_no_pasa_del_tope() {
        // Apaisada: todo el ancho.
        let (c, alto) = album(&[2.0], 400.0, 300.0, 200.0, 2.0);
        assert_eq!((c[0].ancho, c[0].alto, alto), (400.0, 200.0, 200.0));
        // Vertical: el tope de alto manda y la foto se estrecha.
        let (c, alto) = album(&[0.5], 400.0, 300.0, 200.0, 2.0);
        assert_eq!((c[0].ancho, c[0].alto, alto), (150.0, 300.0, 300.0));
        // Sin medidas, un 4:3 en vez de nada.
        let (c, _) = album(&[f32::NAN], 400.0, 600.0, 200.0, 2.0);
        assert_eq!(c[0].alto, 300.0);
    }

    #[test]
    fn las_filas_del_album_llenan_el_ancho_justo_y_no_se_pisan() {
        let (c, alto) = album(&[1.0, 1.5, 0.75, 1.0, 1.0], 400.0, 300.0, 200.0, 2.0);
        assert_eq!(c.len(), 5);
        // Fila de tres y fila de dos, cada una acaba en el borde.
        for fila in [&c[0..3], &c[3..5]] {
            let ultima = fila.last().unwrap();
            assert!((ultima.x + ultima.ancho - 400.0).abs() < 0.01);
            assert!(
                fila.iter()
                    .all(|k| k.y == fila[0].y && k.alto == fila[0].alto)
            );
            for par in fila.windows(2) {
                assert!(par[0].x + par[0].ancho <= par[1].x);
            }
        }
        assert!(c[3].y >= c[0].y + c[0].alto + 2.0);
        assert!((alto - (c[3].y + c[3].alto)).abs() < 0.01);
    }
    #[test]
    fn una_foto_pegada_va_en_su_propio_renglon_y_sale_portatil() {
        let f = vec![renglon_de_foto("1-pegado.png", 1)];
        assert_eq!(
            en_su_renglon("hola", "", &f),
            "\n![foto 1](notas/1-pegado.png)\n"
        );
        // A principio de renglon y con mas texto detras, sin saltos de mas.
        assert_eq!(
            en_su_renglon("hola\n", "\nadios", &f),
            "![foto 1](notas/1-pegado.png)"
        );
        let nota = format!("hola\n{}\n[ver](notas/x.pdf)", f[0]);
        assert_eq!(
            enlaces_portatiles(&nota, Some("pixpin:files/guardados/pc/c1/notas/")),
            "hola\n![foto 1](pixpin:files/guardados/pc/c1/notas/1-pegado.png)\n[ver](pixpin:files/guardados/pc/c1/notas/x.pdf)"
        );
        // Caso negativo: sin ruta portatil no se toca, y lo que ya era
        // portatil o de otra carpeta tampoco.
        assert_eq!(enlaces_portatiles(&nota, None), nota);
        let otra = "![](archivos/a.png) ![](pixpin:files/notas/b.png)";
        assert_eq!(enlaces_portatiles(otra, Some("pixpin:files/x/notas")), otra);
        // Y la foto relativa se lee como foto.
        assert!(matches!(&bloques(&nota)[1], Bloque::Imagenes(_)));
        assert_eq!(resumen(&nota), "hola\n🖼 foto 1\nver");
    }

    #[test]
    fn una_ruta_de_windows_con_espacios_y_parentesis_es_una_sola_foto() {
        let b = bloques(r"![](C:\Users\Max Antony\Fotos (2)\obra 1.png) y sigue");
        match &b[0] {
            Bloque::Imagenes(v) => {
                assert_eq!(v[0].enlace, r"C:\Users\Max Antony\Fotos (2)\obra 1.png")
            }
            otro => panic!("{otro:?}"),
        }
        assert!(matches!(&b[1], Bloque::Texto(_, l) if l.texto == "y sigue"));
        // Entre `<…>` vale un parentesis suelto.
        let b = bloques("![](<C:/a (b.png>)");
        assert!(matches!(&b[0], Bloque::Imagenes(v) if v[0].enlace == "C:/a (b.png"));
        // Y un enlace de la Wikipedia no pierde su final.
        let l = linea("[casa](https://es.wikipedia.org/wiki/Casa_(edificio)) fin");
        assert_eq!(l.texto, "casa fin");
        assert_eq!(
            l.enlaces[0].url,
            "https://es.wikipedia.org/wiki/Casa_(edificio)"
        );
    }

    #[test]
    fn lo_que_va_en_codigo_no_es_una_foto() {
        // Quien explica la sintaxis la escribe entre comillas: es texto.
        let b = bloques("se pone `![](foto.png)` asi");
        assert_eq!(b.len(), 1, "{b:?}");
        assert!(
            matches!(&b[0], Bloque::Texto(Clase::Parrafo, l) if l.texto == "se pone ![](foto.png) asi")
        );
        // Y en un bloque de codigo, igual.
        let b = bloques("```\n![](x.png)\n```");
        assert_eq!(b, vec![Bloque::Codigo("![](x.png)".into())]);
        // Caso de control: fuera de las comillas si lo es.
        let b = bloques("`a` ![](foto.png)");
        assert!(b.iter().any(|x| matches!(x, Bloque::Imagenes(_))));
    }

    #[test]
    fn los_blancos_del_final_y_los_saltos_de_windows_no_son_formato() {
        let llana = "hola  \r\nque tal\t\r\nadios";
        assert!(!tiene_formato(&bloques(llana), llana));
        // Caso negativo: un escape si cambia lo que se ve.
        assert!(tiene_formato(&bloques(r"5 \* 3"), r"5 \* 3"));
    }

    #[test]
    fn las_marcas_sin_cerrar_y_los_emojis_no_descuadran_las_posiciones() {
        // Un `**` sin cerrar se queda; la negrita de detras empieza donde
        // toca contando el emoji (2 en UTF-16) y la ñ (1).
        let l = linea("🙂 ñ **abierto y **cerrado**");
        assert_eq!(l.texto, "🙂 ñ **abierto y cerrado");
        let m = l.marcas.iter().find(|m| m.negrita).unwrap();
        let antes: u32 = "🙂 ñ **abierto y ".encode_utf16().count() as u32;
        assert_eq!((m.inicio, m.largo), (antes, 7));
    }
}
