//! **Las hojas de calculo del chat**: abrirlas, editarlas celda a celda y
//! guardarlas donde las guarda Windows (`por-traducir/pixpin/src/hoja_pin.rs`):
//!
//! - **Una tabla del PC** (mensaje `MINIAPP` con `miniapp = "tabla"`): el
//!   JSON de `TablaDeCalculo` va en el `texto` del mensaje, y guardar lo
//!   reescribe ahi con `cuaderno::cambiar` y su `tocado` claramente despues
//!   de la hora del mensaje (`editada_a`).
//! - **Una tabla del movil** (mensaje `TABLA` con `referencia`): su JSON
//!   vive en `tablas/<ref>.json` (`vista::tabla_del_movil`) y se guarda en
//!   ESE fichero (`vista::guardar_tabla_del_movil`), que es el que viaja.
//! - **Las hojas viejas de un Excel del movil** (antes del 13-oct-2026 el
//!   movil guardaba las hojas editadas de un `.xlsx` solo en
//!   `tablas/libro-<mensaje>-<hoja>.json`): se leen
//!   (`vista::tablas_viejas_del_libro`) y, al guardar, todas pasan a mensajes
//!   `MINIAPP` tabla con esa referencia, como en Windows (`hoja_pin.rs`,
//!   commit 01dccd4). Los ficheros no se tocan. El Mac aun no edita un
//!   `.xlsx` (lo ensena Vista Rapida): sin hojas viejas no es una tabla.
//!
//! **Un solo formato** (15-oct-2026): el JSON de `Tabla` en todos los
//! caminos. `Tabla::leer` abre cualquier objeto JSON (estilos-palabra del PC
//! viejo, numeros en celdas de un movil viejo...) y lo que no entiende vuelve
//! al guardar.
//!
//! Lo que el Mac no entiende (`protegida`, claves nuevas del movil) se
//! conserva: `Tabla` y `EstiloDeCelda` guardan lo desconocido tal cual.
//!
//! # Rendimiento
//!
//! La tabla es un mapa de celda a texto (una con `A1` y `Z900` son dos
//! entradas). Swift pide **solo las celdas del rectangulo que se ve**
//! ([`HojaDeCalculo::celdas`]) y las formulas se calculan una vez por cambio
//! (`formula::evaluar_todo`), no por fotograma.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use pixpin_proyecto::formula::{self, Valor};
use pixpin_proyecto::tabla::{self, EstiloDeCelda, Ref, Tabla};
use pixpin_proyecto::{almacen, cuaderno, importar_hojas, portapapeles_tabla, vista};

use crate::chats::validar_chat;

// Iguales que los de Windows (su estilo, sin rustfmt): asi se comparan facil.
#[rustfmt::skip]
mod editor;
#[rustfmt::skip]
mod libro;
use crate::{ErrorNucleo, MensajeChat, Nucleo};
use editor::{Alineacion, Editor, Formato, HojaEditable, Interruptor, Seleccion};

/// El formato de una celda, como lo guardan Windows y el movil.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct EstiloCeldaHoja {
    pub negrita: bool,
    pub cursiva: bool,
    pub subrayado: bool,
    pub tachado: bool,
    /// `i`, `c`, `d` o nada (la que toque por el valor).
    pub alineacion: Option<String>,
    /// El fondo, `#rrggbb`.
    pub fondo: Option<String>,
    /// El color de la letra, `#rrggbb`.
    pub color: Option<String>,
    /// Ajustar el texto (varias lineas).
    pub ajustar: bool,
    /// `numero`, `moneda`, `porcentaje`, `fecha` o nada («General»).
    pub formato_numero: Option<String>,
}

impl From<&EstiloDeCelda> for EstiloCeldaHoja {
    fn from(e: &EstiloDeCelda) -> Self {
        EstiloCeldaHoja {
            negrita: e.n,
            cursiva: e.k,
            subrayado: e.s,
            tachado: e.t,
            alineacion: e.a.clone(),
            fondo: e.f.clone(),
            color: e.c.clone(),
            ajustar: e.w,
            formato_numero: e.nf.clone(),
        }
    }
}

/// Pone en `e` lo que dice `n`, sin tocar lo que el Mac no entiende.
fn aplicar(e: &mut EstiloDeCelda, n: &EstiloCeldaHoja) {
    let limpio = |v: &Option<String>| v.clone().filter(|s| !s.trim().is_empty());
    e.n = n.negrita;
    e.k = n.cursiva;
    e.s = n.subrayado;
    e.t = n.tachado;
    e.a = limpio(&n.alineacion).filter(|a| matches!(a.as_str(), "i" | "c" | "d"));
    e.f = limpio(&n.fondo);
    e.c = limpio(&n.color);
    e.w = n.ajustar;
    e.nf = limpio(&n.formato_numero);
}

/// Una celda con algo, tal como se ensena.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CeldaHoja {
    /// Desde 0.
    pub fila: u32,
    pub columna: u32,
    /// Lo escrito (una formula empieza por `=`).
    pub contenido: String,
    /// Lo que se ensena: el resultado de la formula o el valor, con el
    /// separador decimal de este equipo (vista «General»).
    pub mostrado: String,
    /// El numero, si el valor lo es (para aplicarle su formato en Swift).
    pub numero: Option<f64>,
    /// Si el valor es un error de formula (`#¡DIV/0!`...).
    pub error: bool,
    pub estilo: Option<EstiloCeldaHoja>,
}

/// El ancho de una columna puesto a mano, en pixeles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AnchoColumna {
    pub columna: u32,
    pub ancho: u32,
}

/// De donde viene el libro y donde se guarda (`hoja_pin::Origen` de
/// Windows).
#[derive(Debug, Clone)]
enum Origen {
    /// Tablas del chat (`MINIAPP` tabla): cada hoja en el `texto` de su
    /// mensaje (`HojaEditable::id_mensaje`).
    Chat { carpeta: PathBuf },
    /// `TABLA` del movil: en su fichero `tablas/<ref>.json`.
    DelMovil { archivo: PathBuf },
    /// Un Excel, OpenDocument o CSV del chat: sus hojas, leidas la primera
    /// vez (o las viejas del movil, `tablas/libro-*.json`), sin mensaje
    /// hasta que se guardan; entonces todas pasan a mensajes `MINIAPP` tabla
    /// con la referencia del libro (`libro::mensajes_de_hojas`) y el archivo
    /// no cambia. Las que ya eran mensajes se guardan en ellos.
    Libro {
        carpeta: PathBuf,
        /// El mensaje del archivo.
        libro: Box<cuaderno::Mensaje>,
        ruta: Option<PathBuf>,
        proyecto: String,
        aparato: String,
    },
}

struct Estado {
    origen: Origen,
    editor: Editor,
    /// Ya se dijo que el archivo original no cambia.
    avisado: bool,
}

/// **Una hoja de calculo del chat abierta para editar**: un libro con sus
/// hojas en pestanas (una tabla suelta es un libro de una hoja). Lo que no
/// dice que hoja, es de la activa.
#[derive(uniffi::Object)]
pub struct HojaDeCalculo {
    estado: Mutex<Estado>,
    /// El separador decimal de este equipo para ensenar los numeros.
    decimal: char,
}

