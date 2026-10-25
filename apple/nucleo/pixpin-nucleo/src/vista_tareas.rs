//! **Las vistas de la ventana de Tareas para Swift** (Windows 5dab99a y
//! 5139e5d, 25-oct-2026): Tarjetas, Kanban y Horario, sin la Lista.
//!
//! El usuario (en Windows): «ahora quiero que la funcion de Horario aparezca
//! en la ventana de Tareas; de alli quita la vista de Lista, deja Tarjetas y
//! Kanban y anade Horario». Las cuentas son las de Windows
//! (`pixpin_ui::vista_tareas`, el mismo crate): aqui solo se pasan a tipos
//! que UniFFI entiende.
//!
//! - [`VistaTareas`]: las tres vistas, su palabra (la «lista» y las «por
//!   fecha» / «por lista» de antes del Mac se leen como Tarjetas) y su
//!   atajo.
//! - [`marco_de_vista_tareas`]: la ventana mas ancha en el Horario.
//! - [`disponer_horario_de_tareas`]: la lista de asignaturas y la semana.
//! - [`lunes_a_la_vista`], [`chips_de_la_semana`] y [`caja_de_chip`]: las
//!   tareas con hora (`⏰`) de la semana, como chapitas en su dia.

use pixpin_geom::Rect;
use pixpin_ia::fechas;
use pixpin_ui::vista_tareas as vt;

/// Como se ensenan las tareas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum VistaTareas {
    /// Cuadrados en rejilla, con la foto de fondo.
    Tarjetas,
    /// Una columna por lista.
    Kanban,
    /// La semana del horario de clases, con sus asignaturas al lado.
    Horario,
}

impl From<vt::Vista> for VistaTareas {
    fn from(v: vt::Vista) -> Self {
        match v {
            vt::Vista::Tarjetas => VistaTareas::Tarjetas,
            vt::Vista::Kanban => VistaTareas::Kanban,
            vt::Vista::Horario => VistaTareas::Horario,
        }
    }
}

impl From<VistaTareas> for vt::Vista {
    fn from(v: VistaTareas) -> Self {
        match v {
            VistaTareas::Tarjetas => vt::Vista::Tarjetas,
            VistaTareas::Kanban => vt::Vista::Kanban,
            VistaTareas::Horario => vt::Vista::Horario,
        }
    }
}

/// La vista recordada: `tarjetas`, `kanban` u `horario` (como el
/// `tareas-vista.txt` de Windows). Lo demas (la «lista» quitada, las «Por
/// fecha» y «Por lista» que el Mac guardaba antes) son las tarjetas.
#[uniffi::export]
pub fn vista_tareas_de_texto(texto: String) -> VistaTareas {
    vt::Vista::de_texto(&texto).into()
}

#[uniffi::export]
pub fn vista_tareas_como_texto(vista: VistaTareas) -> String {
    vt::Vista::from(vista).como_texto().to_string()
}

/// La letra de su atajo: T, K y H (en Windows con Ctrl; en el Mac con ⌃⌘,
/// porque ⌘H oculta la app y ⌥⌘H oculta las demas).
#[uniffi::export]
pub fn vista_tareas_atajo(vista: VistaTareas) -> String {
    vt::Vista::from(vista).atajo().to_string()
}

/// La vista de una letra pulsada con el atajo, si es la de alguna.
#[uniffi::export]
pub fn vista_tareas_de_atajo(letra: String) -> Option<VistaTareas> {
    let mut c = letra.chars();
    match (c.next(), c.next()) {
        (Some(l), None) => vt::Vista::de_atajo(l).map(Into::into),
        _ => None,
    }
}

/// El ancho de la ventana en esta vista, en puntos.
#[uniffi::export]
pub fn ancho_de_vista_tareas(vista: VistaTareas) -> u32 {
    vt::Vista::from(vista).ancho()
}

/// Un marco de ventana en puntos de pantalla. Solo cambian `x` y `ancho`,
/// asi que da igual que la `y` crezca hacia arriba (AppKit) o hacia abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct MarcoVentanaTareas {
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
}

impl From<MarcoVentanaTareas> for Rect {
    fn from(m: MarcoVentanaTareas) -> Self {
        Rect { x: m.x, y: m.y, ancho: m.ancho, alto: m.alto }
    }
}

impl From<Rect> for MarcoVentanaTareas {
    fn from(r: Rect) -> Self {
        MarcoVentanaTareas { x: r.x, y: r.y, ancho: r.ancho, alto: r.alto }
    }
}

