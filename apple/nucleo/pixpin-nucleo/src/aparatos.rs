//! **Los aparatos del grupo**: quien hay, donde estaba la ultima vez, como
//! se llama este y como se sale (lo de «Este aparato» y «Tus aparatos» de
//! `sincronizar.rs` de Windows, copia de `SincronizarActivity.kt`).
//!
//! - **`sincro/direcciones.txt`**, el mismo formato que el movil y Windows:
//!   `id`, `host` y `puerto` separados por tabuladores, uno por linea. Lo
//!   visto en Bonjour se apunta; la proxima vez se prueba primero la
//!   direccion recordada (una sonda `PING` de poco mas de un segundo) y solo
//!   si no contesta se mira el anuncio de la red ([`donde_esta`], el
//!   `al_dia.rs` de Windows, puerto de `AlDia.aparatos` del movil).
//! - **Renombrar este aparato** (`Disco.renombrar`): el nombre cambia
//!   tambien en su ficha de miembro.
//! - **Salir del grupo** (`Disco.salirDelGrupo`): lo acordado con los demas
//!   se olvida (`sincro/base/`), y si vuelve a entrar la primera vuelta se
//!   hace como la primera vez. No se borra ningun chat.
//! - **Los miembros con «hace X»**: todos los del grupo, tambien los que no
//!   estan en la red, con la ultima vez que se sincronizo con cada uno
//!   (`sincro/elegidos/<id>.cuando`).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use pixpin_proyecto::identidad::Identidad;
use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::disco::Disco;

use crate::{ErrorNucleo, InfoAparato, Nucleo};

/// El tope del nombre de un aparato, el del movil.
pub const LARGO_DEL_NOMBRE: usize = 40;

// ------------------------------------------------------------- al dia

/// Un aparato visto en el anuncio de la red, con lo que importa aqui.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VistoEnRed {
    /// El `id` del TXT; puede venir vacio de versiones viejas.
    pub id: String,
    /// El `n` del TXT (su nombre).
    pub nombre: String,
    pub host: String,
    pub puerto: u16,
}

/// Donde llamar.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum DondeEsta {
    /// La de siempre contesto: se usa sin buscar, que buscar cuesta.
    LaDeSiempre,
    /// La de siempre no contesto y el anuncio lo da en otra: hay que
    /// apuntarla.
    Nueva { host: String, puerto: u16 },
    /// Ni contesta ni se anuncia: apagado o fuera de esta Wi-Fi.
    NoEsta,
}

/// **Decide donde llamar a un aparato** (`al_dia::donde_esta` de Windows).
/// `id` es el suyo si se sabe; sin el se reconoce por el nombre (como el
/// movil, que usa el nombre cuando el `id` del anuncio viene en blanco).
///
/// `contesta` solo se llama una vez y `buscar` solo si hace falta: con la
/// direccion buena no se sale a la red.
pub fn donde_esta(
    id: Option<&str>,
    nombre: &str,
    host: &str,
    puerto: u16,
    contesta: impl FnOnce(&str, u16) -> bool,
    buscar: impl FnOnce() -> Vec<VistoEnRed>,
) -> DondeEsta {
    if contesta(host, puerto) {
        return DondeEsta::LaDeSiempre;
    }
    let vistos = buscar();
    let es_el = |v: &&VistoEnRed| match id.filter(|i| !i.is_empty()) {
        // Con id, solo el id: dos aparatos pueden llamarse igual.
        Some(i) if !v.id.is_empty() => v.id == i,
        _ => !nombre.is_empty() && v.nombre == nombre,
    };
    match vistos.iter().find(es_el) {
        // Anunciado en la misma: la sonda fallo por otra cosa (estaba
        // ocupado atendiendo). Se intenta ahi, que es donde esta.
        Some(v) if v.host == host && v.puerto == puerto => DondeEsta::LaDeSiempre,
        Some(v) => DondeEsta::Nueva {
            host: v.host.clone(),
            puerto: v.puerto,
        },
        None => DondeEsta::NoEsta,
    }
}

/// [`donde_esta`] para Swift, que ya sabe si contesto (su sonda) y tiene
/// la lista de Bonjour a mano: `vistos` solo se mira si no contesto.
#[uniffi::export]
pub fn donde_esta_aparato(
    id: Option<String>,
    nombre: String,
    host: String,
    puerto: u16,
    contesta: bool,
    vistos: Vec<VistoEnRed>,
) -> DondeEsta {
    donde_esta(
        id.as_deref(),
        &nombre,
        &host,
        puerto,
        |_, _| contesta,
        || vistos,
    )
}

