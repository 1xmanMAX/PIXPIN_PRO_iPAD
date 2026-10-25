//! **Donde va cada nota del margen**, sin que dos se pisen.
//!
//! Las notas van en **franjas**: los margenes de la hoja con sitio para una
//! nota legible y, si se puede salir de ella, el espacio de fuera (el de
//! anotar del lector, el lienzo alrededor de una foto o de un plano). Cada
//! franja lleva la cuenta de los tramos de alto ya ocupados.
//!
//! Las notas llegan **de una en una** (en vivo, segun las escribe el
//! modelo), asi que no se puede ordenar todo y repartir: cada una busca, en
//! cada franja, el hueco libre mas cercano a la altura de lo que senala
//! (justo ahi, o pegada debajo o encima de una que ya esta), y se queda con
//! el que menos la aleja. Salir de la hoja cuesta un poco (mejor dentro si
//! hay sitio) y el lado contrario a lo senalado tambien, asi que con un
//! margen lleno se pasa al otro lado: se alternan solas. Si no cabe en
//! ninguna, no se pone (y se dice): una nota encima de otra no se lee.

/// El ancho minimo de una franja, en letras de alto: unas 18 letras por
/// renglon (por debajo, una explicacion de diez palabras ocupa media hoja).
pub const LETRAS_MINIMAS: f32 = 9.0;

/// Lo que cuesta salir de la hoja, en letras (de alto) de desplazamiento.
const COSTE_FUERA: f32 = 12.0;
/// Lo que cuesta ir al lado contrario de lo senalado.
const COSTE_OTRO_LADO: f32 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Izquierda,
    Derecha,
}

/// Una franja vertical donde caben notas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Franja {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    pub lado: Lado,
    /// Fuera de la hoja (en el espacio para anotar).
    pub fuera: bool,
}

/// Donde quedo una nota.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colocada {
    pub franja: Franja,
    pub caja: [f32; 4],
}

/// Las franjas de una pagina y lo ya ocupado en cada una.
#[derive(Debug, Clone)]
pub struct Colocador {
    franjas: Vec<Franja>,
    ocupado: Vec<Vec<[f32; 2]>>,
    hueco: f32,
}

impl Colocador {
    /// `hueco`: el aire entre dos notas.
    pub fn nuevo(franjas: Vec<Franja>, hueco: f32) -> Colocador {
        let ocupado = vec![Vec::new(); franjas.len()];
        Colocador {
            franjas,
            ocupado,
            hueco: hueco.max(0.0),
        }
    }

    pub fn franjas(&self) -> &[Franja] {
        &self.franjas
    }

    /// Lo que ya hay en `caja` (texto al margen, otra nota) no se tapa.
    pub fn ocupar(&mut self, caja: [f32; 4]) {
        for (f, o) in self.franjas.iter().zip(self.ocupado.iter_mut()) {
            if caja[2] > f.x0 && caja[0] < f.x1 {
                o.push([caja[1], caja[3]]);
            }
        }
    }

    /// El hueco libre de alto `alto` mas cercano a `deseado` en la franja
    /// `i`, si lo hay.
    fn hueco_en(&self, i: usize, deseado: f32, alto: f32) -> Option<f32> {
        let f = &self.franjas[i];
        let (min, max) = (f.y0, f.y1 - alto);
        if max < min {
            return None;
        }
        let g = self.hueco;
        let libre = |y: f32| {
            y >= min - 1e-3
                && y <= max + 1e-3
                && self.ocupado[i]
                    .iter()
                    .all(|t| y + alto + g <= t[0] || y >= t[1] + g)
        };
        let mut candidatos = vec![deseado.clamp(min, max)];
        for t in &self.ocupado[i] {
            candidatos.push(t[1] + g);
            candidatos.push(t[0] - g - alto);
        }
        candidatos
            .into_iter()
            .filter(|y| libre(*y))
            .min_by(|a, b| (a - deseado).abs().total_cmp(&(b - deseado).abs()))
    }

    /// **Coloca una nota** para lo senalado en `ancla`: `alto_con(ancho)`
    /// dice cuanto mide de alto la nota partida a ese ancho, y no se hace
    /// mas ancha que `ancho_maximo`. Queda apuntada como ocupada.
    pub fn colocar(
        &mut self,
        ancla: [f32; 4],
        alto_con: &dyn Fn(f32) -> f32,
        ancho_maximo: f32,
    ) -> Option<Colocada> {
        let letra = (self.hueco * 2.0).max(1.0);
        let centro_x = (ancla[0] + ancla[2]) / 2.0;
        // El lado de lo senalado: el de la mitad de la zona de franjas en
        // que cae.
        let (izq, der) = self
            .franjas
            .iter()
            .fold((f32::MAX, f32::MIN), |a, f| (a.0.min(f.x1), a.1.max(f.x0)));
        let lado_natural = if centro_x < (izq + der) / 2.0 {
            Lado::Izquierda
        } else {
            Lado::Derecha
        };
        let mut mejor: Option<(f32, usize, f32, f32, f32)> = None;
        for (i, f) in self.franjas.iter().enumerate() {
            let ancho = (f.x1 - f.x0).min(ancho_maximo);
            if ancho <= 0.0 {
                continue;
            }
            let alto = alto_con(ancho);
            // Arriba a la altura de lo senalado.
            let deseado = ancla[1] - self.hueco * 0.5;
            let Some(y) = self.hueco_en(i, deseado, alto) else {
                continue;
            };
            let mut coste = (y - deseado).abs();
            if f.fuera {
                coste += COSTE_FUERA * letra;
            }
            if f.lado != lado_natural {
                coste += COSTE_OTRO_LADO * letra;
            }
            if mejor.is_none_or(|m| coste < m.0) {
                mejor = Some((coste, i, y, ancho, alto));
            }
        }
        let (_, i, y, ancho, alto) = mejor?;
        let f = self.franjas[i];
        // Pegada al texto: a la derecha empieza en el borde de dentro; a la
        // izquierda acaba en el.
        let x0 = match f.lado {
            Lado::Derecha => f.x0,
            Lado::Izquierda => f.x1 - ancho,
        };
        self.ocupado[i].push([y, y + alto]);
        Some(Colocada {
            franja: f,
            caja: [x0, y, x0 + ancho, y + alto],
        })
    }
}
