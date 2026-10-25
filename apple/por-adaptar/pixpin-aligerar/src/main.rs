//! `pixpin-aligerar.exe`: ver la cabecera de `lib.rs`.
//!
//! Subsistema de consola a proposito: `pixpinmax` lo lanza con
//! `CREATE_NO_WINDOW`, asi que no se ve ninguna ventana, y la salida estandar
//! funciona igual lanzado a mano para medir.

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    std::process::exit(pixpin_aligerar::ejecutar(&args));
}
