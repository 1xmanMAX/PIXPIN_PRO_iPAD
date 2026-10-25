//! **El telepronter: el texto que baja solo mientras uno lo lee en voz alta.**
//!
//! Es `guardados/TelepronterActivity.kt` (540 lineas de Compose) con la
//! ventana quitada: aqui esta la cuenta, y la cuenta es casi todo lo que
//! hay. La ventana vive en `apps/pixpin/src/teleprompter.rs`.
//!
//! Lo que hay que entender de esta pantalla es **por que no pasa por ningun
//! reconocedor**: el texto ya se tiene, y como baja a una velocidad que se
//! conoce, se sabe en que segundo cada parrafo cruzo la franja de lectura.
//! Ese es su minuto. Da igual que baje solo o que se empuje con la rueda:
//! se mira **donde esta**, no a que velocidad va (`TelepronterActivity.kt:176-186`).
//! De ahi sale un cuerpo `[m:ss] parrafo` identico al de una transcripcion,
//! y la nota entra en el chat como `clase = VOZ` con el texto ya puesto.
//!
//! Aqui no hay Windows, ni sonido, ni pintura: solo el desfile.

use crate::tiempos::marca_de_tiempo;

/// A que altura de la pantalla esta la linea de lectura, en fraccion desde
/// arriba. `FRANJA` (`TelepronterActivity.kt:505`).
pub const FRANJA: f32 = 0.3;

/// Desde cuanto se cuenta antes de arrancar a grabar. `CUENTA_ATRAS` (:540).
pub const CUENTA_ATRAS: u32 = 3;

/// Velocidad del desfile, en pixeles logicos por segundo. Minimo, maximo y
/// la de partida (`TelepronterActivity.kt`, el deslizador de velocidad).
pub const VELOCIDAD_MINIMA: f32 = 10.0;
pub const VELOCIDAD_MAXIMA: f32 = 150.0;
pub const VELOCIDAD_POR_DEFECTO: f32 = 45.0;

/// Tamano de la letra, en puntos. 16 a 48, como en el movil.
pub const TAMANO_MINIMO: f32 = 16.0;
pub const TAMANO_MAXIMO: f32 = 48.0;
pub const TAMANO_POR_DEFECTO: f32 = 28.0;

/// La velocidad dicha con palabras: un numero de pixeles por segundo no
/// significa nada para quien va a leer. `nombreDeLaVelocidad` (:531-537).
///
/// Devuelve la **clave** del catalogo y no el texto: este crate no sabe en
/// que idioma esta la interfaz, y meter aqui una palabra en castellano la
/// dejaria en castellano para siempre.
pub fn clave_de_la_velocidad(v: f32) -> &'static str {
    if v < 30.0 {
        "telepronter-velocidad-muy-lenta"
    } else if v < 55.0 {
        "telepronter-velocidad-lenta"
    } else if v < 85.0 {
        "telepronter-velocidad-normal"
    } else if v < 115.0 {
        "telepronter-velocidad-rapida"
    } else {
        "telepronter-velocidad-muy-rapida"
    }
}

/// Deja una velocidad dentro de lo que el deslizador permite.
pub fn acotar_velocidad(v: f32) -> f32 {
    v.clamp(VELOCIDAD_MINIMA, VELOCIDAD_MAXIMA)
}

pub fn acotar_tamano(t: f32) -> f32 {
    t.clamp(TAMANO_MINIMO, TAMANO_MAXIMO)
}

