//! El almacen: la verdad de todo lo pineado (D21, D25, D27).
//!
//! Ficheros reales navegables con el Explorador mas un indice JSON que es
//! lo UNICO que se reescribe — siempre a temporal + rename, la disciplina
//! que el ExcalidrawStore del Android aprendio a golpes. Los objetos se
//! crean con contador y no se tocan jamas; solo "Eliminar del almacen"
//! (S2-B) los borrara.
//!
//! Este modulo es std::fs puro: se prueba entero con directorios
//! temporales, sin Windows.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ErrorAlmacen {
    #[error("no se pudo acceder a {1}: {0}")]
    Io(#[source] std::io::Error, PathBuf),
    #[error("el indice del almacen tiene un error: {0}")]
    Indice(#[from] serde_json::Error),
    #[error("no existe ninguna entrada con id {0}")]
    NoExiste(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinGuardado {
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
    /// DPI del monitor donde vivia, para restaurar con sentido (spec 5.2).
    pub escala_por_cien: u32,
    /// Zoom del texto de una nota (100 = el tamano con el que nacio). La
    /// rueda y Ctrl + arrastrar lo cambian; estirar la caja por la esquina
    /// no. Los demas tipos lo ignoran. Ausente en indices viejos: 100.
    #[serde(default = "cien")]
    pub zoom_por_cien: u32,
    /// Cuartos de vuelta a la derecha (0 a 3) y volteos, para que un pin
    /// girado vuelva girado. Ausentes en indices viejos: sin girar.
    #[serde(default)]
    pub giro: u8,
    #[serde(default)]
    pub volteo_h: bool,
    #[serde(default)]
    pub volteo_v: bool,
    /// Si el pin deja pasar los clics a lo que hay debajo (P1.4). Un pin
    /// asi se ve pero no se toca, para tener una referencia delante sin
    /// que estorbe. Ausente en indices viejos: se toca, como siempre.
    #[serde(default)]
    pub pasante: bool,
    /// Los filtros de imagen: gris, invertido y pasos de brillo. Ausentes
    /// en indices viejos: la imagen tal cual se capturo.
    #[serde(default)]
    pub gris: bool,
    #[serde(default)]
    pub invertido: bool,
    #[serde(default)]
    pub brillo: i32,
    /// La opacidad del pin en por ciento (rediseno v2). Ausente en indices
    /// viejos: opaco.
    #[serde(default = "cien_u8")]
    pub opacidad: u8,
}

fn cien_u8() -> u8 {
    100
}

fn cien() -> u32 {
    100
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoEntrada {
    Imagen,
    Nota,
    /// Por referencia, nunca copiado al almacen (D28).
    Archivo,
}

/// La paleta de grupos (D24): un grupo ES su color, no tiene nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorGrupo {
    Rojo,
    Naranja,
    Ambar,
    Verde,
    Cian,
    Azul,
    Violeta,
    Rosa,
}

impl ColorGrupo {
    /// Los ocho, en el orden de la paleta del diseno.
    pub const TODOS: [ColorGrupo; 8] = [
        ColorGrupo::Rojo,
        ColorGrupo::Naranja,
        ColorGrupo::Ambar,
        ColorGrupo::Verde,
        ColorGrupo::Cian,
        ColorGrupo::Azul,
        ColorGrupo::Violeta,
        ColorGrupo::Rosa,
    ];

    /// Indice en la paleta: como viaja el color hasta `pixpin-pin`, que no
    /// puede ver este crate (los dos son L2).
    pub fn indice(self) -> u8 {
        Self::TODOS.iter().position(|c| *c == self).unwrap_or(0) as u8
    }

    pub fn por_indice(i: u8) -> Option<ColorGrupo> {
        Self::TODOS.get(i as usize).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grupo {
    pub id: u32,
    pub color: ColorGrupo,
    #[serde(default)]
    pub oculto: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrada {
    pub id: u64,
    pub tipo: TipoEntrada,
    /// ISO-8601 UTC, texto: el indice se lee con un editor.
    pub creado: String,
    pub origen: String,
    /// Ruta relativa a `almacen/`. Vacia en las entradas por referencia.
    #[serde(default)]
    pub objeto: String,
    /// Ruta absoluta del fichero del usuario (solo `Archivo`, D28). Que
    /// apunte a algo que ya no existe es normal: se muestra como "no
    /// encontrado", no se oculta.
    #[serde(default)]
    pub ruta: Option<PathBuf>,
    pub grupo: Option<u32>,
    pub pin: Option<PinGuardado>,
}

/// El fichero indice.json entero. `#[serde(default)]` en todo: un indice de
/// una version futura abre igual (misma regla que los ajustes).
#[derive(Debug, Default, Serialize, Deserialize)]
struct Indice {
    #[serde(default = "version_uno")]
    version: u32,
    #[serde(default)]
    siguiente_id: u64,
    #[serde(default)]
    grupos: Vec<Grupo>,
    #[serde(default)]
    entradas: Vec<Entrada>,
}

fn version_uno() -> u32 {
    1
}

pub struct Almacen {
    dir: PathBuf,
    indice: Indice,
}

impl Almacen {
    pub fn abrir(raiz: &Path) -> Result<Almacen, ErrorAlmacen> {
        let dir = raiz.join("almacen");
        fs::create_dir_all(dir.join("objetos")).map_err(|e| ErrorAlmacen::Io(e, dir.clone()))?;
        let ruta = dir.join("indice.json");
        let indice = match fs::read_to_string(&ruta) {
            Ok(texto) => serde_json::from_str(&texto)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Indice {
                version: 1,
                siguiente_id: 1,
                grupos: Vec::new(),
                entradas: Vec::new(),
            },
            Err(e) => return Err(ErrorAlmacen::Io(e, ruta)),
        };
        Ok(Almacen { dir, indice })
    }

    pub fn entradas(&self) -> &[Entrada] {
        &self.indice.entradas
    }

    pub fn ruta_objeto(&self, e: &Entrada) -> PathBuf {
        self.dir.join(&e.objeto)
    }

    pub fn grupos(&self) -> &[Grupo] {
        &self.indice.grupos
    }

    pub fn grupo_de(&self, id_entrada: u64) -> Option<Grupo> {
        let g = self
            .indice
            .entradas
            .iter()
            .find(|e| e.id == id_entrada)?
            .grupo?;
        self.indice.grupos.iter().copied().find(|x| x.id == g)
    }

    /// Escribe un objeto propio del almacen (imagen o nota) y anota su
    /// entrada. Los objetos se escriben UNA vez y no se tocan mas.
    fn guardar_objeto(
        &mut self,
        bytes: &[u8],
        extension: &str,
        tipo: TipoEntrada,
        origen: &str,
        pin: Option<PinGuardado>,
    ) -> Result<u64, ErrorAlmacen> {
        self.guardar_objeto_con(extension, tipo, origen, pin, |ruta| fs::write(ruta, bytes))
    }

    /// Como `guardar_objeto`, con lo que escribe el fichero aparte (bytes en
    /// memoria o una copia de otro fichero).
    fn guardar_objeto_con(
        &mut self,
        extension: &str,
        tipo: TipoEntrada,
        origen: &str,
        pin: Option<PinGuardado>,
        escribir: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> Result<u64, ErrorAlmacen> {
        let id = self.indice.siguiente_id.max(1);
        self.indice.siguiente_id = id + 1;

        let (anio, mes) = anio_mes_utc();
        let relativa = format!("objetos/{anio:04}/{mes:02}/{id:06}.{extension}");
        let ruta = self.dir.join(&relativa);
        if let Some(padre) = ruta.parent() {
            fs::create_dir_all(padre).map_err(|e| ErrorAlmacen::Io(e, padre.to_path_buf()))?;
        }
        escribir(&ruta).map_err(|e| ErrorAlmacen::Io(e, ruta.clone()))?;

        self.indice.entradas.push(Entrada {
            id,
            tipo,
            creado: ahora_iso(),
            origen: origen.to_string(),
            objeto: relativa,
            ruta: None,
            grupo: None,
            pin,
        });
        self.persistir()?;
        Ok(id)
    }

    pub fn guardar_imagen(
        &mut self,
        png: &[u8],
        origen: &str,
        pin: Option<PinGuardado>,
    ) -> Result<u64, ErrorAlmacen> {
        self.guardar_objeto(png, "png", TipoEntrada::Imagen, origen, pin)
    }

    /// Una imagen que ya es un fichero (una foto abierta con PixPin): se
    /// copia TAL CUAL, con su extension. Recodificarla a PNG costaba mas que
    /// todo lo demas de abrirla (una foto de 12 MP son cientos de ms y un
    /// PNG cuatro veces mayor que el JPEG), y perdia su EXIF.
    pub fn guardar_imagen_de_fichero(
        &mut self,
        fichero: &Path,
        origen: &str,
        pin: Option<PinGuardado>,
    ) -> Result<u64, ErrorAlmacen> {
        let extension = extension_de_objeto(fichero);
        self.guardar_objeto_con(&extension, TipoEntrada::Imagen, origen, pin, |ruta| {
            fs::copy(fichero, ruta).map(|_| ())
        })
    }

    /// Una nota es un .txt UTF-8: se lee con el Bloc de notas (spec 5.1).
    pub fn guardar_nota(
        &mut self,
        texto: &str,
        origen: &str,
        pin: Option<PinGuardado>,
    ) -> Result<u64, ErrorAlmacen> {
        self.guardar_objeto(texto.as_bytes(), "txt", TipoEntrada::Nota, origen, pin)
    }

    /// Por referencia (D28): se anota la ruta, no se copia ni un byte.
    pub fn guardar_archivo(
        &mut self,
        ruta: &Path,
        pin: Option<PinGuardado>,
    ) -> Result<u64, ErrorAlmacen> {
        let id = self.indice.siguiente_id.max(1);
        self.indice.siguiente_id = id + 1;
        self.indice.entradas.push(Entrada {
            id,
            tipo: TipoEntrada::Archivo,
            creado: ahora_iso(),
            origen: "portapapeles".to_string(),
            objeto: String::new(),
            ruta: Some(ruta.to_path_buf()),
            grupo: None,
            pin,
        });
        self.persistir()?;
        Ok(id)
    }

    /// Asigna el color a la entrada. El color ES el grupo (D24): si ya hay
    /// un grupo de ese color se reutiliza. Devuelve el id del grupo, o None
    /// al quitarlo. Un grupo sin entradas desaparece del indice.
    pub fn poner_grupo(
        &mut self,
        id_entrada: u64,
        color: Option<ColorGrupo>,
    ) -> Result<Option<u32>, ErrorAlmacen> {
        if !self.indice.entradas.iter().any(|e| e.id == id_entrada) {
            return Err(ErrorAlmacen::NoExiste(id_entrada));
        }

        let nuevo = match color {
            None => None,
            Some(c) => Some(match self.indice.grupos.iter().find(|g| g.color == c) {
                Some(g) => g.id,
                None => {
                    let id = self.indice.grupos.iter().map(|g| g.id).max().unwrap_or(0) + 1;
                    self.indice.grupos.push(Grupo {
                        id,
                        color: c,
                        oculto: false,
                    });
                    id
                }
            }),
        };

        if let Some(e) = self.indice.entradas.iter_mut().find(|e| e.id == id_entrada) {
            e.grupo = nuevo;
        }
        self.purgar_grupos_vacios();
        self.persistir()?;
        Ok(nuevo)
    }

    pub fn poner_grupo_oculto(&mut self, id_grupo: u32, oculto: bool) -> Result<(), ErrorAlmacen> {
        if let Some(g) = self.indice.grupos.iter_mut().find(|g| g.id == id_grupo) {
            g.oculto = oculto;
        }
        self.persistir()
    }

    /// La UNICA accion destructiva (menu 4.3). Borra la entrada y, si el
    /// objeto es propiedad del almacen, tambien el fichero. Un archivo
    /// referenciado es del usuario: no se toca jamas.
    pub fn eliminar(&mut self, id_entrada: u64) -> Result<(), ErrorAlmacen> {
        let posicion = self
            .indice
            .entradas
            .iter()
            .position(|e| e.id == id_entrada)
            .ok_or(ErrorAlmacen::NoExiste(id_entrada))?;
        let entrada = self.indice.entradas.remove(posicion);
        if !entrada.objeto.is_empty() {
            let ruta = self.dir.join(&entrada.objeto);
            // Que el objeto ya no este no es un fallo: el indice manda, y el
            // usuario pudo borrarlo con el Explorador (la carpeta es suya).
            if let Err(e) = fs::remove_file(&ruta) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(ErrorAlmacen::Io(e, ruta));
                }
            }
        }
        self.purgar_grupos_vacios();
        self.persistir()
    }

    fn purgar_grupos_vacios(&mut self) {
        let entradas = &self.indice.entradas;
        self.indice
            .grupos
            .retain(|g| entradas.iter().any(|e| e.grupo == Some(g.id)));
    }

    pub fn actualizar_pin(
        &mut self,
        id: u64,
        pin: Option<PinGuardado>,
    ) -> Result<(), ErrorAlmacen> {
        let entrada = self
            .indice
            .entradas
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(ErrorAlmacen::NoExiste(id))?;
        entrada.pin = pin;
        self.persistir()
    }

    /// Temporal + rename: un proceso que muera a mitad deja el indice
    /// anterior intacto, nunca uno a medias que no abre.
    fn persistir(&self) -> Result<(), ErrorAlmacen> {
        let definitivo = self.dir.join("indice.json");
        let temporal = self.dir.join("indice.json.tmp");
        let texto = serde_json::to_string_pretty(&self.indice)?;
        fs::write(&temporal, texto).map_err(|e| ErrorAlmacen::Io(e, temporal.clone()))?;
        fs::rename(&temporal, &definitivo).map_err(|e| ErrorAlmacen::Io(e, definitivo))?;
        Ok(())
    }
}

/// (año, mes) actuales en UTC, sin dependencias: dias desde epoch con el
/// algoritmo civil de Howard Hinnant, suficiente y determinista.
fn anio_mes_utc() -> (i64, u32) {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let dias = segundos.div_euclid(86_400);
    let (anio, mes, _dia) = civil_desde_dias(dias);
    (anio, mes)
}

/// La extension con que se guarda la copia de una imagen: la suya, en
/// minusculas, si es corta y solo de letras y numeros; si no (una foto del
/// movil sin extension, un nombre raro), `img`. Quien la lee mira DENTRO
/// del fichero, no el nombre; la extension es para quien abra la carpeta.
pub fn extension_de_objeto(fichero: &Path) -> String {
    fichero
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty() && e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "img".to_string())
}

fn ahora_iso() -> String {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let dias = segundos.div_euclid(86_400);
    let resto = segundos.rem_euclid(86_400);
    let (a, m, d) = civil_desde_dias(dias);
    format!(
        "{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        resto / 3600,
        (resto % 3600) / 60,
        resto % 60
    )
}

/// Conversion dias-desde-epoch -> fecha civil (Hinnant, dominio ±millones
/// de años; aqui solo se usa con fechas reales).
fn civil_desde_dias(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn raiz(etiqueta: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-almacen-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Un PNG minimo valido no hace falta: el almacen guarda BYTES y no
    /// valida el formato (eso es del codec). Cuatro bytes bastan.
    const BYTES: &[u8] = &[0x89, b'P', b'N', b'G'];

    #[test]
    fn una_foto_abierta_se_copia_tal_cual_con_su_extension() {
        let dir = raiz("foto-copiada");
        let foto = dir.join("IMG_0001.JPG");
        fs::write(&foto, b"\xFF\xD8 jpeg de mentira").unwrap();
        let mut a = Almacen::abrir(&dir.join("almacen")).unwrap();
        let id = a
            .guardar_imagen_de_fichero(&foto, "abierta", Some(pin()))
            .unwrap();
        let e = a.entradas().iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.tipo, TipoEntrada::Imagen);
        assert!(e.objeto.ends_with(".jpg"), "{}", e.objeto);
        assert_eq!(
            fs::read(a.ruta_objeto(e)).unwrap(),
            b"\xFF\xD8 jpeg de mentira"
        );
        assert!(foto.exists(), "el original no se toca");
    }

    #[test]
    fn caso_negativo_una_extension_rara_o_ausente_se_guarda_como_img() {
        assert_eq!(extension_de_objeto(Path::new("C:/x/foto")), "img");
        assert_eq!(extension_de_objeto(Path::new("C:/x/foto.a b")), "img");
        assert_eq!(
            extension_de_objeto(Path::new("C:/x/foto.demasiadolarga")),
            "img"
        );
        assert_eq!(extension_de_objeto(Path::new("C:/x/foto.HEIC")), "heic");
        // Un fichero que no existe no deja entrada a medias.
        let dir = raiz("foto-que-no-esta");
        let mut a = Almacen::abrir(&dir).unwrap();
        assert!(
            a.guardar_imagen_de_fichero(&dir.join("no.jpg"), "x", None)
                .is_err()
        );
        assert!(a.entradas().is_empty());
    }

    fn pin() -> PinGuardado {
        PinGuardado {
            x: 100,
            y: 200,
            ancho: 300,
            alto: 150,
            escala_por_cien: 150,
            zoom_por_cien: 100,
            giro: 0,
            volteo_h: false,
            volteo_v: false,
            pasante: false,
            gris: false,
            invertido: false,
            brillo: 0,
            opacidad: 100,
        }
    }

    #[test]
    fn una_nota_va_y_vuelve_con_sus_acentos() {
        let r = raiz("nota");
        let mut a = Almacen::abrir(&r).unwrap();
        let id = a
            .guardar_nota("Señal de canción — ñandú", "portapapeles", Some(pin()))
            .unwrap();

        let e = a.entradas().iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.tipo, TipoEntrada::Nota);
        assert!(
            e.objeto.ends_with(".txt"),
            "la nota es un .txt: {}",
            e.objeto
        );
        let leido = fs::read_to_string(a.ruta_objeto(e)).unwrap();
        assert_eq!(leido, "Señal de canción — ñandú", "UTF-8 intacto");
    }

    #[test]
    fn un_archivo_se_guarda_por_referencia_y_nunca_se_copia() {
        // D28: pinear una carpeta de 40 GB no puede significar copiarla.
        let r = raiz("archivo");
        let ajeno = r.join("ajeno.pdf");
        fs::write(&ajeno, b"contenido ajeno").unwrap();
        let mut a = Almacen::abrir(&r).unwrap();

        let id = a.guardar_archivo(&ajeno, None).unwrap();

        let e = a.entradas().iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.tipo, TipoEntrada::Archivo);
        assert_eq!(e.ruta.as_deref(), Some(ajeno.as_path()));
        assert!(e.objeto.is_empty(), "un archivo no tiene objeto propio");
        assert!(
            !r.join("almacen/objetos").join("ajeno.pdf").exists(),
            "el fichero NO se copia al almacen"
        );
    }

    #[test]
    fn el_grupo_se_crea_una_vez_y_desaparece_al_quedarse_vacio() {
        let r = raiz("grupos");
        let mut a = Almacen::abrir(&r).unwrap();
        let uno = a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        let dos = a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();

        let g1 = a.poner_grupo(uno, Some(ColorGrupo::Rojo)).unwrap();
        let g2 = a.poner_grupo(dos, Some(ColorGrupo::Rojo)).unwrap();
        assert_eq!(
            g1, g2,
            "el mismo color es el mismo grupo: el color ES el grupo"
        );
        assert_eq!(a.grupos().len(), 1);

        // Caso negativo del recuento: quitar UNO no borra el grupo, porque
        // el otro sigue dentro. Borrarlo aqui perderia el color del segundo.
        a.poner_grupo(uno, None).unwrap();
        assert_eq!(a.grupos().len(), 1, "aun queda una entrada en el grupo");

        a.poner_grupo(dos, None).unwrap();
        assert!(a.grupos().is_empty(), "sin entradas, el grupo desaparece");
    }

    #[test]
    fn ocultar_un_grupo_persiste_y_conserva_los_pines() {
        let r = raiz("ocultar");
        let mut a = Almacen::abrir(&r).unwrap();
        let id = a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        let g = a.poner_grupo(id, Some(ColorGrupo::Verde)).unwrap().unwrap();

        a.poner_grupo_oculto(g, true).unwrap();

        let vuelta = Almacen::abrir(&r).unwrap();
        assert!(vuelta.grupos()[0].oculto, "el oculto sobrevive al reinicio");
        let e = vuelta.entradas().iter().find(|e| e.id == id).unwrap();
        assert_eq!(
            e.pin,
            Some(pin()),
            "ocultar conserva el rect: mostrar devuelve el pin a su sitio"
        );
    }

    #[test]
    fn eliminar_borra_la_entrada_y_su_objeto_pero_nunca_el_archivo_ajeno() {
        let r = raiz("eliminar");
        let ajeno = r.join("ajeno.txt");
        fs::write(&ajeno, b"no me toques").unwrap();
        let mut a = Almacen::abrir(&r).unwrap();
        let img = a.guardar_imagen(BYTES, "recorte", None).unwrap();
        let arch = a.guardar_archivo(&ajeno, None).unwrap();
        let ruta_objeto = a.ruta_objeto(a.entradas().iter().find(|e| e.id == img).unwrap());

        a.eliminar(img).unwrap();
        a.eliminar(arch).unwrap();

        assert!(a.entradas().is_empty());
        assert!(!ruta_objeto.exists(), "el objeto propio se borra");
        assert!(
            ajeno.exists(),
            "el archivo referenciado es del usuario: eliminar del almacen NO lo borra"
        );
    }

    #[test]
    fn un_indice_de_s2a_sigue_abriendo() {
        // Compatibilidad hacia atras: el indice que escribio S2-A no tiene
        // ni `grupos` ni `ruta`. Debe abrir sin perder nada.
        let r = raiz("compatible");
        let dir = r.join("almacen");
        fs::create_dir_all(dir.join("objetos")).unwrap();
        fs::write(
            dir.join("indice.json"),
            r#"{"version":1,"siguiente_id":8,"entradas":[
                {"id":7,"tipo":"imagen","creado":"2026-09-02T03:07:55Z","origen":"recorte",
                 "objeto":"objetos/2026/09/000007.png","grupo":null,
                 "pin":{"x":1,"y":2,"ancho":3,"alto":4,"escala_por_cien":150}}]}"#,
        )
        .unwrap();

        let a = Almacen::abrir(&r).unwrap();

        assert_eq!(a.entradas().len(), 1);
        assert!(a.grupos().is_empty());
        assert_eq!(a.entradas()[0].ruta, None);
        assert_eq!(a.entradas()[0].pin.unwrap().ancho, 3);
    }

    #[test]
    fn guardar_crea_objeto_e_indice_y_sobrevive_a_reabrir() {
        let r = raiz("basico");
        let id = {
            let mut a = Almacen::abrir(&r).unwrap();
            a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap()
        };
        // Reabrir desde disco: nada vive solo en memoria (D25).
        let a = Almacen::abrir(&r).unwrap();
        let e = a
            .entradas()
            .iter()
            .find(|e| e.id == id)
            .expect("la entrada persiste");
        assert_eq!(e.tipo, TipoEntrada::Imagen);
        assert_eq!(e.origen, "recorte");
        assert_eq!(e.pin, Some(pin()));
        // El objeto es un fichero real navegable (D27).
        assert_eq!(fs::read(a.ruta_objeto(e)).unwrap(), BYTES);
    }

    #[test]
    fn cerrar_un_pin_no_borra_nada() {
        // D21 como test: actualizar a None conserva entrada y objeto.
        let r = raiz("cerrar");
        let mut a = Almacen::abrir(&r).unwrap();
        let id = a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        a.actualizar_pin(id, None).unwrap();

        let a2 = Almacen::abrir(&r).unwrap();
        let e = a2.entradas().iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.pin, None, "cerrado");
        assert!(a2.ruta_objeto(e).is_file(), "el contenido sigue ahi");
    }

    #[test]
    fn los_ids_y_los_objetos_nunca_se_reutilizan() {
        // Caso negativo del contador: dos guardados dan ids y rutas
        // distintos aunque el primero se "cierre" entre medias.
        let r = raiz("contador");
        let mut a = Almacen::abrir(&r).unwrap();
        let id1 = a.guardar_imagen(BYTES, "recorte", None).unwrap();
        a.actualizar_pin(id1, None).unwrap();
        let id2 = a.guardar_imagen(BYTES, "recorte", None).unwrap();
        assert_ne!(id1, id2);
        let e1 = a
            .entradas()
            .iter()
            .find(|e| e.id == id1)
            .unwrap()
            .objeto
            .clone();
        let e2 = a
            .entradas()
            .iter()
            .find(|e| e.id == id2)
            .unwrap()
            .objeto
            .clone();
        assert_ne!(e1, e2, "cada objeto tiene su fichero");
    }

    #[test]
    fn actualizar_un_id_inexistente_da_error() {
        let r = raiz("no-existe");
        let mut a = Almacen::abrir(&r).unwrap();
        assert!(matches!(
            a.actualizar_pin(999, None),
            Err(ErrorAlmacen::NoExiste(999))
        ));
    }

    #[test]
    fn un_indice_con_claves_desconocidas_abre_igual() {
        // La regla de compatibilidad de los ajustes, aplicada al indice: un
        // fichero escrito por una version futura no impide arrancar.
        let r = raiz("futuro");
        {
            let mut a = Almacen::abrir(&r).unwrap();
            a.guardar_imagen(BYTES, "recorte", None).unwrap();
        }
        let ruta = r.join("almacen").join("indice.json");
        let texto = fs::read_to_string(&ruta).unwrap().replacen(
            "\"version\":",
            "\"funcion_del_futuro\": 42, \"version\":",
            1,
        );
        fs::write(&ruta, texto).unwrap();
        assert!(Almacen::abrir(&r).is_ok());
    }

    #[test]
    fn el_indice_se_escribe_por_temporal_mas_rename() {
        // No queda ningun .tmp tras operar: si quedara, la escritura no fue
        // atomica o el rename fallo en silencio.
        let r = raiz("atomico");
        let mut a = Almacen::abrir(&r).unwrap();
        a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        let sobras: Vec<_> = fs::read_dir(r.join("almacen"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(sobras.is_empty(), "quedo un temporal: {sobras:?}");
    }

    #[test]
    fn la_opacidad_se_guarda_y_un_indice_viejo_vuelve_opaco() {
        // Caso negativo: un pin guardado antes del rediseno no trae
        // opacidad, y tiene que volver opaco, no invisible.
        let viejo = r#"{"x":1,"y":2,"ancho":3,"alto":4,"escala_por_cien":100}"#;
        let g: PinGuardado = serde_json::from_str(viejo).unwrap();
        assert_eq!(g.opacidad, 100);
        let medio = PinGuardado {
            opacidad: 60,
            ..pin()
        };
        let texto = serde_json::to_string(&medio).unwrap();
        let vuelta: PinGuardado = serde_json::from_str(&texto).unwrap();
        assert_eq!(vuelta.opacidad, 60);
    }
}