impl HojaDeCalculo {
    fn tomar(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn nueva(origen: Origen, hojas: Vec<HojaEditable>) -> Result<HojaDeCalculo, ErrorNucleo> {
        let editor = Editor::nuevo(hojas, 0).ok_or_else(|| ErrorNucleo::Argumento {
            detalle: "el libro no tiene hojas".into(),
        })?;
        Ok(HojaDeCalculo {
            estado: Mutex::new(Estado {
                origen,
                editor,
                avisado: false,
            }),
            decimal: pixpin_sistema::entorno::separador_decimal(),
        })
    }
}

/// El `tocado` de una tabla guardada: ahora, pero siempre despues de la hora
/// de su mensaje (`hoja_pin::editada_a`).
fn editada_a(ahora: i64, cuando: i64) -> i64 {
    ahora.max(cuando + 2_001)
}

/// **Un cambio de formato en lo elegido** (`editor::Formato`): los botones y
/// los atajos de la barra de formato.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FormatoHoja {
    Negrita,
    Cursiva,
    Subrayado,
    Tachado,
    Ajustar,
    /// El fondo `#rrggbb`; `None`, sin color.
    Fondo {
        color: Option<String>,
    },
    /// El color de la letra; `None`, el automatico.
    Letra {
        color: Option<String>,
    },
    /// `i`, `c` o `d`; la misma otra vez la quita.
    Alinear {
        alineacion: String,
    },
    /// La siguiente alineacion (el boton, como el movil).
    AlinearSiguiente,
    /// `numero`, `moneda`, `porcentaje`, `fecha`; `None`, General.
    Numero {
        formato: Option<String>,
    },
    /// Quitar todo el formato.
    Borrar,
}

/// Lo elegido: el ancla y la celda activa (desde 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct SeleccionHoja {
    pub ancla_fila: u32,
    pub ancla_columna: u32,
    pub fila: u32,
    pub columna: u32,
}

/// Lo copiado: el texto con tabuladores (los valores) y la tabla HTML (las
/// formulas), como lo copian Excel y Windows.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CopiaHoja {
    pub texto: String,
    pub html: String,
}

#[uniffi::export]
impl HojaDeCalculo {
    /// El nombre de la hoja activa.
    pub fn nombre(&self) -> String {
        self.tomar().editor.hoja().nombre.clone()
    }

    /// **Las pestanas**: el nombre de cada hoja, en su orden (sin el
    /// «libro · » que les pone `importar_hojas`).
    pub fn hojas(&self) -> Vec<String> {
        self.tomar()
            .editor
            .hojas
            .iter()
            .map(|h| libro::nombre_de_pestana(&h.nombre).to_string())
            .collect()
    }

    /// La hoja que se ve.
    pub fn activa(&self) -> u32 {
        self.tomar().editor.activa as u32
    }

    /// Pasa a otra hoja (lo tecleado en la que se deja ya se confirmo).
    pub fn cambiar_hoja(&self, hoja: u32) {
        self.tomar().editor.cambiar_hoja(hoja as usize);
    }

    /// Hasta donde llega lo escrito: `[columnas, filas]`.
    pub fn tamano(&self) -> Vec<u32> {
        let (c, f) = self.tomar().editor.hoja().tabla.tamano();
        vec![c, f]
    }

    /// **Las celdas con algo del rectangulo** `filas x columnas` (desde 0,
    /// fin excluido), calculadas. Las vacias sin formato no vienen.
    pub fn celdas(
        &self,
        desde_fila: u32,
        hasta_fila: u32,
        desde_columna: u32,
        hasta_columna: u32,
    ) -> Vec<CeldaHoja> {
        let g = self.tomar();
        let h = g.editor.hoja();
        let valores = h.valores();
        let dentro = |r: &Ref| {
            (desde_fila..hasta_fila).contains(&r.fila)
                && (desde_columna..hasta_columna).contains(&r.columna)
        };
        let mut v: Vec<CeldaHoja> = Vec::new();
        let mut vistas = std::collections::BTreeSet::new();
        for (clave, contenido) in &h.tabla.celdas {
            let Some(r) = tabla::ref_de(clave) else {
                continue;
            };
            if !dentro(&r) {
                continue;
            }
            let valor = valores
                .get(clave)
                .cloned()
                .unwrap_or_else(|| formula::literal(contenido));
            vistas.insert(clave.clone());
            v.push(CeldaHoja {
                fila: r.fila,
                columna: r.columna,
                contenido: contenido.clone(),
                mostrado: valor.mostrar(self.decimal),
                numero: match valor {
                    Valor::Numero(n) => Some(n),
                    _ => None,
                },
                error: matches!(valor, Valor::Error(_)),
                estilo: h.tabla.estilos.get(clave).map(EstiloCeldaHoja::from),
            });
        }
        // Las vacias con formato (un fondo puesto) tambien se ven.
        for (clave, e) in &h.tabla.estilos {
            if vistas.contains(clave) || e.sin_formato() {
                continue;
            }
            let Some(r) = tabla::ref_de(clave) else {
                continue;
            };
            if dentro(&r) {
                v.push(CeldaHoja {
                    fila: r.fila,
                    columna: r.columna,
                    contenido: String::new(),
                    mostrado: String::new(),
                    numero: None,
                    error: false,
                    estilo: Some(EstiloCeldaHoja::from(e)),
                });
            }
        }
        v
    }

    /// Lo escrito en una celda.
    pub fn contenido(&self, fila: u32, columna: u32) -> String {
        self.tomar()
            .editor
            .hoja()
            .tabla
            .celda(Ref { columna, fila })
            .to_string()
    }

    /// **Escribe una celda** (vacio la quita), como un paso de deshacer.
    /// Devuelve si cambio algo.
    pub fn poner(&self, fila: u32, columna: u32, contenido: String) -> bool {
        let mut g = self.tomar();
        let e = &mut g.editor;
        e.edicion = None;
        e.poner_seleccion(Seleccion::en(Ref { columna, fila }));
        e.edicion = Some(contenido);
        e.confirmar()
    }

    /// El formato de una celda (`None` sin formato).
    pub fn estilo(&self, fila: u32, columna: u32) -> Option<EstiloCeldaHoja> {
        let clave = tabla::ref_a(Ref { columna, fila });
        self.tomar()
            .editor
            .hoja()
            .tabla
            .estilos
            .get(&clave)
            .map(EstiloCeldaHoja::from)
    }

    /// **Pone el formato de una celda.** Uno sin nada que se vea (y sin lo
    /// que el Mac no entiende) se quita en vez de guardarse vacio.
    pub fn poner_estilo(&self, fila: u32, columna: u32, estilo: EstiloCeldaHoja) -> bool {
        self.tomar()
            .editor
            .estilo_de_celda(Ref { columna, fila }, |e| aplicar(e, &estilo))
    }

    /// Elige un rango (el ancla y la celda activa; las dos iguales, una
    /// celda).
    pub fn elegir(&self, s: SeleccionHoja) {
        let mut g = self.tomar();
        g.editor.confirmar();
        g.editor.poner_seleccion(Seleccion {
            ancla: Ref {
                columna: s.ancla_columna,
                fila: s.ancla_fila,
            },
            activa: Ref {
                columna: s.columna,
                fila: s.fila,
            },
        });
    }