/// El texto en parrafos para leer.
///
/// `parrafosDe` (`TelepronterActivity.kt:516-525`): fuera las marcas de
/// Markdown que no se leen en voz alta (`#`, `**`, `>`, vinetas, imagenes),
/// y partido por lineas en blanco. Las lineas de un mismo parrafo se juntan
/// con un espacio, porque al leer no se hace pausa en el salto de linea.
pub fn parrafos_de(texto: &str) -> Vec<String> {
    bloques(texto)
        .into_iter()
        .filter_map(|bloque| {
            let limpio = bloque
                .lines()
                // Una imagen de Markdown no se lee: se mira, y aqui no se ve.
                .filter(|l| !l.trim_start().starts_with("!["))
                .map(|l| {
                    l.trim_start_matches(['#', ' ', '>', '-', '*'])
                        .replace("**", "")
                        .replace("__", "")
                        .replace('`', "")
                        .trim()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(" ");
            let limpio = limpio.trim().to_string();
            (!limpio.is_empty()).then_some(limpio)
        })
        .collect()
}

/// Parte por lineas en blanco (`\n\s*\n` del movil), sin traer `regex`.
fn bloques(texto: &str) -> Vec<String> {
    let mut salida = Vec::new();
    let mut actual: Vec<&str> = Vec::new();
    for linea in texto.lines() {
        if linea.trim().is_empty() {
            if !actual.is_empty() {
                salida.push(actual.join("\n"));
                actual.clear();
            }
        } else {
            actual.push(linea);
        }
    }
    if !actual.is_empty() {
        salida.push(actual.join("\n"));
    }
    salida
}

/// Un parrafo ya medido: donde empieza dentro de la columna, sin desplazar,
/// y cuanto ocupa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Medida {
    pub cima: f32,
    pub alto: f32,
}

/// El desfile: por donde va el texto y en que milisegundo cruzo cada
/// parrafo la linea de lectura.
///
/// Nace sin medir: quien pinta mide con su tipografia y su tamano y lo
/// entrega con [`Desfile::medir`]. Asi el desfile no sabe nada de fuentes y
/// se prueba con numeros a mano.
#[derive(Debug, Clone, Default)]
pub struct Desfile {
    medidas: Vec<Medida>,
    /// En que milisegundo cruzo cada parrafo; `-1` es «todavia no».
    tiempos: Vec<i64>,
    /// Cuanto se ha desplazado el texto hacia arriba, en pixeles.
    pub y: f32,
    alto_total: f32,
    alto_visible: f32,
}

impl Desfile {
    pub fn nuevo(parrafos: usize) -> Desfile {
        Desfile {
            medidas: Vec::new(),
            tiempos: vec![-1; parrafos],
            y: 0.0,
            alto_total: 0.0,
            alto_visible: 0.0,
        }
    }

    /// Lo que mide cada parrafo y cuanto se ve de la ventana. Se llama en
    /// cada fotograma: cambiar el tamano de la letra vuelve a medir, y el
    /// desfile tiene que seguir cuadrando.
    pub fn medir(&mut self, medidas: Vec<Medida>, alto_visible: f32) {
        self.alto_total = medidas
            .last()
            .map(|m| m.cima + m.alto)
            .unwrap_or(0.0)
            .max(0.0);
        self.alto_visible = alto_visible.max(0.0);
        if medidas.len() != self.tiempos.len() {
            // Cambio el texto: los minutos de antes ya no son de nadie.
            self.tiempos = vec![-1; medidas.len()];
        }
        self.medidas = medidas;
        self.y = self.acotar(self.y);
    }

    /// Solo cambia cuanto se ve, sin volver a medir los parrafos.
    ///
    /// Es lo que se llama en cada fotograma cuando la letra no ha cambiado:
    /// medir un texto largo con DirectWrite cuesta, y la ventana puede
    /// haberse redimensionado.
    pub fn ajustar_ventana(&mut self, alto_visible: f32) {
        self.alto_visible = alto_visible.max(0.0);
        self.y = self.acotar(self.y);
    }

    /// Lo que mide un parrafo, para pintarlo donde toca.
    pub fn medida(&self, i: usize) -> Option<Medida> {
        self.medidas.get(i).copied()
    }

    /// Hasta donde se puede bajar: el final del texto se queda a la vista,
    /// no se va por arriba.
    pub fn tope(&self) -> f32 {
        (self.alto_total - self.alto_visible).max(0.0)
    }

    fn acotar(&self, y: f32) -> f32 {
        y.clamp(0.0, self.tope())
    }

    /// Donde cae la linea de lectura, en pixeles desde arriba de la ventana.
    pub fn linea_de_lectura(&self) -> f32 {
        self.alto_visible * FRANJA
    }

    /// Baja el texto lo que toque en `segundos` a esa velocidad. Devuelve
    /// `true` cuando ya se llego al final y no hay mas que bajar, que es lo
    /// que para el desfile solo (`TelepronterActivity.kt:163-171`).
    pub fn avanzar(&mut self, segundos: f32, velocidad: f32) -> bool {
        self.y = self.acotar(self.y + acotar_velocidad(velocidad) * segundos.max(0.0));
        self.y >= self.tope()
    }

    /// Mueve el texto a mano (la rueda, las teclas). Cuenta igual que el
    /// desfile: los minutos salen de donde esta el texto, no de como llego.
    pub fn empujar(&mut self, pixeles: f32) {
        self.y = self.acotar(self.y + pixeles);
    }

    /// Vuelve arriba y olvida los minutos. Es lo que hace empezar a grabar.
    pub fn reiniciar(&mut self) {
        self.y = 0.0;
        self.tiempos = vec![-1; self.tiempos.len()];
    }

    /// Apunta, en `ms`, los parrafos que ya han cruzado la linea de
    /// lectura y no tenian minuto. Se llama cada vez que el texto se mueve.
    ///
    /// Solo la primera vez: volver a subir el texto no le quita el minuto a
    /// un parrafo que ya se leyo.
    pub fn apuntar(&mut self, ms: i64) {
        let linea = self.linea_de_lectura();
        for (i, m) in self.medidas.iter().enumerate() {
            if self.tiempos[i] < 0 && m.cima - self.y <= linea {
                self.tiempos[i] = ms.max(0);
            }
        }
    }

    /// El minuto de un parrafo, o `None` si todavia no cruzo.
    pub fn tiempo(&self, i: usize) -> Option<i64> {
        self.tiempos.get(i).copied().filter(|t| *t >= 0)
    }

    /// Los parrafos que se ven ahora mismo, como rango de indices.
    ///
    /// Pintar los mil parrafos de un texto largo en cada fotograma es lo
    /// que convierte un desfile suave en una cinta a tirones.
    pub fn visibles(&self) -> std::ops::Range<usize> {
        let arriba = self.y;
        let abajo = self.y + self.alto_visible;
        let primero = self
            .medidas
            .iter()
            .position(|m| m.cima + m.alto > arriba)
            .unwrap_or(self.medidas.len());
        let ultimo = self
            .medidas
            .iter()
            .rposition(|m| m.cima < abajo)
            .map(|i| i + 1)
            .unwrap_or(primero);
        primero..ultimo.max(primero)
    }

    /// El cuerpo que se guarda en `Mensaje.transcripcion`: cada parrafo con
    /// su minuto, separados por un renglon en blanco.
    ///
    /// Lo que no llego a cruzar la linea se queda con el final de la
    /// grabacion: se leyo, si acaso, al acabar (`terminar()` :230-233).
    pub fn cuerpo(&self, parrafos: &[String], duracion_ms: i64) -> String {
        parrafos
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let ms = self
                    .tiempo(i)
                    .unwrap_or(duracion_ms)
                    .min(duracion_ms.max(0));
                format!("[{}] {p}", marca_de_tiempo(ms))
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn medidas(n: usize, alto: f32) -> Vec<Medida> {
        (0..n)
            .map(|i| Medida {
                cima: i as f32 * alto,
                alto,
            })
            .collect()
    }

    #[test]
    fn el_texto_se_parte_por_lineas_en_blanco_y_pierde_las_marcas_de_markdown() {
        let texto = "# Titulo\n\n- **uno** y `dos`\nsigue aqui\n\n> citado";
        assert_eq!(
            parrafos_de(texto),
            ["Titulo", "uno y dos sigue aqui", "citado"]
        );
    }

    #[test]
    fn un_texto_vacio_no_da_ningun_parrafo_que_leer() {
        assert!(parrafos_de("").is_empty());
        assert!(parrafos_de("   \n\n  \n").is_empty());
        // Un bloque que solo tenia una imagen tampoco: no hay nada que leer.
        assert!(parrafos_de("![foto](obra.png)").is_empty());
    }

    #[test]
    fn el_desfile_baja_a_la_velocidad_pedida_y_se_para_al_final() {
        let mut d = Desfile::nuevo(4);
        d.medir(medidas(4, 100.0), 200.0);
        // 400 de texto, 200 de ventana: se puede bajar 200.
        assert_eq!(d.tope(), 200.0);
        assert!(!d.avanzar(1.0, 50.0));
        assert_eq!(d.y, 50.0);
        assert!(d.avanzar(10.0, 50.0), "al llegar al final deja de bajar");
        assert_eq!(d.y, 200.0);
    }

    #[test]
    fn un_texto_mas_corto_que_la_ventana_no_se_mueve() {
        let mut d = Desfile::nuevo(1);
        d.medir(medidas(1, 50.0), 400.0);
        assert!(d.avanzar(5.0, 150.0));
        assert_eq!(d.y, 0.0);
    }

    #[test]
    fn cada_parrafo_se_queda_con_el_milisegundo_en_que_cruzo_la_linea() {
        let mut d = Desfile::nuevo(3);
        d.medir(medidas(3, 100.0), 200.0);
        // La linea de lectura esta a 200 * 0.3 = 60.
        assert!((d.linea_de_lectura() - 60.0).abs() < 0.001);
        d.apuntar(0);
        // Solo el primero (cima 0) esta por encima de 60.
        assert_eq!(
            (d.tiempo(0), d.tiempo(1), d.tiempo(2)),
            (Some(0), None, None)
        );
        d.empujar(50.0); // el segundo (cima 100) queda a 50: ya cruzo.
        d.apuntar(5_000);
        assert_eq!(d.tiempo(1), Some(5_000));
        assert_eq!(d.tiempo(2), None);
    }

    #[test]
    fn volver_a_subir_el_texto_no_le_quita_el_minuto_a_lo_ya_leido() {
        let mut d = Desfile::nuevo(2);
        d.medir(medidas(2, 100.0), 100.0);
        d.empujar(100.0);
        d.apuntar(4_000);
        assert_eq!(d.tiempo(1), Some(4_000));
        d.empujar(-100.0);
        d.apuntar(9_000);
        assert_eq!(
            d.tiempo(1),
            Some(4_000),
            "el minuto es el de la primera vez"
        );
    }

    #[test]
    fn reiniciar_devuelve_el_texto_arriba_y_borra_los_minutos() {
        let mut d = Desfile::nuevo(2);
        d.medir(medidas(2, 100.0), 100.0);
        d.empujar(50.0);
        d.apuntar(1_000);
        d.reiniciar();
        assert_eq!(d.y, 0.0);
        assert_eq!(d.tiempo(0), None);
    }

    #[test]
    fn el_cuerpo_pone_el_final_a_lo_que_no_llego_a_cruzar() {
        let mut d = Desfile::nuevo(2);
        d.medir(medidas(2, 100.0), 300.0);
        d.apuntar(2_000);
        let parrafos = vec!["primero".to_string(), "segundo".to_string()];
        assert_eq!(
            d.cuerpo(&parrafos, 61_000),
            "[0:02] primero\n\n[1:01] segundo"
        );
    }

    #[test]
    fn un_minuto_mas_alla_del_final_de_la_grabacion_se_recorta() {
        let mut d = Desfile::nuevo(1);
        d.medir(medidas(1, 100.0), 300.0);
        d.apuntar(99_000);
        assert_eq!(d.cuerpo(&["uno".to_string()], 5_000), "[0:05] uno");
    }

    #[test]
    fn solo_se_pinta_el_trozo_que_se_ve() {
        let mut d = Desfile::nuevo(10);
        d.medir(medidas(10, 100.0), 250.0);
        assert_eq!(d.visibles(), 0..3);
        d.empujar(400.0);
        assert_eq!(d.visibles(), 4..7);
        // Sin texto no se pinta nada, y el rango sigue siendo valido.
        let vacio = Desfile::nuevo(0);
        assert!(vacio.visibles().is_empty());
    }

    #[test]
    fn la_velocidad_se_acota_y_se_dice_con_palabras() {
        assert_eq!(acotar_velocidad(5.0), VELOCIDAD_MINIMA);
        assert_eq!(acotar_velocidad(500.0), VELOCIDAD_MAXIMA);
        assert_eq!(
            clave_de_la_velocidad(20.0),
            "telepronter-velocidad-muy-lenta"
        );
        assert_eq!(
            clave_de_la_velocidad(VELOCIDAD_POR_DEFECTO),
            "telepronter-velocidad-lenta"
        );
        assert_eq!(
            clave_de_la_velocidad(150.0),
            "telepronter-velocidad-muy-rapida"
        );
    }

    #[test]
    fn cambiar_de_texto_deja_los_minutos_a_cero_en_vez_de_mezclarlos() {
        let mut d = Desfile::nuevo(2);
        d.medir(medidas(2, 100.0), 300.0);
        d.apuntar(3_000);
        assert_eq!(d.tiempo(0), Some(3_000));
        d.medir(medidas(5, 100.0), 300.0);
        assert_eq!(d.tiempo(0), None);
    }
}
