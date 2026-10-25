//! El generador de enlaces de UniFFI, como lo recomienda su documentacion:
//! un binario dentro del propio crate para que la version del generador sea
//! SIEMPRE la de la biblioteca (un generador de otra version produce Swift
//! que no casa con los simbolos y falla al cargar, no al compilar).
//!
//! ```sh
//! cargo run -p pixpin-nucleo --features bindgen --bin uniffi-bindgen -- \
//!     generate --library target/debug/libpixpin_nucleo.so --language swift --out-dir salida
//! ```

fn main() {
    uniffi::uniffi_bindgen_main()
}