    /// Lo elegido en la hoja activa.
    pub fn seleccion(&self) -> SeleccionHoja {
        let s = self.tomar().editor.hoja().sel;
        SeleccionHoja {
            ancla_fila: s.ancla.fila,
            ancla_columna: s.ancla.columna,
            fila: s.activa.fila,
            columna: s.activa.columna,
        }
    }

    /// **Un cambio de formato en todo lo elegido**, como un solo paso de
    /// deshacer (un interruptor se pone si alguna celda no lo tiene y se
    /// quita si ya lo tienen todas). `false` si no cambio nada o el rango es
    /// demasiado grande.
    pub fn formato(&self, f: FormatoHoja) -> bool {
        use pixpin_proyecto::tabla::FormatoNumero;
        let f = match f {
            FormatoHoja::Negrita => Formato::Alternar(Interruptor::Negrita),
            FormatoHoja::Cursiva => Formato::Alternar(Interruptor::Cursiva),
            FormatoHoja::Subrayado => Formato::Alternar(Interruptor::Subrayado),
            FormatoHoja::Tachado => Formato::Alternar(Interruptor::Tachado),
            FormatoHoja::Ajustar => Formato::Alternar(Interruptor::Ajustar),
            FormatoHoja::Fondo { color } => Formato::Fondo(color.filter(|c| !c.trim().is_empty())),
            FormatoHoja::Letra { color } => Formato::Letra(color.filter(|c| !c.trim().is_empty())),
            FormatoHoja::Alinear { alineacion } => match Alineacion::de(&alineacion) {
                Some(a) => Formato::Alinear(a),
                None => return false,
            },
            FormatoHoja::AlinearSiguiente => Formato::AlinearSiguiente,
            FormatoHoja::Numero { formato } => Formato::Numero(match formato.as_deref() {
                None | Some("") => None,
                Some(p) => match FormatoNumero::de(p) {
                    Some(n) => Some(n),
                    None => return false,
                },
            }),
            FormatoHoja::Borrar => Formato::Borrar,
        };
        self.tomar().editor.formato(f)
    }

    /// Suprimir: vacia lo elegido (el formato se queda, como en Excel).
    pub fn borrar_seleccion(&self) -> bool {
        self.tomar().editor.borrar_seleccion()
    }

    /// **Copia lo elegido** (texto con tabuladores y HTML).
    pub fn copiar(&self) -> CopiaHoja {
        let g = self.tomar();
        let h = g.editor.hoja();
        let (texto, html) =
            portapapeles_tabla::copiar(&h.tabla, h.sel.ancla, h.sel.activa, self.decimal);
        CopiaHoja { texto, html }
    }

    /// **Pega** lo del portapapeles (la tabla HTML de Excel o Sheets, o
    /// texto con tabuladores) con la esquina en la celda activa, como un
    /// solo paso de deshacer. Lo pegado queda elegido.
    pub fn pegar(&self, html: Option<String>, texto: Option<String>) -> bool {
        let Some(filas) = portapapeles_tabla::leer(html.as_deref(), texto.as_deref()) else {
            return false;
        };
        self.tomar().editor.pegar(&filas)
    }

    /// Deshace el ultimo cambio (vuelve a su hoja y a su seleccion).
    pub fn deshacer(&self) -> bool {
        self.tomar().editor.deshacer()
    }

    pub fn rehacer(&self) -> bool {
        self.tomar().editor.rehacer()
    }

    pub fn puede_deshacer(&self) -> bool {
        self.tomar().editor.puede_deshacer()
    }

    pub fn puede_rehacer(&self) -> bool {
        self.tomar().editor.puede_rehacer()
    }

    /// Cuantos cambios lleva (sube tambien al deshacer): si cambia, lo que
    /// se ve hay que pedirlo otra vez.
    pub fn cambios(&self) -> u64 {
        self.tomar().editor.cambios()
    }

    /// El archivo del libro (para «Abrir con su programa»), si lo es.
    pub fn ruta_original(&self) -> Option<String> {
        match &self.tomar().origen {
            Origen::Libro { ruta, .. } => ruta.as_ref().map(|r| r.display().to_string()),
            _ => None,
        }
    }

    /// **Si hay que avisar de que el archivo original no cambia**: la
    /// primera vez que se edita un libro que aun no tiene tablas en el chat.
    /// Una vez por hoja abierta.
    pub fn aviso_al_editar(&self) -> bool {
        let mut g = self.tomar();
        let primera = matches!(g.origen, Origen::Libro { .. })
            && g.editor.hojas.iter().any(|h| h.id_mensaje.is_none());
        if g.avisado || !primera {
            return false;
        }
        g.avisado = true;
        true
    }

    /// Los anchos puestos a mano.
    pub fn anchos(&self) -> Vec<AnchoColumna> {
        self.tomar()
            .editor
            .hoja()
            .tabla
            .anchos
            .iter()
            .filter_map(|(k, v)| {
                let r = tabla::ref_de(&format!("{k}1"))?;
                Some(AnchoColumna {
                    columna: r.columna,
                    ancho: *v,
                })
            })
            .collect()
    }

    /// Pone el ancho de una columna (`None` vuelve al de siempre).
    pub fn poner_ancho(&self, columna: u32, ancho: Option<u32>) -> bool {
        let letras = tabla::ref_a(Ref { columna, fila: 0 })
            .trim_end_matches('1')
            .to_string();
        let mut g = self.tomar();
        let h = g.editor.hoja_mut();
        let antes = h.tabla.anchos.get(&letras).copied();
        let nuevo = ancho.map(|a| a.clamp(16, 2000));
        if antes == nuevo {
            return false;
        }
        match nuevo {
            Some(a) => h.tabla.anchos.insert(letras, a),
            None => h.tabla.anchos.remove(&letras),
        };
        h.tocada = true;
        true
    }

    /// Si hay cambios sin guardar.
    pub fn tocada(&self) -> bool {
        self.tomar().editor.tocado()
    }