/// **«Hace X»** (`haceCuanto` del movil): sin horas exactas, que no hacen
/// falta para decidir. `cuando` y `ahora` en milisegundos UTC.
#[uniffi::export]
pub fn hace_cuanto(cuando: i64, ahora: i64) -> String {
    let min = (ahora - cuando).max(0) / 60_000;
    match min {
        0 => "ahora mismo".into(),
        1..=59 => format!("hace {min} min"),
        60..=1439 => format!("hace {} h", min / 60),
        1440..=2879 => "ayer".into(),
        _ => format!("hace {} días", min / 1440),
    }
}

// ------------------------------------------------- direcciones.txt

/// Una direccion recordada de un aparato.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DireccionRecordada {
    pub id: String,
    pub host: String,
    pub puerto: u16,
}

fn fichero_de_direcciones(raiz: &Path) -> PathBuf {
    raiz.join("sincro").join("direcciones.txt")
}

/// `sincro/direcciones.txt` con el formato del movil; lo roto se salta.
pub fn leer_direcciones_de(texto: &str) -> Vec<DireccionRecordada> {
    texto
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            let (id, host) = (p.next()?, p.next()?);
            let puerto = p.next()?.trim().parse().ok()?;
            Some(DireccionRecordada {
                id: id.to_string(),
                host: host.to_string(),
                puerto,
            })
        })
        .collect()
}

pub fn leer_direcciones(raiz: &Path) -> Vec<DireccionRecordada> {
    std::fs::read_to_string(fichero_de_direcciones(raiz))
        .map(|t| leer_direcciones_de(&t))
        .unwrap_or_default()
}

/// Apunta (o cambia) la direccion de `id`. Sin host o sin puerto no se
/// apunta nada. Devuelve si cambio el fichero: lo mismo otra vez no se
/// vuelve a escribir (Bonjour avisa a menudo).
pub fn apuntar_direccion(raiz: &Path, id: &str, host: &str, puerto: u16) -> std::io::Result<bool> {
    if puerto == 0 || host.is_empty() || id.is_empty() {
        return Ok(false);
    }
    let mut todas = leer_direcciones(raiz);
    if todas
        .iter()
        .any(|d| d.id == id && d.host == host && d.puerto == puerto)
    {
        return Ok(false);
    }
    todas.retain(|d| d.id != id);
    todas.push(DireccionRecordada {
        id: id.to_string(),
        host: host.to_string(),
        puerto,
    });
    let texto: String = todas
        .iter()
        .map(|d| format!("{}\t{}\t{}\n", d.id, d.host, d.puerto))
        .collect();
    let fichero = fichero_de_direcciones(raiz);
    if let Some(carpeta) = fichero.parent() {
        std::fs::create_dir_all(carpeta)?;
    }
    pixpin_sincro::disco::escribir_atomico(&fichero, texto.as_bytes())?;
    Ok(true)
}

/// **La sonda** (`Red.sondear`): un `PING` y esperar el `PONG`, con topes
/// cortos (1,2 s para llegar y 1,5 s para la respuesta) en vez de esperar
/// a que una conexion entera se agote.
pub fn sondear(host: &str, puerto: u16) -> bool {
    let Ok(dirs) = crate::red::direcciones(host, puerto) else {
        return false;
    };
    dirs.iter().any(|d| {
        let Ok(mut s) = TcpStream::connect_timeout(d, Duration::from_millis(1200)) else {
            return false;
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(1500)));
        if s.write_all(pixpin_sincro::SONDA).is_err() {
            return false;
        }
        let mut b = [0u8; 4];
        s.read_exact(&mut b).is_ok() && &b == pixpin_sincro::SONDA_RESPUESTA
    })
}

// ------------------------------------------------------------ miembros

/// Un aparato del grupo (no este), para la lista «Tus aparatos».
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MiembroGrupo {
    pub id: String,
    pub nombre: String,
    pub letra: Option<String>,
    /// La direccion recordada, si hay.
    pub direccion: Option<DireccionRecordada>,
    /// La ultima vez que se sincronizo con el (ms UTC), 0 si nunca.
    pub ultima_vez: i64,
}