/// **La ventana para otra vista**: el ancho de la vista, con el mismo
/// centro y el mismo alto, metida en `trabajo` (lo visible de la pantalla).
#[uniffi::export]
pub fn marco_de_vista_tareas(
    actual: MarcoVentanaTareas,
    trabajo: MarcoVentanaTareas,
    vista: VistaTareas,
) -> MarcoVentanaTareas {
    vt::marco_para(actual.into(), trabajo.into(), vt::Vista::from(vista).ancho(), 100).into()
}

/// Una caja en puntos de la ventana (la `y` hacia abajo, como SwiftUI).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CajaVistaTareas {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl From<vt::Caja> for CajaVistaTareas {
    fn from(c: vt::Caja) -> Self {
        CajaVistaTareas { x: c.x, y: c.y, ancho: c.ancho, alto: c.alto }
    }
}

/// Donde va cada parte del horario.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct DisposicionHorario {
    /// La lista de asignaturas (y debajo, las tareas de la elegida).
    pub lateral: CajaVistaTareas,
    /// Las franjas de la semana, sin la columna de las horas (a su
    /// izquierda) ni la fila de los dias (encima).
    pub semana: CajaVistaTareas,
    /// El ancho de la columna de las horas y el alto de la fila de los
    /// dias, ya con la escala.
    pub columna_horas: f32,
    pub cabecera_dias: f32,
}

/// **El horario en `area`** (lo de debajo de la barra de arriba), con las
/// medidas de Windows.
#[uniffi::export]
pub fn disponer_horario_de_tareas(area: CajaVistaTareas, escala: f32) -> DisposicionHorario {
    let m = vt::MedidasHorario::default();
    let d = vt::disponer_horario(
        vt::Caja { x: area.x, y: area.y, ancho: area.ancho, alto: area.alto },
        escala,
        &m,
    );
    DisposicionHorario {
        lateral: d.lateral.into(),
        semana: d.semana.into(),
        columna_horas: m.columna_horas * escala,
        cabecera_dias: m.cabecera_dias * escala,
    }
}

/// **El lunes de la semana que se ve** (dias desde 1970) para el dia
/// `hoy` (sus ms locales). Sin la columna del domingo (`dias` 6), en
/// domingo se ve la semana que empieza manana: la pasada ya no sirve
/// (Windows, 18-oct).
#[uniffi::export]
pub fn lunes_a_la_vista(hoy_ms_locales: i64, dias: u8) -> i64 {
    let hoy = hoy_ms_locales.div_euclid(86_400_000);
    let semana = fechas::dia_de_la_semana(hoy);
    if semana >= i64::from(dias) { hoy + 7 - semana } else { hoy - semana }
}

/// Una tarea con hora colocada en la semana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ChipSemana {
    /// Su sitio en las horas que se pasaron.
    pub indice: u32,
    /// 0 lunes … 6 domingo.
    pub dia: u8,
    /// El minuto del dia donde se pinta (ya dentro de la rejilla).
    pub minuto: u32,
    /// 0 la primera de su hora, 1 la de debajo...
    pub carril: u8,
    /// Las de su hora que no caben, si es la ultima que cabe («+2»).
    pub mas: u32,
}

/// **Las tareas con hora de la semana que se ve**, como chapitas
/// (`pixpin_ui::vista_tareas::chips_de_la_semana`): `horas` en ms locales,
/// `lunes` en dias desde 1970, `dias` las columnas, `desde`/`hasta` los
/// minutos de la rejilla y `paso` lo que junta dos en la misma hora.
#[uniffi::export]
pub fn chips_de_la_semana(
    horas: Vec<i64>,
    lunes: i64,
    dias: u8,
    desde: u32,
    hasta: u32,
    paso: u32,
) -> Vec<ChipSemana> {
    vt::chips_de_la_semana(&horas, lunes, dias, desde, hasta, paso)
        .into_iter()
        .map(|c| ChipSemana {
            indice: c.indice as u32,
            dia: c.dia,
            minuto: c.minuto,
            carril: c.carril,
            mas: c.mas as u32,
        })
        .collect()
}

