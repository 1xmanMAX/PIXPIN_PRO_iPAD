//! El `Disco` de PixPin Android, sobre una carpeta `files` como la del movil.
//!
//! Puerto directo de `sincro/Disco.kt` para las pruebas: con el, un «movil»
//! y el PC (o dos moviles) se sincronizan de verdad por un socket en esta
//! maquina, como hace `SincronizarDeVerdadTest.kt`. Guarda lo mismo que el
//! movil y donde el movil: `guardados.jsonl` con todos los chats,
//! `proyectos/proyectos.json`, `pins/draw/<id>.excalidraw.gz`…, y cada
//! mensaje con los 32 campos que escribe `Disco.JSON`.
//!
//! Las rutas absolutas que guarda van con un prefijo de Android inventado
//! (`/data/user/0/<aparato>/files/`) y no con el de esta carpeta: asi se
//! comprueba lo mismo que en el movil —que lo que viaja no lleva rutas de
//! un aparato— sin depender de como escribe las rutas Windows.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::canonico::Json;
use crate::disco::{self, Disco, GENERAL, Identidad, NOMBRE_GENERAL, PORTATIL, escribir_atomico};
use crate::kotlin;
use crate::mensajes::{Aparato, Chat};

pub struct DiscoAndroid {
    files: PathBuf,
    /// El prefijo absoluto de este «aparato», con la barra del final.
    prefijo: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct IdentidadEnDisco {
    yo: Aparato,
    codigo: Option<String>,
    miembros: Vec<Aparato>,
}

impl DiscoAndroid {
    /// `nombre` distingue el prefijo de cada aparato.
    pub fn nuevo(files: PathBuf, nombre: &str) -> DiscoAndroid {
        let _ = std::fs::create_dir_all(&files);
        DiscoAndroid {
            files,
            prefijo: format!("/data/user/0/{nombre}/files/"),
        }
    }

    /// La ruta absoluta que guardaria el movil para `rel`.
    pub fn absoluta(&self, rel: &str) -> String {
        format!("{}{rel}", self.prefijo)
    }

    pub fn prefijo(&self) -> &str {
        &self.prefijo
    }

    fn archivo_del_chat(&self) -> PathBuf {
        self.files.join("guardados.jsonl")
    }