    /// **Guarda** donde vive cada hoja (ver la doc del modulo y
    /// `Editable::guardar` de Windows). Devuelve si escribio algo.
    /// Bloqueante: Swift lo llama fuera del hilo principal.
    pub fn guardar(&self) -> Result<bool, ErrorNucleo> {
        let mut g = self.tomar();
        let st = &mut *g;
        let ed = &mut st.editor;
        ed.confirmar();
        if !ed.tocado() {
            return Ok(false);
        }
        let ahora = crate::ahora_ms();
        let disco = |e: serde_json::Error| ErrorNucleo::Disco {
            detalle: e.to_string(),
        };
        let carpeta = match &st.origen {
            Origen::Chat { carpeta } | Origen::Libro { carpeta, .. } => carpeta.clone(),
            Origen::DelMovil { archivo } => {
                // Una tabla del movil se guarda en SU fichero, el que viaja.
                for h in ed.hojas.iter_mut().filter(|h| h.tocada) {
                    h.tabla.tocado = h.tabla.tocado.max(ahora);
                    let texto = h.tabla.escribir().map_err(disco)?;
                    vista::guardar_tabla_del_movil(archivo, &texto)?;
                    h.tocada = false;
                }
                return Ok(true);
            }
        };
        if let Origen::Libro {
            libro,
            proyecto,
            aparato,
            ..
        } = &st.origen
            && ed.hojas.iter().any(|h| h.id_mensaje.is_none())
        {
            let c = cuaderno::Cuaderno::leer_de(&carpeta)?;
            let hechas = libro::tablas_hechas(&c.mensajes, libro);
            if hechas.len() == ed.hojas.len() {
                // Otro (Windows, el movil) ya las paso a mensajes mientras
                // se editaba: se guardan en ellas, sin duplicarlas.
                for (h, i) in ed.hojas.iter_mut().zip(hechas) {
                    h.id_mensaje = Some(c.mensajes[i].id.clone());
                    h.tocada = true;
                }
            } else {
                let sello = cuaderno::Sello {
                    cuando: ahora,
                    numero: c.siguiente_numero(Some(proyecto)),
                    aparato: aparato.clone(),
                    proyecto: proyecto.clone(),
                };
                // La hoja cambiada lleva un `tocado` posterior a la hora de
                // su mensaje (`libro::editada`); las demas, la de leerla.
                for (i, h) in ed.hojas.iter_mut().enumerate().filter(|(_, h)| h.tocada) {
                    h.tabla.tocado = editada_a(ahora, sello.cuando + i as i64);
                }
                let listas = ed
                    .hojas
                    .iter()
                    .map(|h| {
                        Ok(libro::HojaLista {
                            nombre: h.nombre.clone(),
                            documento: h.tabla.escribir().map_err(disco)?,
                        })
                    })
                    .collect::<Result<Vec<_>, ErrorNucleo>>()?;
                let nuevos = libro::mensajes_de_hojas(libro, &listas, &sello);
                let lineas = nuevos
                    .iter()
                    .map(cuaderno::linea_de)
                    .collect::<std::io::Result<Vec<_>>>()?;
                cuaderno::anadir_lineas(&carpeta, &lineas)?;
                for (h, m) in ed.hojas.iter_mut().zip(&nuevos) {
                    h.id_mensaje = Some(m.id.clone());
                    h.tocada = false;
                }
                return Ok(true);
            }
        }
        for h in ed.hojas.iter_mut().filter(|h| h.tocada) {
            let Some(id) = h.id_mensaje.clone() else {
                continue;
            };
            let tabla = &mut h.tabla;
            let mut fallo = None;
            let hecho = cuaderno::cambiar(&carpeta, &id, |m| {
                tabla.tocado = editada_a(ahora, m.cuando);
                match tabla.escribir() {
                    Ok(t) => {
                        m.texto = t;
                        true
                    }
                    Err(e) => {
                        fallo = Some(e.to_string());
                        false
                    }
                }
            })?;
            if let Some(detalle) = fallo {
                return Err(ErrorNucleo::Disco { detalle });
            }
            // Se borro del chat mientras se editaba: no se resucita.
            if hecho.is_none() {
                return Err(ErrorNucleo::NoExiste { que: id });
            }
            h.tocada = false;
        }
        Ok(true)
    }
}

/// **Las hojas viejas del movil de un libro** (`tablas/libro-<id>-<hoja>.json`,
/// ver `vista::tablas_viejas_del_libro`), con su nombre de pestana. Se leen
/// hasta la primera que no se entienda (como `TablasDelChat.crearLasDelLibro`
/// del movil); si ni la primera se entiende, `None`. Los ficheros no se
/// tocan: uno roto sigue ahi tal cual. Como `hoja_pin::tablas_viejas_del_libro`
/// de Windows.
fn tablas_viejas_del_libro(
    raiz: &std::path::Path,
    proyecto: &str,
    libro_m: &cuaderno::Mensaje,
) -> Option<Vec<(String, Tabla)>> {
    let mut hojas = Vec::new();
    for (i, archivo) in vista::tablas_viejas_del_libro(raiz, proyecto, &libro_m.id)
        .into_iter()
        .enumerate()
    {
        let leida = std::fs::read_to_string(&archivo)
            .map_err(|e| e.to_string())
            .and_then(|t| Tabla::leer(&t).map_err(|e| e.to_string()));
        match leida {
            Ok(t) => {
                let nombre = if t.nombre.trim().is_empty() {
                    format!("Hoja{}", i + 1)
                } else {
                    t.nombre.clone()
                };
                hojas.push((nombre, t));
            }
            Err(_) => break,
        }
    }
    (!hojas.is_empty()).then_some(hojas)
}

/// El archivo de un libro del chat en este equipo, si esta.
fn ruta_del_libro(raiz: &std::path::Path, chat: &str, m: &cuaderno::Mensaje) -> Option<PathBuf> {
    m.ruta
        .as_deref()
        .and_then(|r| vista::ruta_real(raiz, chat, r))
        .filter(|r| r.is_file())
}

/// Si `m` es un libro que se edita aqui (`hoja_pin::que_abrir`): un Excel,
/// OpenDocument o CSV que no sea un `.xls` antiguo (no se lee).
fn es_libro_editable(m: &cuaderno::Mensaje) -> bool {
    libro::es_libro(m) && libro::extension(m) != "xls"
}

/// Una tabla del chat como hoja: con el nombre de su tabla o el de su
/// mensaje.
fn hoja_de_mensaje(m: &cuaderno::Mensaje, texto: &str) -> Result<HojaEditable, ErrorNucleo> {
    let t = Tabla::leer(texto).map_err(|e| ErrorNucleo::Argumento {
        detalle: format!("tabla que no se entiende: {e}"),
    })?;
    let nombre = if t.nombre.trim().is_empty() {
        m.nombre.clone()
    } else {
        t.nombre.clone()
    };
    Ok(HojaEditable::nueva(nombre, t, Some(m.id.clone())))
}

/// Un color de la barra de formato: `#rrggbb` y su nombre.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ColorHoja {
    pub hex: String,
    pub nombre: String,
}

