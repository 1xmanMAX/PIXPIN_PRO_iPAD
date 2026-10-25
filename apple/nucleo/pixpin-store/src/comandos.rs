//! El registro de comandos: una fila por cada cosa que el programa sabe
//! hacer.
//!
//! Antes cada atajo era una constante, una rama del `match` del bucle y una
//! entrada escrita a mano en el menu de la bandeja; anadir una funcion
//! costaba tocar cuatro sitios y era facil olvidarse de uno. El PixPin
//! original lo resuelve con un registro —nombre estable, titulo traducido,
//! atajo opcional y marca de si sale en la bandeja— y esa idea es la que se
//! copia aqui (ver `docs/investigacion/2026-09-03-pixpin-original-estructura.md`).
//!
//! Reglas que sostienen todo lo demas:
//!
//! - **Todo comando del catalogo hace algo.** No se anaden filas «para
//!   luego»: un atajo que no responde se vive como una averia. Anadir una
//!   funcion es anadir su fila Y atenderla.
//! - **El nombre es el contrato con el fichero de ajustes.** El titulo se
//!   traduce y el atajo se cambia; el nombre, no.
//! - **El identificador numerico sale de la posicion en el catalogo**, y lo
//!   comparten `RegisterHotKey` y el menu de la bandeja: un solo espacio de
//!   numeros para las dos vias.

use std::collections::BTreeMap;

use pixpin_sistema::Atajo;
use serde::{Deserialize, Serialize};

/// Cada cosa que el programa sabe hacer y se puede atar a un atajo o sacar
/// en la bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Comando {
    /// Capturar una region y ensenar la barra de resultado.
    CapturarRegion,
    /// Capturar y copiar al portapapeles, sin confirmacion.
    CapturarYCopiar,
    /// Captura larga con scroll.
    CapturarConScroll,
    /// Capturar y entrar directo a anotar, sin pasar por la barra.
    CapturarYAnotar,
    /// Repetir la ultima region capturada, sin overlay ni preguntas.
    CapturarUltimaRegion,
    /// Grabar una zona de la pantalla como GIF animado.
    GrabarGif,
    /// Cuentagotas: copiar el color bajo el cursor.
    Cuentagotas,
    /// Recortar y dejarlo flotando como pin.
    Pinear,
    /// Elegir una zona y dejarla flotando como pin en vivo.
    PinearEnVivo,
    /// Pinear lo que haya en el portapapeles.
    PinearPortapapeles,
    /// Anotar sobre la pantalla en vivo.
    Anotar,
    /// Anotar sobre una captura estatica de la pantalla.
    AnotarCongelada,
    /// Cerrar todos los pines de la pantalla.
    CerrarTodosLosPines,
    /// Quitar los pines de la pantalla, o devolverlos si ya no estan.
    AlternarPines,
    /// Que los pines dejen pasar el clic a lo que hay debajo, o que
    /// vuelvan a recogerlo (P1.4).
    AlternarPasoDeClics,
    /// Soltar todos los atajos globales un rato, y volver a tomarlos
    /// (P1.7).
    SilenciarAtajos,
    /// Capturar despues de una cuenta atras, para poder dejar desplegado
    /// un menu que se cerraria al perder el foco (P2.1).
    CapturarConRetardo,
    /// Pinear lo que este seleccionado ahora mismo en el Explorador o en
    /// el escritorio, sin pasar por el portapapeles (P1.6).
    PinearSeleccion,
    /// Devolver a la pantalla el ultimo pin que se cerro.
    RestaurarUltimoPin,
    /// Capturar una region y copiar el texto que se lea en ella.
    CopiarTexto,
    /// Fijar encima de todo la ventana que haya bajo el raton, o bajarla.
    VentanaEncima,
    /// Abrir la ventana de chat: los proyectos como conversaciones.
    AbrirChat,
    /// Abrir la ventana de ajustes.
    AbrirAjustes,
    /// Esperar a que el movil mande algo por la wifi (D147). Ya no sale en
    /// la bandeja —recibir cuelga de Sincronizar, como en el movil—, pero el
    /// comando sigue para quien lo tenga atado a un atajo en su TOML.
    RecibirDelMovil,
    /// Cerrar el programa.
    Salir,
    /// Abrir Sincronizar: tus aparatos, y recibir y enviar por Wi-Fi.
    Sincronizar,
    /// Buscar en PixPin (rediseno v2): archivos, tareas, lecciones y
    /// capturas, con lo mismo que el plugin de Flow. Nace sin atajo.
    Buscar,
    /// Grabar una clase o una reunion (24-oct-2026): el microfono y el
    /// sonido del equipo, con capturas de una zona en su minuto, y despues
    /// la transcripcion. Nace sin atajo.
    GrabarClase,
}