/// **La caja de una chapita** en la columna de su dia, a la altura `y` de
/// su minuto: pegada a la derecha (las clases se leen a la izquierda).
#[uniffi::export]
pub fn caja_de_chip(
    x_dia: f32,
    ancho_dia: f32,
    y: f32,
    carril: u8,
    alto: f32,
    escala: f32,
) -> CajaVistaTareas {
    vt::caja_de_chip(x_dia, ancho_dia, y, carril, alto, escala).into()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_vista_se_lee_de_su_palabra_y_lo_de_antes_del_mac_son_las_tarjetas() {
        for v in [VistaTareas::Tarjetas, VistaTareas::Kanban, VistaTareas::Horario] {
            assert_eq!(vista_tareas_de_texto(vista_tareas_como_texto(v)), v);
            assert_eq!(vista_tareas_de_atajo(vista_tareas_atajo(v)), Some(v));
        }
        // Lo que guardaba el Mac antes (AppStorage) y la lista de Windows.
        for viejo in ["Por fecha", "Por lista", "lista", ""] {
            assert_eq!(vista_tareas_de_texto(viejo.into()), VistaTareas::Tarjetas);
        }
        assert_eq!(vista_tareas_de_texto("Kanban".into()), VistaTareas::Tarjetas, "la palabra, no el rotulo");
        assert_eq!(vista_tareas_de_atajo("h".into()), Some(VistaTareas::Horario));
        // Casos negativos: la L ya no es de nadie, ni dos letras.
        assert_eq!(vista_tareas_de_atajo("L".into()), None);
        assert_eq!(vista_tareas_de_atajo("TK".into()), None);
        assert_eq!(ancho_de_vista_tareas(VistaTareas::Horario), 1180);
        assert_eq!(ancho_de_vista_tareas(VistaTareas::Kanban), 660);
    }

    #[test]
    fn el_horario_ensancha_la_ventana_con_el_mismo_centro() {
        let trabajo = MarcoVentanaTareas { x: 0, y: 25, ancho: 1440, alto: 875 };
        let tareas = MarcoVentanaTareas { x: 400, y: 100, ancho: 660, alto: 640 };
        let h = marco_de_vista_tareas(tareas, trabajo, VistaTareas::Horario);
        assert_eq!((h.ancho, h.alto, h.y), (1180, 640, 100));
        assert_eq!(h.x + h.ancho as i32 / 2, 730);
        assert_eq!(marco_de_vista_tareas(h, trabajo, VistaTareas::Tarjetas), tareas);
        // Una pantalla estrecha: todo lo que hay.
        let chica = MarcoVentanaTareas { x: 0, y: 0, ancho: 1024, alto: 700 };
        assert_eq!(marco_de_vista_tareas(tareas, chica, VistaTareas::Horario).ancho, 1024);
    }

    #[test]
    fn la_disposicion_deja_sitio_a_las_horas_y_a_los_dias() {
        let d = disponer_horario_de_tareas(
            CajaVistaTareas { x: 0.0, y: 64.0, ancho: 1180.0, alto: 700.0 },
            1.0,
        );
        assert_eq!(d.lateral.ancho, 280.0);
        assert_eq!(d.semana.x - d.columna_horas, 16.0 + 280.0 + 16.0);
        assert_eq!(d.semana.y - d.cabecera_dias, 64.0);
    }

    #[test]
    fn el_lunes_que_se_ve_y_el_domingo_sin_su_columna() {
        // 2026-10-21 fue miercoles; 2026-10-19 su lunes.
        let lunes = fechas::dias_de(2026, 10, 19);
        let miercoles = (lunes + 2) * 86_400_000 + 9 * 3_600_000;
        assert_eq!(lunes_a_la_vista(miercoles, 6), lunes);
        let domingo = (lunes + 6) * 86_400_000 + 20 * 3_600_000;
        assert_eq!(lunes_a_la_vista(domingo, 7), lunes, "con domingo, la de hoy");
        assert_eq!(lunes_a_la_vista(domingo, 6), lunes + 7, "sin el, la que empieza manana");
    }

    #[test]
    fn las_chapitas_pasan_por_el_puente_con_su_cuenta() {
        let lunes = 20_000_i64;
        let a_las_diez = lunes * 86_400_000 + 10 * 3_600_000;
        let horas: Vec<i64> = (0..5).map(|k| a_las_diez + k * 60_000).collect();
        let c = chips_de_la_semana(horas, lunes, 6, 7 * 60, 22 * 60, 30);
        assert_eq!(c.len(), 3);
        assert_eq!(c.last().map(|x| (x.carril, x.mas)), Some((2, 2)));
        // Negativo: lo de la semana siguiente no sale.
        assert!(chips_de_la_semana(vec![a_las_diez + 7 * 86_400_000], lunes, 6, 420, 1320, 30).is_empty());
        let k = caja_de_chip(100.0, 150.0, 300.0, 1, 18.0, 1.0);
        assert!(k.x > 100.0 && k.x + k.ancho <= 250.0 && k.y > 300.0 + 18.0);
    }
}
