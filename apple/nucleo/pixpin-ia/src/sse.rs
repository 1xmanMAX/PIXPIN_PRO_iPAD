//! **Leer la respuesta en vivo** (Server-Sent Events al estilo OpenAI).
//!
//! `llama-server` manda lineas `data: {json}` con trocitos (`delta`) de
//! texto, de razonamiento y de llamadas a herramientas; las llamadas llegan
//! por pedazos (`index` + nombre una vez + argumentos poco a poco) y hay que
//! juntarlas. Al final, `data: [DONE]`. Esta pieza no sabe nada de sockets:
//! se le dan bytes y devuelve trozos, para poder probarla sin red.

use serde_json::Value;

use crate::ErrorIa;
use crate::motor::{Fin, LlamadaCruda, Medidas, Respuesta, Trozo};

#[derive(Default)]
pub struct Acumulador {
    /// Bytes recibidos que aun no forman una linea completa.
    pendiente: Vec<u8>,
    texto: String,
    pensamiento: String,
    llamadas: Vec<LlamadaCruda>,
    fin: Option<Fin>,
    medidas: Medidas,
    acabado: bool,
}

impl Acumulador {
    pub fn nuevo() -> Acumulador {
        Acumulador::default()
    }

    pub fn acabado(&self) -> bool {
        self.acabado
    }

    /// Mete bytes y devuelve los trozos nuevos que salen de ellos.
    pub fn meter(&mut self, bytes: &[u8]) -> Result<Vec<Trozo>, ErrorIa> {
        self.pendiente.extend_from_slice(bytes);
        let mut trozos = Vec::new();
        while let Some(fin) = self.pendiente.iter().position(|&b| b == b'\n') {
            let linea: Vec<u8> = self.pendiente.drain(..=fin).collect();
            let linea = String::from_utf8_lossy(&linea);
            let linea = linea.trim_end_matches(['\r', '\n']);
            if let Some(datos) = linea.strip_prefix("data:") {
                self.evento(datos.trim(), &mut trozos)?;
            }
            // Las lineas vacias separan eventos; `:` son comentarios
            // (latidos); `event:` no lo usa llama-server. Todo se ignora.
        }
        Ok(trozos)
    }

    fn evento(&mut self, datos: &str, trozos: &mut Vec<Trozo>) -> Result<(), ErrorIa> {
        if datos == "[DONE]" {
            self.acabado = true;
            return Ok(());
        }
        let v: Value = serde_json::from_str(datos)
            .map_err(|e| ErrorIa::Protocolo(format!("evento ilegible ({e}): {datos:.120}")))?;
        if let Some(err) = v.get("error") {
            let m = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("error del motor");
            return Err(ErrorIa::Protocolo(m.to_string()));
        }
        if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
            self.medidas.tokens_entrada = num(u, "prompt_tokens") as u32;
            self.medidas.tokens_salida = num(u, "completion_tokens") as u32;
            // DeepSeek: lo que acerto su cache de disco.
            if num(u, "prompt_cache_hit_tokens") > 0.0 {
                self.medidas.tokens_cache_leidos = num(u, "prompt_cache_hit_tokens") as u32;
            }
        }
        if let Some(t) = v.get("timings").filter(|t| !t.is_null()) {
            self.medidas.tokens_por_segundo_lectura = num(t, "prompt_per_second") as f32;
            self.medidas.tokens_por_segundo_escritura = num(t, "predicted_per_second") as f32;
            // llama.cpp: los tokens del principio que reutilizo de la ranura.
            if num(t, "cache_n") > 0.0 {
                self.medidas.tokens_cache_leidos = num(t, "cache_n") as u32;
            }
        }
        let Some(eleccion) = v.get("choices").and_then(|c| c.get(0)) else {
            return Ok(());
        };
        let delta = eleccion.get("delta").or_else(|| eleccion.get("message"));
        if let Some(d) = delta {
            if let Some(t) = d
                .get("content")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
            {
                self.texto.push_str(t);
                trozos.push(Trozo::Texto(t.to_string()));
            }
            if let Some(t) = d
                .get("reasoning_content")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
            {
                self.pensamiento.push_str(t);
                trozos.push(Trozo::Pensando(t.to_string()));
            }
            if let Some(Value::Array(lista)) = d.get("tool_calls") {
                for (pos, l) in lista.iter().enumerate() {
                    let indice = l
                        .get("index")
                        .and_then(Value::as_u64)
                        .map_or(pos, |i| i as usize);
                    if indice > 64 {
                        return Err(ErrorIa::Protocolo("demasiadas llamadas a la vez".into()));
                    }
                    while self.llamadas.len() <= indice {
                        self.llamadas.push(LlamadaCruda {
                            id: String::new(),
                            nombre: String::new(),
                            argumentos: String::new(),
                        });
                    }
                    let destino = &mut self.llamadas[indice];
                    if let Some(id) = l.get("id").and_then(Value::as_str) {
                        destino.id = id.to_string();
                    }
                    if let Some(f) = l.get("function") {
                        if let Some(n) = f
                            .get("name")
                            .and_then(Value::as_str)
                            .filter(|n| !n.is_empty())
                        {
                            let empezaba = destino.nombre.is_empty();
                            destino.nombre.push_str(n);
                            if empezaba {
                                trozos.push(Trozo::Llamando {
                                    nombre: destino.nombre.clone(),
                                });
                            }
                        }
                        if let Some(a) = f.get("arguments").and_then(Value::as_str) {
                            destino.argumentos.push_str(a);
                        }
                    }
                }
            }
        }
        if let Some(motivo) = eleccion.get("finish_reason").and_then(Value::as_str) {
            self.fin = Some(match motivo {
                "tool_calls" => Fin::Herramientas,
                "length" => Fin::Longitud,
                _ => Fin::Natural,
            });
        }
        Ok(())
    }

    /// La respuesta entera. `cortado`: el usuario paro a medias.
    pub fn respuesta(mut self, cortado: bool) -> Respuesta {
        for (i, l) in self.llamadas.iter_mut().enumerate() {
            if l.id.is_empty() {
                l.id = format!("llamada-{i}");
            }
        }
        self.llamadas.retain(|l| !l.nombre.is_empty());
        let fin = if cortado {
            Fin::Cortado
        } else if !self.llamadas.is_empty() {
            Fin::Herramientas
        } else {
            self.fin.unwrap_or(Fin::Natural)
        };
        Respuesta {
            texto: self.texto,
            pensamiento: self.pensamiento,
            llamadas: self.llamadas,
            fin,
            medidas: self.medidas,
        }
    }
}

