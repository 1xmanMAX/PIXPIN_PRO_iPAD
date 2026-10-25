//! **Las vistas de la ventana de Tareas**: Tarjetas, Kanban y Horario
//! (25-oct-2026), las cuentas sin pintar nada.
//!
//! El usuario: «ahora quiero que la funcion de Horario aparezca en la
//! ventana de Tareas; de alli quita la vista de Lista, deja Tarjetas y
//! Kanban y anade Horario».
//!
//! - [`Vista`]: las tres vistas del conmutador, como se recuerdan en
//!   `tareas-vista.txt` (la «lista» de antes se lee como Tarjetas) y su
//!   atajo de teclado.
//! - [`marco_para`]: la ventana mas ancha en el Horario (la semana necesita
//!   sitio) y la de siempre en las otras dos, sin salirse de la pantalla.
//! - [`disponer_horario`]: donde van la lista de asignaturas y la semana
//!   dentro de la ventana, debajo de la barra comun.
//! - [`chips_de_la_semana`] y [`caja_de_chip`]: las tareas con hora
//!   (`⏰`) de esta semana, como chapitas en su dia y su hora.

use pixpin_geom::Rect;

// ------------------------------------------------------------------- vista

/// Como se ensenan las tareas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vista {
    /// Cuadrados en rejilla, con la foto de fondo (tareas-v4).
    #[default]
    Tarjetas,
    /// Una columna por lista; arrastrar una tarjeta a otra la pasa alli
    /// (17-oct-2026).
    Kanban,
    /// La semana del horario de clases, con sus asignaturas al lado
    /// (25-oct-2026).
    Horario,
}

impl Vista {
    /// Las vistas en el orden del conmutador.
    pub const TODAS: [Vista; 3] = [Vista::Tarjetas, Vista::Kanban, Vista::Horario];

    /// Lo que no se entiende (un fichero roto, de otra version) y la
    /// «lista» que ya no existe son las tarjetas.
    pub fn de_texto(s: &str) -> Vista {
        match s.trim() {
            "kanban" => Vista::Kanban,
            "horario" => Vista::Horario,
            _ => Vista::Tarjetas,
        }
    }

    pub fn como_texto(self) -> &'static str {
        match self {
            Vista::Tarjetas => "tarjetas",
            Vista::Kanban => "kanban",
            Vista::Horario => "horario",
        }
    }

    /// Si las tarjetas son las de la vista en cuadrados (con su foto de
    /// fondo): en la rejilla y en el tablero Kanban, que usa las mismas.
    pub fn con_cuadrados(self) -> bool {
        matches!(self, Vista::Tarjetas | Vista::Kanban)
    }

    /// Si la vista ensena tareas (y con ellas el buscador y la caja de
    /// apuntar). El horario ensena la semana.
    pub fn con_tareas(self) -> bool {
        self != Vista::Horario
    }

    /// La letra de su atajo, con Ctrl: T, K y H.
    pub fn atajo(self) -> char {
        match self {
            Vista::Tarjetas => 'T',
            Vista::Kanban => 'K',
            Vista::Horario => 'H',
        }
    }

    /// La vista de una letra pulsada con Ctrl, si es la de alguna.
    pub fn de_atajo(c: char) -> Option<Vista> {
        let c = c.to_ascii_uppercase();
        Vista::TODAS.into_iter().find(|v| v.atajo() == c)
    }

    /// El ancho de la ventana en esta vista, en pixeles logicos.
    pub fn ancho(self) -> u32 {
        match self {
            Vista::Horario => ANCHO_HORARIO,
            _ => ANCHO_TAREAS,
        }
    }
}

/// El ancho de la ventana de Tareas en Tarjetas y Kanban (logicos).
pub const ANCHO_TAREAS: u32 = 660;
/// Y en el Horario: lo que media la ventana del horario de antes.
pub const ANCHO_HORARIO: u32 = 1180;

/// **La ventana para otra vista**: `ancho` logicos (con la escala), con el
/// mismo centro que tenia y el mismo alto, metida en `trabajo` (el area de
/// trabajo del monitor). Si no cabe, todo el ancho que haya.
pub fn marco_para(actual: Rect, trabajo: Rect, ancho: u32, escala_por_cien: u32) -> Rect {
    let w = (ancho * escala_por_cien / 100).min(trabajo.ancho).max(1);
    let centro = actual.x + actual.ancho as i32 / 2;
    let max_x = trabajo.x + trabajo.ancho as i32 - w as i32;
    let x = (centro - w as i32 / 2).clamp(trabajo.x, max_x.max(trabajo.x));
    Rect {
        x,
        ancho: w,
        ..actual
    }
}