/// La ficha de un comando en el catalogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    pub comando: Comando,
    /// Clave estable para el fichero de ajustes. No cambia nunca.
    pub nombre: &'static str,
    /// Clave del catalogo de idiomas con el titulo que ve el usuario.
    pub clave_titulo: &'static str,
    /// Atajo con el que nace, o `None` si nace sin atajo.
    pub atajo_por_defecto: Option<&'static str>,
    /// Si aparece como entrada del menu de la bandeja.
    pub en_bandeja: bool,
}

/// Todos los comandos, en el orden en que salen en la bandeja. La posicion
/// manda: de ella sale el identificador numerico.
pub const CATALOGO: &[Descriptor] = &[
    Descriptor {
        comando: Comando::CapturarRegion,
        nombre: "capturar-region",
        clave_titulo: "comando-capturar-region",
        // El UNICO atajo de fabrica (D140): lleva a la seleccion con la
        // barra, y ahi se elige que hacer con la captura. Lo rapido va por
        // los gestos con Alt; demasiados atajos le estorbaban al usuario.
        atajo_por_defecto: Some("Ctrl+Alt+X"),
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::CapturarYCopiar,
        nombre: "capturar-y-copiar",
        clave_titulo: "comando-capturar-y-copiar",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::CapturarConScroll,
        nombre: "capturar-con-scroll",
        clave_titulo: "comando-capturar-con-scroll",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::CapturarYAnotar,
        nombre: "capturar-y-anotar",
        clave_titulo: "comando-capturar-y-anotar",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::GrabarGif,
        nombre: "grabar-gif",
        clave_titulo: "comando-grabar-gif",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::CapturarUltimaRegion,
        nombre: "capturar-ultima-region",
        clave_titulo: "comando-capturar-ultima-region",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::Cuentagotas,
        nombre: "cuentagotas",
        clave_titulo: "comando-cuentagotas",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::Pinear,
        nombre: "pinear",
        clave_titulo: "comando-pinear",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::PinearEnVivo,
        nombre: "pinear-en-vivo",
        clave_titulo: "comando-pinear-en-vivo",
        // Sin atajo: el usuario no quiere que PixPin se quede combinaciones
        // que no pidio (D81). Sale en la bandeja y se ata en los ajustes.
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::PinearPortapapeles,
        nombre: "pinear-portapapeles",
        clave_titulo: "comando-pinear-portapapeles",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::Anotar,
        nombre: "anotar",
        clave_titulo: "comando-anotar",
        atajo_por_defecto: None,
        // Desde la bandeja: es el anotador de pantalla (el editor encima del
        // escritorio). Sin atajo de fabrica, como todo salvo el general.
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::AnotarCongelada,
        nombre: "anotar-congelada",
        clave_titulo: "comando-anotar-congelada",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::PinearSeleccion,
        nombre: "pinear-seleccion",
        clave_titulo: "comando-pinear-seleccion",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::CapturarConRetardo,
        nombre: "capturar-con-retardo",
        clave_titulo: "comando-capturar-con-retardo",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::SilenciarAtajos,
        nombre: "silenciar-atajos",
        clave_titulo: "comando-silenciar-atajos",
        // Sin atajo de fabrica y SIEMPRE en la bandeja, por lo mismo que
        // el paso de clics: al silenciar se sueltan todos los atajos, el
        // suyo incluido, asi que la vuelta no puede ser por teclado.
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::AlternarPasoDeClics,
        nombre: "alternar-paso-de-clics",
        clave_titulo: "comando-alternar-paso-de-clics",
        // Sin atajo de fabrica, pero SIEMPRE en la bandeja: es la unica
        // via de vuelta. Un pin que deja pasar el clic no puede abrir su
        // propio menu, porque el clic derecho pasa de largo.
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::AlternarPines,
        nombre: "alternar-pines",
        clave_titulo: "comando-alternar-pines",
        // Lo pidio el usuario el 2026-09-22: «ocultar pines en pantalla con
        // Control 2, y Control 2 para mostrar de nuevo». Es la excepcion a
        // «nada nace con atajo»; el precio sabido es que Ctrl+2 deja de
        // llegar a los programas que lo usan (la pestana 2 del navegador).
        atajo_por_defecto: Some("Ctrl+2"),
        en_bandeja: true,
    },
    // Los otros dos de pines nacen SIN atajo: son de uso ocasional y meter
    // combinaciones nuevas por defecto es la forma mas rapida de pisar las
    // de otro programa.
    Descriptor {
        comando: Comando::RestaurarUltimoPin,
        nombre: "restaurar-ultimo-pin",
        clave_titulo: "comando-restaurar-ultimo-pin",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::CerrarTodosLosPines,
        nombre: "cerrar-todos-los-pines",
        clave_titulo: "comando-cerrar-todos-los-pines",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::CopiarTexto,
        nombre: "copiar-texto",
        clave_titulo: "comando-copiar-texto",
        // Sin atajo de fabrica (D140): el OCR sigue en la bandeja y en la
        // barra de la seleccion, que es donde se elige que hacer.
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::VentanaEncima,
        nombre: "ventana-encima",
        clave_titulo: "comando-ventana-encima",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::AbrirChat,
        nombre: "abrir-chat",
        clave_titulo: "comando-abrir-chat",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    Descriptor {
        comando: Comando::AbrirAjustes,
        nombre: "abrir-ajustes",
        clave_titulo: "comando-abrir-ajustes",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    // Va al final del catalogo a proposito: la posicion manda sobre el
    // identificador, y meterlo en medio correria los de todos los demas —y
    // con ellos los atajos que la gente tenga escritos en su TOML—.
    Descriptor {
        comando: Comando::RecibirDelMovil,
        nombre: "recibir-del-movil",
        clave_titulo: "comando-recibir-del-movil",
        atajo_por_defecto: None,
        en_bandeja: false,
    },
    Descriptor {
        comando: Comando::Salir,
        nombre: "salir",
        clave_titulo: "comando-salir",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    // Al final por lo mismo que «Recibir del movil»: meterlo en medio
    // correria el numero de todos los que vienen detras. En el menu no sale
    // aqui sino bajo el chat: lo coloca `acciones_de_bandeja`.
    Descriptor {
        comando: Comando::Sincronizar,
        nombre: "sincronizar",
        clave_titulo: "comando-sincronizar",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    // Al final, como Sincronizar: meterlo en medio correria los numeros.
    Descriptor {
        comando: Comando::Buscar,
        nombre: "buscar",
        clave_titulo: "comando-buscar",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
    // Al final, como Buscar: meterlo en medio correria los numeros.
    Descriptor {
        comando: Comando::GrabarClase,
        nombre: "grabar-clase",
        clave_titulo: "comando-grabar-clase",
        atajo_por_defecto: None,
        en_bandeja: true,
    },
];

impl Comando {
    /// La posicion en el catalogo, que es de donde sale el identificador.
    fn indice(self) -> usize {
        CATALOGO
            .iter()
            .position(|d| d.comando == self)
            .expect("todo Comando esta en el catalogo; lo vigila una prueba")
    }

    pub fn descriptor(self) -> &'static Descriptor {
        &CATALOGO[self.indice()]
    }

    pub fn nombre(self) -> &'static str {
        self.descriptor().nombre
    }

    /// El numero con el que viaja por Win32. Empieza en 1 porque el cero no
    /// distingue de «sin identificador» en un `WM_COMMAND`.
    pub fn id(self) -> u32 {
        self.indice() as u32 + 1
    }

    pub fn desde_id(id: u32) -> Option<Comando> {
        let i = (id as usize).checked_sub(1)?;
        CATALOGO.get(i).map(|d| d.comando)
    }

    pub fn desde_nombre(nombre: &str) -> Option<Comando> {
        CATALOGO
            .iter()
            .find(|d| d.nombre == nombre)
            .map(|d| d.comando)
    }
}

/// Que atajo tiene cada comando ahora mismo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enlaces {
    /// Paralelo a `CATALOGO`.
    atajos: Vec<Option<Atajo>>,
}

impl Default for Enlaces {
    fn default() -> Self {
        Self {
            atajos: CATALOGO
                .iter()
                .map(|d| {
                    d.atajo_por_defecto.map(|a| {
                        a.parse()
                            .expect("los atajos por defecto son constantes de este fichero")
                    })
                })
                .collect(),
        }
    }
}

impl Enlaces {
    pub fn atajo_de(&self, c: Comando) -> Option<Atajo> {
        self.atajos[c.indice()]
    }

    pub fn poner(&mut self, c: Comando, atajo: Option<Atajo>) {
        self.atajos[c.indice()] = atajo;
    }

    /// Los que hay que registrar de verdad en el sistema.
    pub fn registrables(&self) -> Vec<(u32, Atajo)> {
        CATALOGO
            .iter()
            .filter_map(|d| self.atajo_de(d.comando).map(|a| (d.comando.id(), a)))
            .collect()
    }

    /// Dos comandos con el mismo atajo. El segundo nunca llegaria a
    /// dispararse, asi que conviene avisar en vez de dejarlo pasar.
    pub fn choque(&self) -> Option<(Comando, Comando)> {
        for (i, a) in CATALOGO.iter().enumerate() {
            let Some(atajo_a) = self.atajo_de(a.comando) else {
                continue;
            };
            for b in CATALOGO.iter().skip(i + 1) {
                if self.atajo_de(b.comando) == Some(atajo_a) {
                    return Some((a.comando, b.comando));
                }
            }
        }
        None
    }

    /// Aplica lo que diga la tabla `[comandos]` del fichero de ajustes.
    /// Devuelve los nombres que no reconoce, para dejarlos en el registro:
    /// un fichero escrito por una version mas nueva no debe impedir
    /// arrancar, pero tampoco callarse.
    pub fn aplicar_tabla(&mut self, tabla: &BTreeMap<String, String>) -> Vec<String> {
        let mut desconocidos = Vec::new();
        for (nombre, texto) in tabla {
            match Comando::desde_nombre(nombre) {
                // Cadena vacia: el usuario quiere ese comando SIN atajo. Es
                // distinto de no nombrarlo, que deja el de por defecto.
                Some(c) if texto.trim().is_empty() => self.poner(c, None),
                Some(c) => match texto.parse() {
                    Ok(a) => self.poner(c, Some(a)),
                    Err(_) => desconocidos.push(format!("{nombre} = {texto}")),
                },
                None => desconocidos.push(nombre.clone()),
            }
        }
        desconocidos
    }

    /// Los enlaces que salen de un fichero de ajustes, en tres capas: los
    /// de por defecto, encima la tabla vieja `[atajos]` y encima la nueva
    /// `[comandos]`. Asi un fichero de antes sigue valiendo, y quien ya use
    /// la tabla nueva manda sobre la vieja.
    pub fn de_ajustes(a: &crate::ajustes::Ajustes) -> (Enlaces, Vec<String>) {
        let mut e = Enlaces::default();
        let v = &a.atajos;
        for (comando, atajo) in [
            (Comando::CapturarRegion, v.region),
            (Comando::CapturarYCopiar, v.copiar),
            (Comando::CapturarConScroll, v.scroll),
            (Comando::Cuentagotas, v.cuentagotas),
            (Comando::Pinear, v.pin),
            (Comando::PinearPortapapeles, v.portapapeles),
            (Comando::Anotar, v.anotar),
            (Comando::AnotarCongelada, v.anotar_congelada),
        ] {
            e.poner(comando, atajo);
        }
        let avisos = e.aplicar_tabla(&a.comandos);
        (e, avisos)
    }

    /// Como se escriben en el fichero de ajustes.
    pub fn a_tabla(&self) -> BTreeMap<String, String> {
        CATALOGO
            .iter()
            .map(|d| {
                let valor = self
                    .atajo_de(d.comando)
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                (d.nombre.to_string(), valor)
            })
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Todos los comandos que existen, para poder recorrerlos en las
    /// pruebas. Si se anade uno y no se anade aqui, falla la prueba de
    /// cobertura de abajo.
    const TODOS: &[Comando] = &[
        Comando::CapturarRegion,
        Comando::CapturarYCopiar,
        Comando::CapturarConScroll,
        Comando::CapturarYAnotar,
        Comando::CapturarUltimaRegion,
        Comando::GrabarGif,
        Comando::Cuentagotas,
        Comando::Pinear,
        Comando::PinearEnVivo,
        Comando::PinearPortapapeles,
        Comando::Anotar,
        Comando::AnotarCongelada,
        Comando::AlternarPines,
        Comando::AlternarPasoDeClics,
        Comando::SilenciarAtajos,
        Comando::CapturarConRetardo,
        Comando::PinearSeleccion,
        Comando::RestaurarUltimoPin,
        Comando::CerrarTodosLosPines,
        Comando::CopiarTexto,
        Comando::VentanaEncima,
        Comando::AbrirChat,
        Comando::AbrirAjustes,
        Comando::RecibirDelMovil,
        Comando::Salir,
        Comando::Sincronizar,
        Comando::Buscar,
        Comando::GrabarClase,
    ];

    #[test]
    fn sincronizar_sale_en_la_bandeja_y_recibir_suelto_ya_no() {
        // Como en el movil: recibir y enviar cuelgan de Sincronizar.
        assert!(Comando::Sincronizar.descriptor().en_bandeja);
        assert!(!Comando::RecibirDelMovil.descriptor().en_bandeja);
        // Caso negativo: quitarlo de la bandeja no lo borra del catalogo, y
        // un TOML que lo nombre sigue valiendo.
        assert_eq!(
            Comando::desde_nombre("recibir-del-movil"),
            Some(Comando::RecibirDelMovil)
        );
        assert_eq!(
            Comando::desde_nombre("sincronizar"),
            Some(Comando::Sincronizar)
        );
    }

    #[test]
    fn el_catalogo_cubre_todos_los_comandos_una_sola_vez() {
        assert_eq!(CATALOGO.len(), TODOS.len(), "sobra o falta una fila");
        for c in TODOS {
            let cuantas = CATALOGO.iter().filter(|d| d.comando == *c).count();
            assert_eq!(cuantas, 1, "{c:?} aparece {cuantas} veces");
        }
    }

    #[test]
    fn los_nombres_y_las_claves_de_titulo_son_unicos() {
        // El nombre es el contrato con el fichero de ajustes: dos iguales
        // harian que uno pisara al otro en silencio.
        for (i, a) in CATALOGO.iter().enumerate() {
            for b in CATALOGO.iter().skip(i + 1) {
                assert_ne!(a.nombre, b.nombre, "nombre repetido: {}", a.nombre);
                assert_ne!(
                    a.clave_titulo, b.clave_titulo,
                    "titulo repetido: {}",
                    a.clave_titulo
                );
            }
        }
    }

    #[test]
    fn los_nombres_son_estables_y_legibles() {
        // Van a un fichero que el usuario edita a mano: minusculas, guiones
        // y nada mas.
        for d in CATALOGO {
            assert!(
                !d.nombre.is_empty()
                    && d.nombre.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "nombre no valido: {}",
                d.nombre
            );
        }
    }

    #[test]
    fn ningun_comando_nace_pisando_el_atajo_de_otro() {
        assert_eq!(Enlaces::default().choque(), None);
    }

    #[test]
    fn el_identificador_va_y_vuelve() {
        for c in TODOS {
            assert_eq!(Comando::desde_id(c.id()), Some(*c));
            assert!(c.id() >= 1, "el cero no vale como identificador");
        }
        // Caso negativo: fuera de rango no inventa un comando.
        assert_eq!(Comando::desde_id(0), None);
        assert_eq!(Comando::desde_id(CATALOGO.len() as u32 + 1), None);
    }

    #[test]
    fn los_identificadores_no_chocan_con_los_grupos_ocultos() {
        // La bandeja usa 200 en adelante para los grupos; los comandos
        // tienen que quedarse muy por debajo.
        assert!(
            CATALOGO.len() < 200,
            "el catalogo invade el espacio de los grupos"
        );
    }

    #[test]
    fn la_tabla_del_fichero_va_y_vuelve() {
        let mut e = Enlaces::default();
        e.poner(Comando::Cuentagotas, Some("Ctrl+Shift+F9".parse().unwrap()));
        e.poner(Comando::Pinear, None);

        let tabla = e.a_tabla();
        let mut vuelta = Enlaces::default();
        assert!(vuelta.aplicar_tabla(&tabla).is_empty());
        assert_eq!(vuelta, e);
    }

    #[test]
    fn una_cadena_vacia_deja_el_comando_sin_atajo() {
        // Distinto de no nombrarlo: no nombrarlo conserva el de por defecto.
        let mut e = Enlaces::default();
        e.poner(Comando::Pinear, Some("Ctrl+Shift+F2".parse().unwrap()));
        assert!(e.atajo_de(Comando::CapturarRegion).is_some());
        let tabla = BTreeMap::from([("capturar-region".to_string(), String::new())]);
        assert!(e.aplicar_tabla(&tabla).is_empty());
        assert_eq!(e.atajo_de(Comando::CapturarRegion), None);
        assert!(
            e.atajo_de(Comando::Pinear).is_some(),
            "los que no se nombran no se tocan"
        );
    }

    #[test]
    fn lo_que_no_se_reconoce_se_ignora_pero_se_avisa() {
        // Un fichero escrito por una version mas nueva no debe impedir
        // arrancar; tampoco debe desaparecer sin dejar rastro.
        let mut e = Enlaces::default();
        let tabla = BTreeMap::from([
            ("funcion-del-futuro".to_string(), "Ctrl+Alt+Z".to_string()),
            ("pinear".to_string(), "NoEsUnAtajo".to_string()),
        ]);
        let avisos = e.aplicar_tabla(&tabla);
        assert_eq!(avisos.len(), 2, "los dos casos se avisan: {avisos:?}");
        assert_eq!(
            e.atajo_de(Comando::Pinear),
            Enlaces::default().atajo_de(Comando::Pinear),
            "un atajo ilegible deja el de antes, no borra el comando"
        );
    }

    #[test]
    fn solo_se_registran_los_que_tienen_atajo() {
        let e = Enlaces::default();
        let ids: Vec<u32> = e.registrables().iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&Comando::CapturarRegion.id()));
        // El atajo general (D140) y Ctrl+2 para ocultar y mostrar los pines,
        // que el usuario pidio por su nombre el 2026-09-22. Ninguno mas.
        assert!(ids.contains(&Comando::AlternarPines.id()));
        assert_eq!(ids.len(), 2, "solo esos dos atajos de fabrica");
        assert!(
            !ids.contains(&Comando::Cuentagotas.id()),
            "el cuentagotas nace sin atajo (D81)"
        );
        assert!(!ids.contains(&Comando::Salir.id()));
    }

    #[test]
    fn detecta_dos_comandos_con_el_mismo_atajo() {
        let mut e = Enlaces::default();
        let repetido = e.atajo_de(Comando::CapturarRegion);
        e.poner(Comando::Cuentagotas, repetido);
        let (a, b) = e.choque().expect("tiene que detectarlo");
        assert!(
            [a, b].contains(&Comando::CapturarRegion) && [a, b].contains(&Comando::Cuentagotas),
            "{a:?} y {b:?}"
        );
    }

    #[test]
    fn un_fichero_con_la_tabla_vieja_sigue_valiendo() {
        // Migracion: quien tenga el TOML de antes no puede quedarse sin
        // atajos de golpe.
        let mut a = crate::ajustes::Ajustes::default();
        a.atajos.pin = Some("Ctrl+Shift+F2".parse().unwrap());
        a.atajos.cuentagotas = Some("Ctrl+Shift+F3".parse().unwrap());

        let (e, avisos) = Enlaces::de_ajustes(&a);
        assert!(avisos.is_empty());
        assert_eq!(
            e.atajo_de(Comando::Pinear).unwrap().to_string(),
            "Ctrl+Shift+F2"
        );
        assert_eq!(
            e.atajo_de(Comando::Cuentagotas).unwrap().to_string(),
            "Ctrl+Shift+F3"
        );
        assert_eq!(
            e.atajo_de(Comando::CapturarRegion).unwrap().to_string(),
            "Ctrl+Alt+X",
            "lo que no toco sigue como estaba"
        );
    }

    #[test]
    fn la_tabla_nueva_manda_sobre_la_vieja() {
        let mut a = crate::ajustes::Ajustes::default();
        a.atajos.pin = Some("Ctrl+Shift+F2".parse().unwrap());
        a.comandos
            .insert("pinear".to_string(), "Ctrl+Shift+F8".to_string());

        let (e, _) = Enlaces::de_ajustes(&a);
        assert_eq!(
            e.atajo_de(Comando::Pinear).unwrap().to_string(),
            "Ctrl+Shift+F8"
        );
    }

    #[test]
    fn el_anotador_de_pantalla_se_abre_desde_la_bandeja_y_nace_sin_atajo() {
        // El usuario quiere gestos y un solo atajo general: el anotador se
        // abre desde la bandeja, y el atajo lo pone quien lo quiera.
        for c in [Comando::Anotar, Comando::AnotarCongelada] {
            let d = c.descriptor();
            assert!(d.en_bandeja, "{}", d.nombre);
            assert_eq!(d.atajo_por_defecto, None, "{}", d.nombre);
        }
        // Y los nombres del TOML no cambian.
        assert_eq!(Comando::Anotar.descriptor().nombre, "anotar");
        assert_eq!(
            Comando::AnotarCongelada.descriptor().nombre,
            "anotar-congelada"
        );
    }

    #[test]
    fn la_bandeja_sale_del_catalogo_y_no_esta_vacia() {
        let en_bandeja: Vec<&str> = CATALOGO
            .iter()
            .filter(|d| d.en_bandeja)
            .map(|d| d.nombre)
            .collect();
        assert!(
            en_bandeja.contains(&"salir"),
            "sin «salir» no se puede cerrar: {en_bandeja:?}"
        );
        assert!(en_bandeja.len() >= 2);
    }
}
