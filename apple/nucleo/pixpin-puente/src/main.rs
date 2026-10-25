//! `pixpin-puente.exe`: ver la cabecera de `lib.rs`.
//!
//! Subsistema de Windows y no de consola: el navegador lo arranca con las
//! tuberias redirigidas, y una consola saldria parpadeando en cada clic.

use std::path::PathBuf;

fn main() {
    let dir_exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from));
    // Solo para las pruebas: no manda nada y usa esa carpeta como raiz.
    let en_seco = std::env::var_os("PIXPIN_PUENTE_EN_SECO").map(PathBuf::from);
    let Some(mut entorno) = pixpin_puente::Entorno::del_equipo(dir_exe.as_deref(), en_seco.clone())
    else {
        std::process::exit(1);
    };
    let mut entrada = std::io::stdin().lock();
    let mut salida = std::io::stdout().lock();
    let codigo = if en_seco.is_some() {
        entorno.ensenar_pedido = true;
        pixpin_puente::correr(
            &mut entrada,
            &mut salida,
            &entorno,
            &mut pixpin_puente::EnSeco,
        )
    } else {
        let mut cartero = pixpin_puente::AlaApp {
            exe: pixpin_puente::exe_de_pixpin(dir_exe.as_deref()),
        };
        pixpin_puente::correr(&mut entrada, &mut salida, &entorno, &mut cartero)
    };
    std::process::exit(codigo);
}