// ----------------------------------------------------------------- horario

/// Una caja en pixeles de ventana.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Caja {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl Caja {
    pub fn contiene(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.ancho && y < self.y + self.alto
    }
}

/// Las medidas del horario, en logicos (sin la escala).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MedidasHorario {
    pub margen: f32,
    /// El ancho que se quiere para la lista de asignaturas.
    pub lateral: f32,
    /// Lo menos que se le deja, en una ventana estrecha.
    pub lateral_min: f32,
    pub columna_horas: f32,
    pub cabecera_dias: f32,
}

impl Default for MedidasHorario {
    fn default() -> Self {
        MedidasHorario {
            margen: 16.0,
            lateral: 280.0,
            lateral_min: 200.0,
            columna_horas: 48.0,
            cabecera_dias: 36.0,
        }
    }
}

/// Donde va cada parte del horario.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Disposicion {
    /// La lista de asignaturas (y debajo, las tareas de la elegida).
    pub lateral: Caja,
    /// Las franjas de la semana, sin la columna de las horas ni la fila de
    /// los dias (que quedan a su izquierda y encima).
    pub semana: Caja,
}

/// **El horario en `area`** (lo de debajo de la barra de arriba), con la
/// escala `s`. La lista de asignaturas mide un cuarto del ancho, entre
/// `lateral_min` y `lateral`: en una ventana estrecha encoge. La semana
/// nunca baja de 60 px por lado (una ventana diminuta no da cajas negativas).
pub fn disponer_horario(area: Caja, s: f32, m: &MedidasHorario) -> Disposicion {
    let margen = m.margen * s;
    let lateral = (area.ancho / 4.0).clamp(m.lateral_min * s, m.lateral * s);
    let x0 = area.x + margen + lateral + margen + m.columna_horas * s;
    let y0 = area.y + m.cabecera_dias * s;
    let ancho = (area.x + area.ancho - margen - x0).max(60.0);
    let alto = (area.y + area.alto - margen - y0).max(60.0);
    Disposicion {
        lateral: Caja {
            x: area.x + margen,
            y: area.y,
            ancho: lateral,
            alto: (area.alto - margen).max(0.0),
        },
        semana: Caja {
            x: x0,
            y: y0,
            ancho,
            alto,
        },
    }
}

// ------------------------------------------------------- tareas en la semana

/// Cuantas chapitas de tarea caben una debajo de otra en la misma hora; las
/// de mas se cuentan en la ultima («+2»).
pub const CARRILES: u8 = 3;

/// Una tarea con hora, colocada en la semana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chip {
    /// Su sitio en lo que se paso a [`chips_de_la_semana`].
    pub indice: usize,
    /// 0 lunes … 6 domingo.
    pub dia: u8,
    /// El minuto del dia donde se pinta (ya dentro de la semana que se ve).
    pub minuto: u32,
    /// 0 la primera de su hora, 1 la de debajo...
    pub carril: u8,
    /// Las de su hora que no caben, si es la ultima que cabe.
    pub mas: usize,
}

/// **Las tareas con hora de la semana que se ve**, como chapitas. `horas`
/// son las horas locales (ms desde 1970) de cada tarea; `lunes`, el lunes
/// de la semana (dias desde 1970); `dias`, cuantos se ven; `desde`/`hasta`,
/// los minutos del dia de la rejilla. Lo de antes o despues de la rejilla se
/// pinta en su borde (una tarea a las 6:30 no desaparece). Dos tareas a
/// menos de `paso` minutos en el mismo dia van una debajo de otra; mas de
/// [`CARRILES`], la ultima lleva la cuenta de las que faltan.
pub fn chips_de_la_semana(
    horas: &[i64],
    lunes: i64,
    dias: u8,
    desde: u32,
    hasta: u32,
    paso: u32,
) -> Vec<Chip> {
    const DIA_MS: i64 = 86_400_000;
    let mut puestas: Vec<(usize, u8, u32)> = horas
        .iter()
        .enumerate()
        .filter_map(|(i, &ms)| {
            let dia = ms.div_euclid(DIA_MS) - lunes;
            if dia < 0 || dia >= i64::from(dias) {
                return None;
            }
            let minuto = (ms.rem_euclid(DIA_MS) / 60_000) as u32;
            let minuto = minuto.clamp(desde, hasta.saturating_sub(paso).max(desde));
            Some((i, dia as u8, minuto))
        })
        .collect();
    puestas.sort_by_key(|&(i, dia, minuto)| (dia, minuto, i));
    let mut chips: Vec<Chip> = Vec::new();
    for (indice, dia, minuto) in puestas {
        // La primera de su grupo: la de su dia de la que esta a menos de
        // `paso`, si la hay.
        let cabeza = chips
            .iter()
            .rposition(|c| c.dia == dia && c.carril == 0 && minuto < c.minuto + paso);
        let carril = match cabeza {
            Some(k) => chips[k..].iter().filter(|c| c.dia == dia).count() as u8,
            None => 0,
        };
        if carril >= CARRILES {
            if let Some(ultima) = chips.iter_mut().rev().find(|c| c.dia == dia) {
                ultima.mas += 1;
            }
            continue;
        }
        let minuto = cabeza.map_or(minuto, |k| chips[k].minuto);
        chips.push(Chip {
            indice,
            dia,
            minuto,
            carril,
            mas: 0,
        });
    }
    chips
}