    /// `leerMensajes`: lo que no se lee, no sale.
    pub fn leer_mensajes(&self) -> Vec<Json> {
        std::fs::read_to_string(self.archivo_del_chat())
            .map(|t| {
                t.lines()
                    .filter(|l| !l.trim().is_empty())
                    .filter_map(kotlin::mensaje_de_texto)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn escribir_mensajes(&self, lista: &[Json]) -> io::Result<()> {
        let texto: String = lista.iter().map(|m| m.a_texto() + "\n").collect();
        escribir_atomico(&self.archivo_del_chat(), texto.as_bytes())
    }

    /// Como hace el chat al guardar: una linea al final.
    pub fn anadir_mensaje(&self, m: &Json) -> io::Result<()> {
        use std::io::Write;
        let n = kotlin::normalizar_mensaje(m).ok_or_else(|| io::Error::other("mensaje roto"))?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.archivo_del_chat())?
            .write_all((n.a_texto() + "\n").as_bytes())
    }

    fn archivo_de_proyectos(&self) -> PathBuf {
        self.files.join("proyectos").join("proyectos.json")
    }

    pub fn leer_proyectos(&self) -> Vec<Json> {
        std::fs::read_to_string(self.archivo_de_proyectos())
            .ok()
            .and_then(|t| Json::analizar(&t).ok())
            .and_then(|j| j.como_lista().map(<[Json]>::to_vec))
            .unwrap_or_default()
            .iter()
            .filter_map(kotlin::normalizar_proyecto)
            .collect()
    }

    pub fn escribir_proyectos(&self, lista: &[Json]) -> io::Result<()> {
        escribir_atomico(
            &self.archivo_de_proyectos(),
            Json::Lista(lista.to_vec()).a_texto().as_bytes(),
        )
    }

    fn portatil(&self, t: &str) -> String {
        t.replace(&self.prefijo, PORTATIL)
    }

    fn local(&self, t: &str) -> String {
        t.replace(PORTATIL, &self.prefijo)
    }

    fn portatil_de(&self, j: &Json) -> Option<Json> {
        Json::analizar(&self.portatil(&j.a_texto())).ok()
    }
}

impl Disco for DiscoAndroid {
    fn raiz(&self) -> PathBuf {
        self.files.clone()
    }

    fn identidad(&self) -> io::Result<Identidad> {
        let i: IdentidadEnDisco = std::fs::read_to_string(self.sincro().join("identidad.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Ok(Identidad {
            yo: i.yo,
            codigo: i.codigo,
            miembros: i.miembros,
        })
    }

    fn guardar_identidad(&self, i: &Identidad) -> io::Result<()> {
        let e = IdentidadEnDisco {
            yo: i.yo.clone(),
            codigo: i.codigo.clone(),
            miembros: i.miembros.clone(),
        };
        escribir_atomico(
            &self.sincro().join("identidad.json"),
            serde_json::to_string(&e)
                .map_err(io::Error::other)?
                .as_bytes(),
        )
    }

    fn chats(&self) -> io::Result<Vec<Chat>> {
        let todos = self.leer_mensajes();
        let cuenta = |c: &str| todos.iter().filter(|m| kotlin::chat_de(m) == c).count() as u32;
        let ultimo = |c: &str| {
            todos
                .iter()
                .filter(|m| kotlin::chat_de(m) == c)
                .filter_map(|m| kotlin::numero(m, "cuando"))
                .max()
                .unwrap_or(0)
                .max(0)
        };
        let mut v = vec![Chat {
            id: GENERAL.into(),
            nombre: NOMBRE_GENERAL.into(),
            mensajes: cuenta(GENERAL),
            tocado: ultimo(GENERAL),
        }];
        for p in self.leer_proyectos() {
            let id = kotlin::cadena(&p, "id").unwrap_or_default().to_string();
            v.push(Chat {
                nombre: kotlin::cadena(&p, "nombre").unwrap_or_default().into(),
                mensajes: cuenta(&id),
                tocado: kotlin::numero(&p, "tocado").unwrap_or(0).max(ultimo(&id)),
                id,
            });
        }
        Ok(v)
    }

    fn mensajes(&self, chat: &str) -> io::Result<Vec<Json>> {
        Ok(self
            .leer_mensajes()
            .iter()
            .filter(|m| kotlin::chat_de(m) == chat)
            .filter_map(|m| self.portatil_de(m))
            .collect())
    }

    fn aplicar_mensajes(
        &self,
        chat: &str,
        poner: &[String],
        borrar: &[String],
        ahora: i64,
    ) -> io::Result<()> {
        if poner.is_empty() && borrar.is_empty() {
            return Ok(());
        }
        let quitar: HashSet<String> = borrar.iter().cloned().collect();
        let llegan: Vec<Json> = poner
            .iter()
            .filter_map(|t| kotlin::mensaje_de_texto(&self.local(t)))
            .filter(|m| kotlin::chat_de(m) == chat)
            .collect();
        let (salida, quitados) =
            disco::aplicar_en_lista(self.leer_mensajes(), chat, &llegan, &quitar);
        self.escribir_mensajes(&salida)?;
        self.marcas_tras_aplicar(chat, &llegan, borrar, &quitados, ahora)?;
        for m in &quitados {
            if let Some(r) = kotlin::cadena(m, "ruta")
                .and_then(|r| r.strip_prefix(&self.prefijo))
                .filter(|r| r.starts_with("guardados/"))
            {
                let _ = std::fs::remove_file(self.files.join(r));
            }
            // Y lo anotado sobre el, que sin su mensaje ya no viaja (v0.96);
            // de una nota, sus comentarios (30-sep).
            if crate::anotado::lleva_anotado(m) {
                self.borrar_anotado(chat, &kotlin::unico(m));
            }
        }
        self.avisar(disco::Cambio::Mensajes);
        Ok(())
    }

    fn sellar(&self) -> io::Result<()> {
        let aparato = grupo_codigo(&self.identidad()?.yo.id);
        let mut lista = self.leer_mensajes();
        let mut chats: Vec<String> = Vec::new();
        for m in &lista {
            let c = kotlin::chat_de(m);
            if !chats.contains(&c) {
                chats.push(c);
            }
        }
        let mut cambio = false;
        for c in chats {
            let idx: Vec<usize> = (0..lista.len())
                .filter(|&i| kotlin::chat_de(&lista[i]) == c)
                .collect();
            let mut suyos: Vec<Json> = idx.iter().map(|&i| lista[i].clone()).collect();
            let cambiados = disco::sellar_chat(&mut suyos, &aparato);
            if !cambiados.is_empty() {
                cambio = true;
                for (k, &i) in idx.iter().enumerate() {
                    lista[i] = suyos[k].clone();
                }
            }
        }
        if cambio {
            self.escribir_mensajes(&lista)?;
        }
        let ps = self.leer_proyectos();
        let sellados: Vec<Json> = ps
            .iter()
            .map(|p| {
                let mut q = p.clone();
                disco::sellar_proyecto(&mut q);
                q
            })
            .collect();
        if sellados != ps {
            self.escribir_proyectos(&sellados)?;
        }
        Ok(())
    }

    fn borrar_chat(&self, chat: &str, motivo: &str, cuando: i64, aparato: &str) -> io::Result<()> {
        self.hacer_copia(chat, motivo, cuando)?;
        let claves: Vec<String> = self
            .mensajes_por_clave(chat)?
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        if !claves.is_empty() {
            self.aplicar_mensajes(chat, &[], &claves, cuando)?;
        }
        let quedan: Vec<Json> = self
            .leer_proyectos()
            .into_iter()
            .filter(|p| kotlin::cadena(p, "id") != Some(chat))
            .collect();
        self.escribir_proyectos(&quedan)?;
        self.anotar_lapida(chat, cuando, aparato)
    }

    fn proyecto_portatil(&self, chat: &str) -> io::Result<Option<Json>> {
        Ok(self
            .leer_proyectos()
            .iter()
            .find(|p| kotlin::cadena(p, "id") == Some(chat))
            .and_then(|p| self.portatil_de(p)))
    }

    fn guardar_proyecto(&self, p: &Json) -> io::Result<()> {
        let Some(nuevo) = kotlin::proyecto_de_texto(&self.local(&p.a_texto())) else {
            return Ok(());
        };
        let id = kotlin::cadena(&nuevo, "id").unwrap_or_default().to_string();
        let mut lista = self.leer_proyectos();
        let antes = lista
            .iter()
            .position(|x| kotlin::cadena(x, "id") == Some(id.as_str()));
        let puesto = disco::actualizado(antes.map(|i| &lista[i]), &nuevo);
        // Una hoja que ya no esta se lleva sus comentarios (30-sep).
        for uid in crate::anotado::hojas_quitadas(antes.map(|i| &lista[i]), &puesto) {
            self.borrar_anotado(&id, &uid);
        }
        match antes {
            Some(i) => lista[i] = puesto,
            None => lista.push(puesto),
        }
        self.escribir_proyectos(&lista)?;
        self.avisar(disco::Cambio::Proyectos);
        Ok(())
    }

    fn ruta(&self, _chat: &str, rel: &str) -> PathBuf {
        self.files.join(rel)
    }

    fn a_portatil(&self, _chat: &str, _rel: &str, texto: String) -> String {
        self.portatil(&texto)
    }

    fn a_local(&self, _chat: &str, texto: String) -> String {
        self.local(&texto)
    }
}

/// `Aparato.codigo`: los cuatro signos fijos de un aparato.
pub fn grupo_codigo(id: &str) -> String {
    crate::grupo::codigo_de_aparato(id)
}

/// Lo que hace falta para que dos «aparatos» se hablen en una prueba, por un
/// socket de verdad en esta maquina, con el protocolo y el cifrado de
/// verdad: uno responde en un hilo y el otro dirige en este.
pub mod prueba {
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicI64, Ordering};

    use crate::disco::{Disco, Identidad};
    use crate::mensajes::Aparato;
    use crate::protocolo::{Hecho, Respondedor, Resultado, Sesion};

    /// El reloj de la prueba: avanza cuando se le pide, no con el de la
    /// maquina, para que las horas sean las mismas cada vez.
    pub struct Reloj(pub AtomicI64);

    impl Reloj {
        pub fn nuevo() -> Reloj {
            Reloj(AtomicI64::new(1_000_000))
        }
        pub fn ahora(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
        /// `reloj++`: la hora de ahora, y la siguiente para la proxima vez.
        pub fn tic(&self) -> i64 {
            self.0.fetch_add(1, Ordering::SeqCst)
        }
        pub fn saltar(&self, ms: i64) {
            self.0.fetch_add(ms, Ordering::SeqCst);
        }
    }

    pub fn presentar<D: Disco>(d: &D, id: &str, nombre: &str) {
        d.guardar_identidad(&Identidad {
            yo: Aparato {
                id: id.into(),
                nombre: nombre.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    }

    /// `Disco.crearGrupo`: este aparato dentro, con la letra `a`.
    pub fn crear_grupo<D: Disco>(d: &D, codigo: &str, ahora: i64) {
        let actual = d.identidad().unwrap();
        let yo = Aparato {
            letra: Some("a".into()),
            desde: ahora,
            ..actual.yo
        };
        d.guardar_identidad(&Identidad {
            yo: yo.clone(),
            codigo: Some(codigo.into()),
            miembros: vec![yo],
        })
        .unwrap();
        d.sellar().unwrap();
    }

    /// Uno responde en otro hilo y `desde` dirige aqui con `uso`.
    pub fn conectado<A, B, T>(
        desde: &A,
        hacia: &B,
        unirme: bool,
        codigo: Option<&str>,
        reloj: &Reloj,
        uso: impl FnOnce(&mut Sesion<'_, A, TcpStream>) -> T,
    ) -> Resultado<T>
    where
        A: Disco + Sync,
        B: Disco + Sync,
    {
        let escucha = TcpListener::bind(("127.0.0.1", 0))?;
        let puerto = escucha.local_addr()?.port();
        std::thread::scope(|hilos| {
            hilos.spawn(|| {
                if let Ok((flujo, _)) = escucha.accept() {
                    let r = Respondedor {
                        disco: hacia,
                        estado: &|_| {},
                        ahora: &|| reloj.ahora(),
                        mi_puerto: 0,
                        al_saludar: &|_, _| {},
                        suelto: None,
                    };
                    let _ = r.atender(flujo, [7u8; 32]);
                }
            });
            let flujo = TcpStream::connect(("127.0.0.1", puerto))?;
            let mut s =
                Sesion::conectar(flujo, desde, unirme, codigo, || reloj.ahora(), 0, [3u8; 32]);
            match &mut s {
                Ok(s) => {
                    let r = uso(s);
                    s.adios();
                    Ok(r)
                }
                Err(_) => Err(s.err().unwrap()),
            }
        })
    }

    /// Sincroniza los chats dados, sin preguntas. Devuelve lo hecho y los
    /// bytes que se movieron.
    pub fn sincronizar<A, B>(
        desde: &A,
        hacia: &B,
        chats: &[&str],
        reloj: &Reloj,
        antes_de_cerrar: &dyn Fn(),
    ) -> (Hecho, u64)
    where
        A: Disco + Sync,
        B: Disco + Sync,
    {
        conectado(desde, hacia, false, None, reloj, |s| {
            let mut hecho = Hecho::default();
            for chat in chats {
                let prep = s.preparar(chat).unwrap();
                s.aplicar(&prep, &mut hecho).unwrap();
                let arch = s.preparar_archivos(&prep).unwrap();
                s.aplicar_archivos(&arch, &mut hecho, &mut |_| {}).unwrap();
                antes_de_cerrar();
                s.cerrar(&prep, reloj.ahora()).unwrap();
            }
            (hecho, s.enviados() + s.recibidos())
        })
        .unwrap()
    }
}