#[uniffi::export]
impl Nucleo {
    /// **Los miembros del grupo menos este**, por nombre, con su direccion
    /// recordada y la ultima vez que se sincronizo con cada uno. Sin grupo,
    /// ninguno.
    pub fn miembros_del_grupo(&self) -> Result<Vec<MiembroGrupo>, ErrorNucleo> {
        let i = self.identidad()?;
        if i.codigo.is_none() {
            return Ok(Vec::new());
        }
        let dirs = leer_direcciones(self.ruta());
        let disco = DiscoPc::nuevo(self.ruta());
        let mut v: Vec<MiembroGrupo> = i
            .miembros
            .iter()
            .filter(|m| m.id != i.yo.id && !m.id.is_empty())
            .map(|m| MiembroGrupo {
                id: m.id.clone(),
                nombre: m.nombre.clone(),
                letra: m.letra.clone(),
                direccion: dirs.iter().find(|d| d.id == m.id).cloned(),
                ultima_vez: disco.ultima_vez(&m.id),
            })
            .collect();
        v.sort_by_key(|m| m.nombre.to_lowercase());
        Ok(v)
    }

    /// Las direcciones recordadas (`sincro/direcciones.txt`).
    pub fn direcciones_recordadas(&self) -> Vec<DireccionRecordada> {
        leer_direcciones(self.ruta())
    }

    /// **Apunta donde esta** el aparato `id` (lo que da la sonda de un
    /// aparato visto en Bonjour, o la direccion nueva de [`donde_esta`]).
    pub fn apuntar_direccion(
        &self,
        id: String,
        host: String,
        puerto: u16,
    ) -> Result<bool, ErrorNucleo> {
        Ok(apuntar_direccion(self.ruta(), &id, &host, puerto)?)
    }

    /// **La sonda `PING`/`PONG`** a una direccion escrita o recordada.
    /// Bloquea hasta unos 3 s como mucho.
    pub fn sondear_direccion(&self, host: String, puerto: u16) -> bool {
        sondear(&host, puerto)
    }