/// **La caja de una chapita** en la columna de su dia (`x_dia`, `ancho_dia`)
/// a la altura `y` de su minuto: pegada a la derecha de la columna (las
/// clases se siguen leyendo a la izquierda), de `alto`, y cada carril debajo
/// del anterior. Como mucho, dos tercios del dia de ancho.
pub fn caja_de_chip(x_dia: f32, ancho_dia: f32, y: f32, carril: u8, alto: f32, s: f32) -> Caja {
    let aire = 3.0 * s;
    let ancho = (ancho_dia * 2.0 / 3.0).max(1.0);
    Caja {
        x: x_dia + ancho_dia - ancho - aire,
        y: y + 1.0 * s + f32::from(carril) * (alto + 2.0 * s),
        ancho: (ancho - aire).max(1.0),
        alto,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_lista_de_antes_se_lee_como_tarjetas_y_cada_vista_vuelve_a_su_texto() {
        for v in Vista::TODAS {
            assert_eq!(Vista::de_texto(v.como_texto()), v);
            assert_eq!(Vista::de_atajo(v.atajo()), Some(v));
        }
        assert_eq!(Vista::de_texto("lista"), Vista::Tarjetas, "la que se quito");
        assert_eq!(Vista::de_texto(" horario\n"), Vista::Horario);
        assert_eq!(Vista::de_texto("¿?"), Vista::Tarjetas);
        assert_eq!(Vista::default(), Vista::Tarjetas);
        // Caso negativo: la L ya no es de ninguna vista.
        assert_eq!(Vista::de_atajo('L'), None);
        assert_eq!(Vista::de_atajo('h'), Some(Vista::Horario));
        assert!(Vista::Kanban.con_cuadrados() && !Vista::Horario.con_cuadrados());
        assert!(!Vista::Horario.con_tareas() && Vista::Tarjetas.con_tareas());
    }

    #[test]
    fn el_horario_ensancha_la_ventana_sin_salirse_de_la_pantalla() {
        let trabajo = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let tareas = Rect {
            x: 630,
            y: 120,
            ancho: 660,
            alto: 800,
        };
        let h = marco_para(tareas, trabajo, Vista::Horario.ancho(), 100);
        assert_eq!((h.ancho, h.alto, h.y), (1180, 800, 120));
        assert_eq!(h.x + h.ancho as i32 / 2, 960, "el mismo centro");
        // Y vuelta: el mismo marco de antes.
        assert_eq!(marco_para(h, trabajo, Vista::Tarjetas.ancho(), 100), tareas);
        // Pegada al borde izquierdo, crece hacia dentro.
        let izq = marco_para(Rect { x: 0, ..tareas }, trabajo, 1180, 100);
        assert_eq!(izq.x, 0);
        // Con escala 150 en una pantalla pequena: todo el ancho que hay.
        let chica = Rect {
            x: 100,
            y: 0,
            ancho: 1366,
            alto: 728,
        };
        let r = marco_para(tareas, chica, 1180, 150);
        assert_eq!((r.x, r.ancho), (100, 1366));
    }

    #[test]
    fn el_horario_reparte_la_lateral_y_la_semana() {
        let m = MedidasHorario::default();
        let area = Caja {
            x: 0.0,
            y: 76.0,
            ancho: 1180.0,
            alto: 704.0,
        };
        let d = disponer_horario(area, 1.0, &m);
        assert_eq!(d.lateral.ancho, 280.0);
        assert_eq!(d.lateral.x, 16.0);
        assert_eq!(d.semana.x, 16.0 + 280.0 + 16.0 + 48.0);
        assert_eq!(d.semana.y, 76.0 + 36.0);
        assert_eq!(
            d.semana.x + d.semana.ancho,
            1180.0 - 16.0,
            "hasta el margen"
        );
        assert!(!d.lateral.contiene((d.semana.x + 1.0, d.semana.y + 1.0)));
        // Estrecha: la lateral encoge, pero no por debajo de su minimo.
        let estrecha = disponer_horario(
            Caja {
                ancho: 660.0,
                ..area
            },
            1.0,
            &m,
        );
        assert!(estrecha.lateral.ancho < 280.0 && estrecha.lateral.ancho >= 200.0);
        // Diminuta: nada negativo.
        let mini = disponer_horario(
            Caja {
                ancho: 100.0,
                alto: 50.0,
                ..area
            },
            1.0,
            &m,
        );
        assert!(mini.semana.ancho >= 60.0 && mini.semana.alto >= 60.0);
        // Con escala 2, todo al doble.
        let doble = disponer_horario(
            Caja {
                ancho: 2360.0,
                alto: 1408.0,
                ..area
            },
            2.0,
            &m,
        );
        assert_eq!(doble.lateral.ancho, 560.0);
    }

    const DIA: i64 = 86_400_000;
    const HORA: i64 = 3_600_000;

    #[test]
    fn las_tareas_con_hora_van_a_su_dia_y_se_apilan_en_la_misma_hora() {
        let lunes = 20_000;
        let base = lunes * DIA;
        let horas = [
            base + 9 * HORA,                   // lunes 9:00
            base + 2 * DIA + 15 * HORA,        // miercoles 15:00
            base + 9 * HORA + 10 * 60_000,     // lunes 9:10: debajo de la primera
            base - DIA + 9 * HORA,             // el domingo de antes: fuera
            base + 7 * DIA,                    // el lunes siguiente: fuera
            base + 4 * DIA + 6 * HORA,         // viernes 6:00: al borde de arriba
            base + 6 * DIA + 10 * HORA,        // domingo: solo si se ve
            base + 2 * DIA + 15 * HORA + 1000, // miercoles 15:00 otra vez
        ];
        let c = chips_de_la_semana(&horas, lunes, 6, 7 * 60, 22 * 60, 30);
        let de = |i: usize| c.iter().find(|x| x.indice == i).copied();
        assert_eq!(
            de(0).map(|x| (x.dia, x.minuto, x.carril)),
            Some((0, 540, 0))
        );
        assert_eq!(
            de(2).map(|x| (x.dia, x.minuto, x.carril)),
            Some((0, 540, 1)),
            "a la hora de la primera, debajo"
        );
        assert_eq!(de(1).map(|x| (x.dia, x.carril)), Some((2, 0)));
        assert_eq!(de(7).map(|x| (x.dia, x.carril)), Some((2, 1)));
        assert_eq!(de(5).map(|x| x.minuto), Some(7 * 60), "al borde");
        // Casos negativos: lo de otra semana y el domingo sin su columna.
        assert_eq!(de(3), None);
        assert_eq!(de(4), None);
        assert_eq!(de(6), None);
        let con_domingo = chips_de_la_semana(&horas, lunes, 7, 7 * 60, 22 * 60, 30);
        assert!(con_domingo.iter().any(|x| x.indice == 6 && x.dia == 6));
    }

    #[test]
    fn mas_tareas_de_las_que_caben_en_una_hora_se_cuentan() {
        let lunes = 20_000;
        let a_las_diez = lunes * DIA + 10 * HORA;
        let horas: Vec<i64> = (0..5).map(|k| a_las_diez + k * 60_000).collect();
        let c = chips_de_la_semana(&horas, lunes, 6, 7 * 60, 22 * 60, 30);
        assert_eq!(c.len(), usize::from(CARRILES));
        assert_eq!(c.last().map(|x| x.mas), Some(2));
        assert!(c.iter().take(2).all(|x| x.mas == 0));
        // Una hora despues ya es otro grupo: vuelve al primer carril.
        let mut horas = horas;
        horas.push(a_las_diez + HORA);
        let c = chips_de_la_semana(&horas, lunes, 6, 7 * 60, 22 * 60, 30);
        assert_eq!(c.last().map(|x| (x.indice, x.carril)), Some((5, 0)));
    }

    #[test]
    fn la_chapita_va_a_la_derecha_de_su_dia_y_cada_carril_debajo() {
        let a = caja_de_chip(100.0, 150.0, 300.0, 0, 18.0, 1.0);
        let b = caja_de_chip(100.0, 150.0, 300.0, 1, 18.0, 1.0);
        assert!(a.x > 100.0 && a.x + a.ancho <= 250.0, "{a:?}");
        assert!(a.ancho <= 100.0);
        assert_eq!(b.x, a.x);
        assert!(b.y >= a.y + a.alto, "sin pisarse");
    }
}
