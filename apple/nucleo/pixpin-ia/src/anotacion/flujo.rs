//! **Las anotaciones segun llegan**: el modelo escribe un objeto JSON por
//! linea y aqui se suelta cada uno en cuanto se cierra su ultima llave, sin
//! esperar al resto. No se fia de los saltos de linea (un modelo parte un
//! objeto en dos lineas o los mete en un bloque ```json): cuenta llaves
//! fuera de las comillas, que es lo que hace un objeto completo.

use serde_json::Value;

/// Lo que va llegando y aun no forma un objeto.
#[derive(Debug, Default)]
pub struct Flujo {
    pendiente: String,
    /// Donde empieza el objeto abierto (en `pendiente`), si hay uno.
    inicio: Option<usize>,
    hondo: usize,
    en_comillas: bool,
    escapado: bool,
    /// Hasta donde ya se miro.
    mirado: usize,
    /// Objetos que no eran JSON (para contarlo).
    pub rotos: usize,
}

impl Flujo {
    pub fn nuevo() -> Flujo {
        Flujo::default()
    }

    /// Mete un trozo y devuelve los objetos que se cerraron con el.
    pub fn empujar(&mut self, trozo: &str) -> Vec<Value> {
        self.pendiente.push_str(trozo);
        let mut salen = Vec::new();
        let bytes = self.pendiente.as_bytes();
        let mut i = self.mirado;
        while i < bytes.len() {
            let c = bytes[i];
            if self.en_comillas {
                if self.escapado {
                    self.escapado = false;
                } else if c == b'\\' {
                    self.escapado = true;
                } else if c == b'"' {
                    self.en_comillas = false;
                }
            } else if c == b'"' {
                if self.inicio.is_some() {
                    self.en_comillas = true;
                }
            } else if c == b'{' {
                if self.inicio.is_none() {
                    self.inicio = Some(i);
                    self.hondo = 0;
                }
                self.hondo += 1;
            } else if c == b'}' && self.inicio.is_some() {
                self.hondo -= 1;
                if self.hondo == 0 {
                    let desde = self.inicio.take().unwrap_or(0);
                    match serde_json::from_str::<Value>(&self.pendiente[desde..=i]) {
                        Ok(v) if v.is_object() => salen.push(v),
                        _ => self.rotos += 1,
                    }
                }
            }
            i += 1;
        }
        // Lo ya cerrado se tira; lo abierto se queda.
        match self.inicio {
            Some(d) => {
                self.pendiente.drain(..d);
                self.inicio = Some(0);
                self.mirado = self.pendiente.len();
            }
            None => {
                self.pendiente.clear();
                self.mirado = 0;
            }
        }
        salen
    }
}