    /// **Renombra este aparato** (`Disco.renombrar`): tambien en su ficha
    /// de miembro, que es lo que ven los demas en la siguiente vuelta. Sin
    /// blancos alrededor y con el tope del movil (40); vacio no vale.
    pub fn renombrar_aparato(&self, nombre: String) -> Result<InfoAparato, ErrorNucleo> {
        let nombre: String = nombre.trim().chars().take(LARGO_DEL_NOMBRE).collect();
        let nombre = nombre.trim().to_string();
        if nombre.is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "el nombre del aparato no puede quedar vacio".into(),
            });
        }
        let mut actual = self.identidad()?;
        actual.yo.nombre = nombre;
        let yo = actual.yo.clone();
        for m in actual.miembros.iter_mut().filter(|m| m.id == yo.id) {
            *m = yo.clone();
        }
        actual.guardar(self.ruta())?;
        self.aparato()
    }

    /// **Sale del grupo** (`Disco.salirDelGrupo`): sin codigo, sin miembros
    /// y sin letra, y se olvida lo acordado con los demas (`sincro/base/`).
    /// No se borra ningun chat. Si vuelve a unirse, la primera vez
    /// preguntara por lo que haya cambiado en los dos lados.
    pub fn salir_del_grupo(&self) -> Result<InfoAparato, ErrorNucleo> {
        let actual = self.identidad()?;
        let mut yo = actual.yo.clone();
        yo.letra = None;
        yo.desde = 0;
        Identidad {
            yo,
            codigo: None,
            miembros: Vec::new(),
            resto: actual.resto,
        }
        .guardar(self.ruta())?;
        let base = self.ruta().join("sincro").join("base");
        if base.exists() {
            std::fs::remove_dir_all(base)?;
        }
        self.aparato()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    fn visto(id: &str, nombre: &str, host: &str, puerto: u16) -> VistoEnRed {
        VistoEnRed {
            id: id.into(),
            nombre: nombre.into(),
            host: host.into(),
            puerto,
        }
    }

    #[test]
    fn si_la_direccion_de_siempre_contesta_no_se_sale_a_buscar() {
        let buscado = Cell::new(false);
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "192.168.1.20",
            47474,
            |_, _| true,
            || {
                buscado.set(true);
                Vec::new()
            },
        );
        assert_eq!(d, DondeEsta::LaDeSiempre);
        assert!(!buscado.get(), "buscar por la red cuesta segundos");
    }

    #[test]
    fn si_cambio_de_ip_se_le_encuentra_por_el_anuncio_con_su_id() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "192.168.1.20",
            47474,
            |_, _| false,
            || {
                vec![
                    visto("otro", "Telefono", "192.168.1.99", 47474),
                    visto("tel", "Telefono", "192.168.1.33", 47475),
                ]
            },
        );
        assert_eq!(
            d,
            DondeEsta::Nueva {
                host: "192.168.1.33".into(),
                puerto: 47475
            }
        );
    }

    #[test]
    fn con_id_otro_aparato_que_se_llame_igual_no_vale() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "192.168.1.20",
            47474,
            |_, _| false,
            || vec![visto("otro", "Telefono", "192.168.1.99", 47474)],
        );
        assert_eq!(d, DondeEsta::NoEsta);
    }

    #[test]
    fn sin_id_se_le_reconoce_por_el_nombre() {
        let d = donde_esta(
            None,
            "Telefono",
            "10.0.0.2",
            47474,
            |_, _| false,
            || {
                vec![
                    visto("", "Portatil", "10.0.0.8", 47474),
                    visto("", "Telefono", "10.0.0.9", 47474),
                ]
            },
        );
        assert_eq!(
            d,
            DondeEsta::Nueva {
                host: "10.0.0.9".into(),
                puerto: 47474
            }
        );
    }

    #[test]
    fn apagado_o_fuera_de_la_wifi_no_esta() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "10.0.0.2",
            47474,
            |_, _| false,
            Vec::new,
        );
        assert_eq!(d, DondeEsta::NoEsta);
    }

    #[test]
    fn anunciado_en_la_misma_direccion_se_intenta_ahi_aunque_el_ping_fallara() {
        let d = donde_esta(
            Some("tel"),
            "Telefono",
            "10.0.0.2",
            47474,
            |_, _| false,
            || vec![visto("tel", "Telefono", "10.0.0.2", 47474)],
        );
        assert_eq!(d, DondeEsta::LaDeSiempre);
    }

    #[test]
    fn sin_id_ni_nombre_no_se_elige_a_cualquiera() {
        let d = donde_esta(
            None,
            "",
            "10.0.0.2",
            47474,
            |_, _| false,
            || vec![visto("", "", "10.0.0.9", 47474)],
        );
        assert_eq!(d, DondeEsta::NoEsta);
    }

    #[test]
    fn la_de_swift_no_mira_la_red_si_contesto() {
        let d = donde_esta_aparato(
            Some("tel".into()),
            "Telefono".into(),
            "10.0.0.2".into(),
            47474,
            true,
            vec![visto("tel", "Telefono", "10.0.0.9", 47474)],
        );
        assert_eq!(d, DondeEsta::LaDeSiempre);
    }

    #[test]
    fn hace_cuanto_habla_como_el_movil() {
        let h = 60_000;
        assert_eq!(hace_cuanto(0, 0), "ahora mismo");
        assert_eq!(hace_cuanto(0, 5 * h), "hace 5 min");
        assert_eq!(hace_cuanto(0, 120 * h), "hace 2 h");
        assert_eq!(hace_cuanto(0, 24 * 60 * h), "ayer");
        assert_eq!(hace_cuanto(0, 3 * 24 * 60 * h), "hace 3 días");
        // Caso negativo: un reloj adelantado no da «hace -3 min».
        assert_eq!(hace_cuanto(10 * h, 0), "ahora mismo");
    }

    #[test]
    fn las_direcciones_se_leen_con_el_formato_del_movil_y_lo_roto_se_salta() {
        let v = leer_direcciones_de("abc\t192.168.1.5\t47474\nroto\nx\t1.2.3.4\tnope\n");
        assert_eq!(
            v,
            vec![DireccionRecordada {
                id: "abc".into(),
                host: "192.168.1.5".into(),
                puerto: 47474
            }]
        );
    }

    fn carpeta(etiqueta: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-aparatos-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn apuntar_una_direccion_cambia_la_de_ese_aparato_y_deja_las_demas() {
        let raiz = carpeta("apuntar");
        assert!(apuntar_direccion(&raiz, "tel", "10.0.0.2", 47474).unwrap());
        assert!(apuntar_direccion(&raiz, "pc", "10.0.0.3", 47474).unwrap());
        // Lo mismo otra vez no reescribe.
        assert!(!apuntar_direccion(&raiz, "tel", "10.0.0.2", 47474).unwrap());
        assert!(apuntar_direccion(&raiz, "tel", "10.0.0.9", 47475).unwrap());
        let texto = std::fs::read_to_string(raiz.join("sincro/direcciones.txt")).unwrap();
        assert_eq!(texto, "pc\t10.0.0.3\t47474\ntel\t10.0.0.9\t47475\n");
        // Caso negativo: sin puerto o sin host no se apunta.
        assert!(!apuntar_direccion(&raiz, "x", "", 47474).unwrap());
        assert!(!apuntar_direccion(&raiz, "x", "10.0.0.1", 0).unwrap());
        assert_eq!(leer_direcciones(&raiz).len(), 2);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_sonda_a_donde_no_escucha_nadie_dice_que_no() {
        // Un puerto que se acaba de soltar: nadie contesta alli.
        let puerto = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert!(!sondear("127.0.0.1", puerto));
        assert!(!sondear("", puerto));
    }
}