fn num(v: &Value, campo: &str) -> f64 {
    v.get(campo).and_then(Value::as_f64).unwrap_or(0.0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn junta_texto_y_llamadas_partidas_entre_paquetes() {
        let flujo = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"Ho\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"la\"}}]}\n\n",
            ": latido\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"abc\",\"type\":\"function\",\"function\":{\"name\":\"grafica\",\"arguments\":\"{\\\"tipo\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"barras\\\"}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":775,\"completion_tokens\":79},\"timings\":{\"prompt_per_second\":63.2,\"predicted_per_second\":15.9}}\n\n",
            "data: [DONE]\n\n",
        );
        let mut a = Acumulador::nuevo();
        let mut trozos = Vec::new();
        // A pedacitos de 7 bytes, cortando lineas y UTF-8 por cualquier sitio.
        for c in flujo.as_bytes().chunks(7) {
            trozos.extend(a.meter(c).unwrap());
        }
        assert!(a.acabado());
        let r = a.respuesta(false);
        assert_eq!(r.texto, "Hola");
        assert_eq!(r.llamadas.len(), 1);
        assert_eq!(r.llamadas[0].id, "abc");
        assert_eq!(r.llamadas[0].nombre, "grafica");
        assert_eq!(r.llamadas[0].argumentos, "{\"tipo\":\"barras\"}");
        assert_eq!(r.fin, Fin::Herramientas);
        assert_eq!(r.medidas.tokens_entrada, 775);
        assert!((r.medidas.tokens_por_segundo_escritura - 15.9).abs() < 1e-3);
        assert_eq!(
            trozos,
            vec![
                Trozo::Texto("Ho".into()),
                Trozo::Texto("la".into()),
                Trozo::Llamando {
                    nombre: "grafica".into()
                }
            ]
        );
    }

    #[test]
    fn las_tildes_partidas_entre_paquetes_no_se_rompen() {
        let flujo = "data: {\"choices\":[{\"delta\":{\"content\":\"canción\"}}]}\n";
        let mut a = Acumulador::nuevo();
        let bytes = flujo.as_bytes();
        let corte = flujo.find('ó').unwrap() + 1; // a mitad de la «ó»
        a.meter(&bytes[..corte]).unwrap();
        a.meter(&bytes[corte..]).unwrap();
        assert_eq!(a.respuesta(false).texto, "canción");
    }

    #[test]
    fn un_error_del_motor_se_convierte_en_error() {
        let mut a = Acumulador::nuevo();
        let e = a
            .meter(b"data: {\"error\":{\"message\":\"contexto lleno\"}}\n")
            .unwrap_err();
        assert!(e.to_string().contains("contexto lleno"));
    }

    #[test]
    fn json_roto_en_el_flujo_es_error_de_protocolo() {
        let mut a = Acumulador::nuevo();
        assert!(matches!(
            a.meter(b"data: {roto\n"),
            Err(ErrorIa::Protocolo(_))
        ));
    }

    #[test]
    fn sin_id_se_le_pone_uno() {
        let mut a = Acumulador::nuevo();
        a.meter(b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"ocr\",\"arguments\":\"{}\"}}]}}]}\n")
            .unwrap();
        let r = a.respuesta(false);
        assert_eq!(r.llamadas[0].id, "llamada-0");
    }

    #[test]
    fn la_respuesta_cortada_lo_dice() {
        let mut a = Acumulador::nuevo();
        a.meter(b"data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n")
            .unwrap();
        assert_eq!(a.respuesta(true).fin, Fin::Cortado);
    }
}