fn colores(lista: &[(&str, &str)]) -> Vec<ColorHoja> {
    lista
        .iter()
        .map(|(hex, clave)| {
            let n = clave.trim_start_matches("hoja-color-");
            let mut c = n.chars();
            let nombre = c
                .next()
                .map(|p| p.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default();
            ColorHoja {
                hex: hex.to_string(),
                nombre,
            }
        })
        .collect()
}

/// Los fondos de «Resaltar»: la paleta del movil, en su orden.
#[uniffi::export]
pub fn hoja_fondos() -> Vec<ColorHoja> {
    colores(&editor::FONDOS)
}

/// Los colores de letra: los fuertes de la paleta de Excel.
#[uniffi::export]
pub fn hoja_colores_de_letra() -> Vec<ColorHoja> {
    colores(&editor::LETRAS)
}

#[uniffi::export]
impl Nucleo {
    /// **Abre la hoja de calculo de un mensaje**: una tabla del PC (`MINIAPP`
    /// tabla), una del movil (`TABLA`) o un libro del chat (Excel,
    /// OpenDocument, CSV) con todas sus hojas. `NoExiste` si el mensaje no
    /// esta o su fichero aun no llego (mejor no abrir que abrir en blanco y
    /// guardar encima); `Argumento` si no es una hoja o no se lee.
    pub fn abrir_hoja_de_calculo(
        &self,
        chat: String,
        mensaje: String,
    ) -> Result<Arc<HojaDeCalculo>, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let carpeta = almacen::carpeta(raiz, &chat);
        let c = cuaderno::Cuaderno::leer_de(&carpeta)?;
        let m = c
            .mensajes
            .iter()
            .find(|m| m.id == mensaje)
            .cloned()
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: mensaje.clone(),
            })?;
        let no_vale = |d: String| ErrorNucleo::Argumento { detalle: d };
        if let Some(archivo) = vista::tabla_del_movil(raiz, &chat, &m) {
            let texto = std::fs::read_to_string(&archivo).map_err(|_| ErrorNucleo::NoExiste {
                que: archivo.display().to_string(),
            })?;
            let h = hoja_de_mensaje(&m, &texto)?;
            return Ok(Arc::new(HojaDeCalculo::nueva(
                Origen::DelMovil { archivo },
                vec![h],
            )?));
        }
        if m.clase == Some(cuaderno::Clase::MiniApp) && m.miniapp.as_deref() == Some(tabla::MINIAPP)
        {
            let h = hoja_de_mensaje(&m, &m.texto)?;
            return Ok(Arc::new(HojaDeCalculo::nueva(
                Origen::Chat { carpeta },
                vec![h],
            )?));
        }
        if !es_libro_editable(&m) {
            return Err(no_vale(format!("no es una tabla: {mensaje}")));
        }
        // Un libro: las tablas que ya salieron de el (con lo cambiado), o
        // las hojas viejas del movil, o el archivo leido por primera vez.
        let ruta = ruta_del_libro(raiz, &chat, &m);
        let hechas = libro::tablas_hechas(&c.mensajes, &m);
        let hojas = if !hechas.is_empty() {
            hechas
                .iter()
                .map(|&i| {
                    let t = &c.mensajes[i];
                    let mut h = hoja_de_mensaje(t, &t.texto)?;
                    h.nombre = t.nombre.clone();
                    Ok(h)
                })
                .collect::<Result<Vec<_>, ErrorNucleo>>()?
        } else if let Some(viejas) = tablas_viejas_del_libro(raiz, &chat, &m) {
            viejas
                .into_iter()
                .map(|(nombre, t)| HojaEditable::nueva(nombre, t, None))
                .collect()
        } else {
            let ruta = ruta.clone().ok_or_else(|| ErrorNucleo::NoExiste {
                que: m.ruta.clone().unwrap_or_default(),
            })?;
            importar_hojas::leer(&ruta, &m.nombre, crate::ahora_ms())
                .map_err(|e| no_vale(e.to_string()))?
                .into_iter()
                .map(|h| HojaEditable::nueva(h.nombre, h.tabla, None))
                .collect()
        };
        Ok(Arc::new(HojaDeCalculo::nueva(
            Origen::Libro {
                carpeta,
                libro: Box::new(m),
                ruta,
                proyecto: chat,
                aparato: self.identidad()?.yo.codigo(),
            },
            hojas,
        )?))
    }

    /// Si el mensaje es una hoja de calculo que se abre con
    /// [`Nucleo::abrir_hoja_de_calculo`] (sin leer su tabla): una tabla, o
    /// un libro (no `.xls`) que este en este equipo o tenga ya sus hojas.
    pub fn es_hoja_de_calculo(&self, chat: String, mensaje: String) -> bool {
        if validar_chat(&chat).is_err() {
            return false;
        }
        let raiz = self.ruta();
        let carpeta = almacen::carpeta(raiz, &chat);
        cuaderno::Cuaderno::leer_de(&carpeta).is_ok_and(|c| {
            c.mensajes.iter().any(|m| {
                m.id == mensaje
                    && ((m.clase == Some(cuaderno::Clase::MiniApp)
                        && m.miniapp.as_deref() == Some(tabla::MINIAPP))
                        || m.clase == Some(cuaderno::Clase::Otra(vista::CLASE_TABLA.into()))
                        || (es_libro_editable(m)
                            && (ruta_del_libro(raiz, &chat, m).is_some()
                                || !libro::tablas_hechas(&c.mensajes, m).is_empty()
                                || tablas_viejas_del_libro(raiz, &chat, m).is_some())))
            })
        })
    }

    /// **Una tabla nueva en un chat**: un mensaje `MINIAPP` tabla con la
    /// tabla vacia, como la crea Windows. Se abre despues con
    /// [`Nucleo::abrir_hoja_de_calculo`].
    pub fn crear_tabla(&self, chat: String, nombre: String) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let indice = almacen::Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let nombre = {
            let n = nombre.trim();
            if n.is_empty() {
                "Tabla".to_string()
            } else {
                n.chars().take(80).collect()
            }
        };
        let cuando = crate::chats::hora_unica();
        let t = Tabla {
            nombre: nombre.clone(),
            tocado: cuando,
            ..Default::default()
        };
        let documento = t.escribir().map_err(|e| ErrorNucleo::Disco {
            detalle: e.to_string(),
        })?;
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        let aparato = self.identidad()?.yo.codigo();
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let m = cuaderno::Mensaje::miniapp(
            tabla::MINIAPP,
            &nombre,
            &documento,
            &cuaderno::Sello {
                cuando,
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero);
        drop(cache);
        let mut indice = indice;
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(MensajeChat::from(m))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nucleo(n: &str) -> (PathBuf, Arc<Nucleo>) {
        let d =
            std::env::temp_dir().join(format!("pixpin-nucleo-hojas-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let nu = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        (d, nu)
    }

    #[test]
    fn una_tabla_del_pc_se_edita_y_vuelve_al_texto_de_su_mensaje() {
        let (d, n) = nucleo("pc");
        let chat = n.crear_chat("Cuentas".into()).unwrap();
        let m = n.crear_tabla(chat.id.clone(), "Gastos".into()).unwrap();
        assert_eq!(m.clase, "MINIAPP");
        assert!(n.es_hoja_de_calculo(chat.id.clone(), m.id.clone()));
        let h = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert_eq!(h.nombre(), "Gastos");
        assert!(!h.guardar().unwrap(), "sin cambios no escribe");
        assert!(h.poner(0, 0, "12.5".into()));
        assert!(h.poner(1, 0, "7".into()));
        assert!(h.poner(2, 0, "=SUMA(A1:A2)".into()));
        assert!(h.poner_estilo(
            2,
            0,
            EstiloCeldaHoja {
                negrita: true,
                formato_numero: Some("moneda".into()),
                ..Default::default()
            }
        ));
        let total = h
            .celdas(0, 10, 0, 5)
            .into_iter()
            .find(|c| c.fila == 2)
            .unwrap();
        assert_eq!(total.numero, Some(19.5));
        assert!(total.estilo.as_ref().unwrap().negrita);
        assert!(h.guardar().unwrap());
        // En el cuaderno: el JSON de `TablaDeCalculo` en el texto, con los
        // nombres cortos del movil y del PC.
        let carpeta = almacen::carpeta(&d, &chat.id);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let guardado = c.mensajes.iter().find(|x| x.id == m.id).unwrap();
        let t = Tabla::leer(&guardado.texto).unwrap();
        assert_eq!(t.celdas.get("A3").map(String::as_str), Some("=SUMA(A1:A2)"));
        assert!(t.estilos["A3"].n);
        assert_eq!(t.estilos["A3"].nf.as_deref(), Some("moneda"));
        assert!(t.tocado >= guardado.cuando + 2_001);
        // Reabrir lee lo guardado; vaciar una celda la quita.
        let h2 = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert_eq!(h2.contenido(0, 0), "12.5");
        assert!(h2.poner(0, 0, String::new()));
        assert!(h2.guardar().unwrap());
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let t = Tabla::leer(&c.mensajes.iter().find(|x| x.id == m.id).unwrap().texto).unwrap();
        assert!(!t.celdas.contains_key("A1"));
        // Caso negativo: una nota no es una tabla.
        let nota = n
            .escribir_nota(chat.id.clone(), "hola".into(), None)
            .unwrap();
        assert!(!n.es_hoja_de_calculo(chat.id.clone(), nota.id.clone()));
        assert!(matches!(
            n.abrir_hoja_de_calculo(chat.id, nota.id),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }

    #[test]
    fn una_tabla_del_movil_se_guarda_en_su_fichero_y_conserva_lo_desconocido() {
        let (d, n) = nucleo("movil");
        let chat = n.crear_chat("Obra".into()).unwrap();
        // El mensaje `TABLA` del movil y su fichero `tablas/<ref>.json`.
        let carpeta = almacen::carpeta(&d, &chat.id);
        let m = cuaderno::Mensaje {
            id: "1700000000000".into(),
            cuando: 1_700_000_000_000,
            clase: Some(cuaderno::Clase::Otra(vista::CLASE_TABLA.into())),
            referencia: Some("t-medidas".into()),
            nombre: "Medidas".into(),
            numero: 1,
            uid: Some("ABCDEFGHJK".into()),
            proyecto: Some(chat.id.clone()),
            ..Default::default()
        };
        cuaderno::anadir(&carpeta, &m).unwrap();
        let archivo = vista::tabla_del_movil(&d, &chat.id, &m).unwrap();
        // Aun no llego el fichero: no se abre en blanco.
        assert!(matches!(
            n.abrir_hoja_de_calculo(chat.id.clone(), m.id.clone()),
            Err(ErrorNucleo::NoExiste { .. })
        ));
        std::fs::create_dir_all(archivo.parent().unwrap()).unwrap();
        std::fs::write(
            &archivo,
            r#"{"nombre":"Medidas","celdas":{"B2":"3"},"anchos":{},"estilos":{"B2":{"n":true}},"tocado":5,"protegida":true}"#,
        )
        .unwrap();
        let h = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert!(h.poner(1, 1, "4".into()));
        assert!(h.poner_ancho(1, Some(120)));
        assert!(h.guardar().unwrap());
        let t = Tabla::leer(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
        assert_eq!(t.celdas["B2"], "4");
        assert!(t.estilos["B2"].n);
        assert_eq!(t.anchos["B"], 120);
        assert_eq!(
            t.resto.get("protegida"),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(t.tocado > 5);
        assert_eq!(
            h.anchos(),
            vec![AnchoColumna {
                columna: 1,
                ancho: 120
            }]
        );
    }

    /// Un chat con un Excel adjunto (el `.xlsx` da igual: el Mac no lo lee)
    /// y, como lo dejaba el movil antes del 13-oct, sus hojas editadas solo
    /// en `tablas/libro-<mensaje>-<hoja>.json` (`pruebas_movil.rs` de Windows).
    fn con_libro_y_hojas_viejas(
        n: &Nucleo,
        d: &std::path::Path,
        viejas: &[&str],
    ) -> (String, cuaderno::Mensaje, Vec<PathBuf>) {
        let chat = n.crear_chat("Obra".into()).unwrap();
        let carpeta = almacen::carpeta(d, &chat.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join("Cuentas.xlsx"), b"PK").unwrap();
        let sello = cuaderno::Sello {
            cuando: 1_700_000_000_000,
            numero: 1,
            aparato: "MOVI".into(),
            proyecto: chat.id.clone(),
        };
        let m = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "Cuentas.xlsx",
            "Cuentas.xlsx",
            2,
            &sello,
        );
        cuaderno::anadir(&carpeta, &m).unwrap();
        let tablas = carpeta.join("android").join("tablas");
        std::fs::create_dir_all(&tablas).unwrap();
        let archivos = viejas
            .iter()
            .enumerate()
            .map(|(i, texto)| {
                let a = tablas.join(format!("libro-{}-{i}.json", m.id));
                std::fs::write(&a, texto).unwrap();
                a
            })
            .collect();
        (chat.id, m, archivos)
    }

    #[test]
    fn las_hojas_viejas_del_movil_se_editan_y_al_guardar_pasan_a_mensajes() {
        let (d, n) = nucleo("viejas");
        // La segunda, con un estilo del formato de antes (una palabra) y un
        // campo que el PC no conoce: se abre igual y no se pierde.
        let hoja0 = r##"{"nombre":"Cuentas · Gastos","celdas":{"A1":"Editado en el movil","B1":"5"},"tocado":1700000500000,"protegida":true}"##;
        let hoja1 = r##"{"nombre":"Cuentas · Resumen","celdas":{"A1":"Total"},"estilos":{"A1":"negrita"},"futuro":{"x":1}}"##;
        let (chat, m, archivos) = con_libro_y_hojas_viejas(&n, &d, &[hoja0, hoja1]);
        let antes: Vec<Vec<u8>> = archivos.iter().map(|a| std::fs::read(a).unwrap()).collect();
        assert_eq!(
            vista::tablas_viejas_del_libro(&d, &chat, &m.id),
            archivos,
            "se encuentran donde las deja la sincronizacion"
        );
        assert!(n.es_hoja_de_calculo(chat.clone(), m.id.clone()));
        let h = n.abrir_hoja_de_calculo(chat.clone(), m.id.clone()).unwrap();
        assert_eq!(
            h.contenido(0, 0),
            "Editado en el movil",
            "las del movil, no el libro"
        );
        assert!(h.poner(0, 1, "8".into()));
        assert!(h.guardar().unwrap());

        // Ahora son mensajes `tabla` con la referencia del libro: el formato
        // unico, el que el movil lee primero.
        let carpeta = almacen::carpeta(&d, &chat);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let hechas = libro::tablas_hechas(&c.mensajes, &m);
        assert_eq!(hechas.len(), 2);
        let gastos = &c.mensajes[hechas[0]];
        assert_eq!(
            gastos.referencia.as_deref(),
            Some(format!("libro-{}-0", m.id).as_str())
        );
        let t0 = Tabla::leer(&gastos.texto).unwrap();
        assert_eq!(t0.celda(tabla::ref_de("B1").unwrap()), "8");
        assert_eq!(t0.resto["protegida"], true, "lo que el PC no conoce sigue");
        assert!(libro::editada(gastos));
        let t1: serde_json::Value = serde_json::from_str(&c.mensajes[hechas[1]].texto).unwrap();
        assert_eq!(
            t1["estilos"]["A1"],
            serde_json::json!({"n": true}),
            "el estilo sale en el formato de ahora"
        );
        assert_eq!(t1["futuro"]["x"], 1);
        // Los ficheros viejos no se tocan (el movil los conserva tambien).
        let despues: Vec<Vec<u8>> = archivos.iter().map(|a| std::fs::read(a).unwrap()).collect();
        assert_eq!(antes, despues);
        // Guardar otra vez va al mensaje, sin duplicar las hojas; y el Excel
        // ya no se abre como tabla (sus hojas son burbujas del chat).
        assert!(h.poner(1, 0, "otra".into()));
        assert!(h.guardar().unwrap());
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let hechas = libro::tablas_hechas(&c.mensajes, &m);
        assert_eq!(hechas.len(), 2, "no se duplican");
        assert_eq!(
            Tabla::leer(&c.mensajes[hechas[0]].texto)
                .unwrap()
                .celda(tabla::ref_de("A2").unwrap()),
            "otra"
        );
        assert!(n.es_hoja_de_calculo(chat.clone(), c.mensajes[hechas[1]].id.clone()));
        // Y el Excel se abre ya por sus tablas, las dos en pestanas.
        assert!(n.es_hoja_de_calculo(chat.clone(), m.id.clone()));
        let otra = n.abrir_hoja_de_calculo(chat, m.id.clone()).unwrap();
        assert_eq!(otra.hojas(), vec!["Gastos", "Resumen"]);
        assert_eq!(otra.contenido(1, 0), "otra");
        assert!(!otra.aviso_al_editar(), "ya no es la primera vez");
    }

    #[test]
    fn una_hoja_vieja_rota_no_se_pierde_y_un_excel_que_no_se_lee_no_se_abre() {
        let (d, n) = nucleo("vieja-rota");
        // El `.xlsx` de la prueba no es un Excel de verdad («PK»).
        let (chat, m, archivos) = con_libro_y_hojas_viejas(&n, &d, &["{ esto no es una tabla"]);
        assert!(
            n.es_hoja_de_calculo(chat.clone(), m.id.clone()),
            "es un libro"
        );
        assert!(matches!(
            n.abrir_hoja_de_calculo(chat.clone(), m.id.clone()),
            Err(ErrorNucleo::Argumento { .. })
        ));
        assert_eq!(
            std::fs::read_to_string(&archivos[0]).unwrap(),
            "{ esto no es una tabla",
            "el fichero roto sigue tal cual"
        );
        // Caso negativo: un `.xls` antiguo no se lee: no es una hoja.
        let carpeta = almacen::carpeta(&d, &chat);
        std::fs::write(carpeta.join("viejo.xls"), b"xx").unwrap();
        let sello = cuaderno::Sello {
            cuando: 1_700_000_000_500,
            numero: 2,
            aparato: "MOVI".into(),
            proyecto: chat.clone(),
        };
        let xls = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "viejo.xls",
            "viejo.xls",
            2,
            &sello,
        );
        cuaderno::anadir(&carpeta, &xls).unwrap();
        assert!(!n.es_hoja_de_calculo(chat.clone(), xls.id.clone()));
        assert!(n.abrir_hoja_de_calculo(chat, xls.id).is_err());
    }

    /// Un movil viejo guardaba numeros sin comillas en las celdas y anchos
    /// con decimales: la tabla se abre igual (`Tabla::leer` tolerante) y se
    /// guarda en el formato de hoy. Un fichero que no es un objeto JSON no se
    /// abre ni se pisa.
    #[test]
    fn una_tabla_del_movil_viejo_con_numeros_se_abre_y_una_rota_no_se_pisa() {
        let (d, n) = nucleo("movil-viejo");
        let chat = n.crear_chat("Obra".into()).unwrap();
        let carpeta = almacen::carpeta(&d, &chat.id);
        let m = cuaderno::Mensaje {
            id: "1700000000001".into(),
            cuando: 1_700_000_000_001,
            clase: Some(cuaderno::Clase::Otra(vista::CLASE_TABLA.into())),
            referencia: Some("t-vieja".into()),
            nombre: "Vieja".into(),
            numero: 1,
            proyecto: Some(chat.id.clone()),
            ..Default::default()
        };
        cuaderno::anadir(&carpeta, &m).unwrap();
        let archivo = vista::tabla_del_movil(&d, &chat.id, &m).unwrap();
        std::fs::create_dir_all(archivo.parent().unwrap()).unwrap();
        std::fs::write(
            &archivo,
            r#"{"nombre":"Vieja","celdas":{"A1":5,"A2":2.5,"A3":"=A1+A2"},"anchos":{"A":90.5}}"#,
        )
        .unwrap();
        let h = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert_eq!(h.contenido(0, 0), "5");
        let total = h
            .celdas(0, 5, 0, 2)
            .into_iter()
            .find(|c| c.fila == 2)
            .unwrap();
        assert_eq!(total.numero, Some(7.5));
        assert!(h.poner(3, 0, "x".into()));
        assert!(h.guardar().unwrap());
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
        assert_eq!(v["celdas"]["A1"], "5", "sale como texto, el formato de hoy");
        // Caso negativo: lo que no es un objeto JSON no se abre y sigue igual.
        std::fs::write(&archivo, "no es json").unwrap();
        assert!(
            n.abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&archivo).unwrap(),
            "no es json",
            "el fichero roto sigue tal cual"
        );
    }

    /// Un chat con un libro de dos hojas de verdad (`proyecto_con_libro` de
    /// `hoja_pin/pruebas.rs` de Windows).
    fn con_libro(n: &Nucleo, d: &std::path::Path) -> (String, cuaderno::Mensaje, PathBuf) {
        use pixpin_proyecto::tabla_xlsx::{HojaParaVer, a_xlsx};
        let tabla_de = |celdas: &[(&str, &str)]| {
            let mut t = Tabla::default();
            for (k, v) in celdas {
                t.poner(tabla::ref_de(k).unwrap(), v);
            }
            t
        };
        let chat = n.crear_chat("Obra".into()).unwrap();
        let carpeta = almacen::carpeta(d, &chat.id);
        let gastos = tabla_de(&[
            ("A1", "Concepto"),
            ("B1", "Importe"),
            ("A2", "Cemento"),
            ("B2", "120"),
            ("A3", "Arena"),
            ("B3", "80"),
            ("B4", "=SUMA(B2:B3)"),
        ]);
        let resumen = tabla_de(&[("A1", "Total"), ("B1", "200")]);
        let bytes = a_xlsx(
            &[
                HojaParaVer {
                    nombre: "Gastos",
                    tabla: &gastos,
                    valores: None,
                },
                HojaParaVer {
                    nombre: "Resumen",
                    tabla: &resumen,
                    valores: None,
                },
            ],
            1,
        )
        .unwrap();
        std::fs::create_dir_all(&carpeta).unwrap();
        let ruta = carpeta.join("Cuentas.xlsx");
        std::fs::write(&ruta, &bytes).unwrap();
        let sello = cuaderno::Sello {
            cuando: 1_700_000_000_000,
            numero: 1,
            aparato: "PC".into(),
            proyecto: chat.id.clone(),
        };
        let m = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "Cuentas.xlsx",
            "Cuentas.xlsx",
            bytes.len() as i64,
            &sello,
        );
        cuaderno::anadir(&carpeta, &m).unwrap();
        (chat.id, m, ruta)
    }

    #[test]
    fn un_excel_se_edita_como_libro_y_se_guarda_en_el_chat_sin_tocar_el_archivo() {
        let (d, n) = nucleo("excel");
        let (chat, m, ruta) = con_libro(&n, &d);
        let original = std::fs::read(&ruta).unwrap();
        assert!(n.es_hoja_de_calculo(chat.clone(), m.id.clone()));
        let h = n.abrir_hoja_de_calculo(chat.clone(), m.id.clone()).unwrap();
        // Todas las hojas, en pestanas, con sus formulas calculadas.
        assert_eq!(h.hojas(), vec!["Gastos", "Resumen"]);
        assert_eq!(h.activa(), 0);
        assert_eq!(h.ruta_original(), Some(ruta.display().to_string()));
        let total = h.celdas(3, 4, 1, 2).pop().unwrap();
        assert_eq!(total.numero, Some(200.0));
        // La primera vez se avisa de que el archivo no cambia; una sola.
        assert!(h.aviso_al_editar());
        assert!(!h.aviso_al_editar());
        h.cambiar_hoja(1);
        assert_eq!(h.nombre(), "Cuentas · Resumen");
        assert!(h.poner(0, 1, "9".into()));
        assert!(h.guardar().unwrap());
        assert!(!h.tocada());
        assert_eq!(
            std::fs::read(&ruta).unwrap(),
            original,
            "el .xlsx original no cambia"
        );
        let carpeta = almacen::carpeta(&d, &chat);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let hechas = libro::tablas_hechas(&c.mensajes, &m);
        assert_eq!(hechas.len(), 2);
        let resumen = &c.mensajes[hechas[1]];
        assert_eq!(
            resumen.referencia.as_deref(),
            Some(libro::id_de_hoja(&m, 1).as_str())
        );
        assert_eq!(
            Tabla::leer(&resumen.texto)
                .unwrap()
                .celda(tabla::ref_de("B1").unwrap()),
            "9"
        );
        assert!(
            libro::editada(resumen),
            "la tabla cambiada se reconoce como editada"
        );
        assert!(
            !libro::editada(&c.mensajes[hechas[0]]),
            "la que no se toco, no"
        );
        // Otro cambio va a SU mensaje, sin anadir otro.
        h.cambiar_hoja(0);
        assert!(h.poner(1, 0, "Grava".into()));
        assert!(h.guardar().unwrap());
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        assert_eq!(
            libro::tablas_hechas(&c.mensajes, &m).len(),
            2,
            "no se duplican"
        );
        assert_eq!(c.mensajes.len(), 3, "el libro y sus dos tablas");
        // Reabrir: sus tablas editadas, no el original.
        let otra = n.abrir_hoja_de_calculo(chat, m.id).unwrap();
        assert_eq!(otra.contenido(1, 0), "Grava");
        assert!(!otra.aviso_al_editar());
    }

    #[test]
    fn un_csv_se_abre_como_libro_de_una_hoja() {
        let (d, n) = nucleo("csv");
        let chat = n.crear_chat("Datos".into()).unwrap();
        let carpeta = almacen::carpeta(&d, &chat.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join("datos.csv"), "Nombre;Edad\nAna;31\n").unwrap();
        let sello = cuaderno::Sello {
            cuando: 5,
            numero: 1,
            aparato: "PC".into(),
            proyecto: chat.id.clone(),
        };
        let m = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "datos.csv",
            "datos.csv",
            20,
            &sello,
        );
        cuaderno::anadir(&carpeta, &m).unwrap();
        let h = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert_eq!(h.hojas().len(), 1);
        assert_eq!(h.contenido(1, 1), "31");
        // Caso negativo: un libro que no esta en este equipo no se abre.
        std::fs::remove_file(carpeta.join("datos.csv")).unwrap();
        assert!(!n.es_hoja_de_calculo(chat.id.clone(), m.id.clone()));
        assert!(matches!(
            n.abrir_hoja_de_calculo(chat.id, m.id),
            Err(ErrorNucleo::NoExiste { .. })
        ));
    }

    #[test]
    fn el_formato_de_un_rango_copiar_pegar_y_deshacer() {
        let (_d, n) = nucleo("rango");
        let chat = n.crear_chat("Cuentas".into()).unwrap();
        let m = n.crear_tabla(chat.id.clone(), "Gastos".into()).unwrap();
        let h = n.abrir_hoja_de_calculo(chat.id, m.id).unwrap();
        assert!(h.poner(0, 0, "1".into()));
        assert!(h.poner(1, 0, "2".into()));
        assert!(h.poner(2, 0, "=A1+A2".into()));
        // A1:B2 en negrita, centrado y con fondo: un paso de deshacer cada uno.
        h.elegir(SeleccionHoja {
            ancla_fila: 0,
            ancla_columna: 0,
            fila: 1,
            columna: 1,
        });
        assert_eq!(h.seleccion().columna, 1);
        assert!(h.formato(FormatoHoja::Negrita));
        assert!(
            h.estilo(1, 1).unwrap().negrita,
            "todo el rango, tambien las vacias"
        );
        assert!(h.formato(FormatoHoja::Alinear {
            alineacion: "c".into()
        }));
        assert!(h.formato(FormatoHoja::Fondo {
            color: Some(hoja_fondos()[0].hex.clone())
        }));
        assert!(h.formato(FormatoHoja::Numero {
            formato: Some("moneda".into())
        }));
        assert_eq!(
            h.estilo(0, 0).unwrap().formato_numero.as_deref(),
            Some("moneda")
        );
        assert!(h.deshacer());
        assert_eq!(
            h.estilo(0, 0).unwrap().formato_numero,
            None,
            "deshacer quita el ultimo"
        );
        assert!(h.rehacer());
        assert!(h.puede_deshacer() && !h.puede_rehacer());
        // Copiar: los valores con tabuladores y la tabla HTML.
        let copia = h.copiar();
        assert_eq!(copia.texto.lines().next(), Some("1\t"));
        assert!(copia.html.contains("<table"));
        // Pegar en C1 lo que viene de otra hoja (texto con tabuladores).
        h.elegir(SeleccionHoja {
            ancla_fila: 0,
            ancla_columna: 2,
            fila: 0,
            columna: 2,
        });
        assert!(h.pegar(None, Some("7\t8\n9\t10".into())));
        assert_eq!(h.contenido(1, 3), "10");
        assert_eq!(
            h.seleccion(),
            SeleccionHoja {
                ancla_fila: 0,
                ancla_columna: 2,
                fila: 1,
                columna: 3
            }
        );
        assert!(h.borrar_seleccion());
        assert_eq!(h.contenido(0, 2), "");
        assert!(h.deshacer(), "borrar se deshace de una vez");
        assert_eq!(h.contenido(0, 2), "7");
        let antes = h.cambios();
        assert!(h.deshacer());
        assert_eq!(h.contenido(1, 3), "", "y pegar tambien");
        assert!(h.cambios() > antes);
        // Casos negativos: una alineacion que no existe y un formato de numero
        // raro no hacen nada; pegar nada tampoco.
        assert!(!h.formato(FormatoHoja::Alinear {
            alineacion: "x".into()
        }));
        assert!(!h.formato(FormatoHoja::Numero {
            formato: Some("rarisimo".into())
        }));
        assert!(!h.pegar(None, None));
        assert_eq!(hoja_colores_de_letra()[0].nombre, "Rojo");
        assert_eq!(hoja_fondos().len(), 8);
    }
}
