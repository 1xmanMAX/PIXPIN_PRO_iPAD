//! Los `Mensaje` y `Proyecto` de PixPin Android, tal como los escribe kotlinx.
//!
//! El resumen de un mensaje (`Disco.resumenDe`) se calcula sobre el texto que
//! sale de `Disco.JSON.encodeToString(Mensaje)` con `encodeDefaults = true`:
//! **todos** los campos de la clase, cada uno con su valor por omision si no
//! lo trae. `Canonico.de` quita los nulos y ordena las claves, asi que el
//! orden y los nulos no cuentan; lo que cuenta es el juego de campos con
//! valor y como se escribe cada valor. Un `"texto":""` de menos, o un `picos`
//! que falte, y el resumen de todo lo nacido en el PC seria otro que el que
//! calcula el movil al recibirlo, y cada vuelta lo tomaria por cambiado.
//!
//! [normalizar_mensaje] y [normalizar_proyecto] dejan un objeto como lo
//! dejaria Kotlin al decodificarlo y volver a codificarlo, y son un punto
//! fijo: pasarlos dos veces no cambia nada. Dos diferencias a proposito:
//!
//! - Lo que no se conoce **se conserva detras** en vez de tirarse
//!   (`ignoreUnknownKeys`): un movil mas nuevo manda campos que esta version
//!   no sabe, los cuenta en su resumen, y tirarlos aqui haria que nunca
//!   cuadrara.
//! - Un valor de tipo equivocado en un campo con valor por omision se
//!   sustituye por ese valor, en vez de tumbar el mensaje como haria kotlinx:
//!   es lo que lo arregla para que el movil lo pueda leer.
//!
//! Esquema: `guardados/Mensajes.kt` y `motor/Proyectos.kt` de v0.51.0.

use crate::canonico::{self, Json, Objeto};

/// Como es cada campo.
#[derive(Clone, Copy)]
enum Tipo {
    Cadena,
    /// `Long` de Kotlin.
    Largo,
    /// `Int` de Kotlin: fuera de su rango kotlinx no lo lee.
    Entero,
    Booleano,
    ListaDeEnteros,
    ListaDeCadenas,
    /// Una de las palabras de [CLASES].
    Clase,
    Objeto(&'static [Campo]),
    ListaDeObjetos(&'static [Campo]),
}

/// Que pasa si falta.
#[derive(Clone, Copy)]
enum PorOmision {
    /// Sin valor por omision: si falta, kotlinx no lo lee.
    Obligado,
    Nulo,
    CadenaVacia,
    Cero,
    Falso,
    Verdadero,
    ListaVacia,
}

type Campo = (&'static str, Tipo, PorOmision);

use PorOmision as P;
use Tipo as T;

/// `enum class Clase`, en su orden.
pub const CLASES: [&str; 10] = [
    "NOTA", "IMAGEN", "ARCHIVO", "VOZ", "DIBUJO", "PAGINA", "PROYECTO", "MINIAPP", "TABLA",
    "CROQUIS",
];

const VIENE_DE: &[Campo] = &[
    ("texto", T::Cadena, P::Obligado),
    ("dibujo", T::Cadena, P::Nulo),
    ("pdf", T::Cadena, P::Nulo),
    ("pagina", T::Entero, P::Nulo),
    ("proyecto", T::Cadena, P::Nulo),
];

const TURNO: &[Campo] = &[
    ("quien", T::Cadena, P::Obligado),
    ("desdeMs", T::Entero, P::Obligado),
    ("hastaMs", T::Entero, P::Obligado),
];

/// `data class Mensaje`, campo a campo y en su orden.
const MENSAJE: &[Campo] = &[
    ("id", T::Cadena, P::Obligado),
    ("cuando", T::Largo, P::Obligado),
    ("clase", T::Clase, P::Obligado),
    ("texto", T::Cadena, P::CadenaVacia),
    ("ruta", T::Cadena, P::Nulo),
    ("nombre", T::Cadena, P::CadenaVacia),
    ("bytes", T::Largo, P::Cero),
    ("referencia", T::Cadena, P::Nulo),
    ("pagina", T::Entero, P::Nulo),
    ("duracionMs", T::Entero, P::Cero),
    ("picos", T::ListaDeEnteros, P::ListaVacia),
    ("miniapp", T::Cadena, P::Nulo),
    ("proyecto", T::Cadena, P::Nulo),
    ("emoji", T::Cadena, P::Nulo),
    ("soloLaFoto", T::Booleano, P::Verdadero),
    ("fijado", T::Booleano, P::Falso),
    ("respondeA", T::Cadena, P::Nulo),
    ("enBuzon", T::Booleano, P::Falso),
    ("unido", T::Booleano, P::Falso),
    ("transcripcion", T::Cadena, P::Nulo),
    ("estadoDelTexto", T::Cadena, P::Nulo),
    ("hojaDelTexto", T::Cadena, P::Nulo),
    ("marcas", T::ListaDeEnteros, P::ListaVacia),
    ("turnos", T::ListaDeObjetos(TURNO), P::ListaVacia),
    ("numero", T::Entero, P::Cero),
    ("letra", T::Cadena, P::Nulo),
    ("uid", T::Cadena, P::Nulo),
    ("aparato", T::Cadena, P::Nulo),
    ("origen", T::Cadena, P::Nulo),
    ("recibidoDe", T::Cadena, P::Nulo),
    ("vieneDe", T::Objeto(VIENE_DE), P::Nulo),
    ("recuerdaEn", T::Largo, P::Nulo),
];

/// `data class Hoja`, en su orden (`deMensaje` va antes que `croquis`).
const HOJA: &[Campo] = &[
    ("id", T::Cadena, P::Obligado),
    ("nombre", T::Cadena, P::CadenaVacia),
    ("dibujo", T::Cadena, P::Nulo),
    ("marco", T::Cadena, P::Nulo),
    ("nota", T::Cadena, P::Nulo),
    ("pagina", T::Entero, P::Nulo),
    ("deMensaje", T::Cadena, P::Nulo),
    ("croquis", T::Cadena, P::Nulo),
    ("vista", T::Cadena, P::Nulo),
    ("tabla", T::Cadena, P::Nulo),
    ("origen", T::Cadena, P::Nulo),
    ("padre", T::Cadena, P::Nulo),
    ("uid", T::Cadena, P::Nulo),
];

/// `data class Proyecto`, en su orden.
const PROYECTO: &[Campo] = &[
    ("id", T::Cadena, P::Obligado),
    ("nombre", T::Cadena, P::Obligado),
    ("hojas", T::ListaDeObjetos(HOJA), P::ListaVacia),
    ("archivado", T::Booleano, P::Falso),
    ("tocado", T::Largo, P::Cero),
    ("pdfOrigen", T::Cadena, P::Nulo),
    ("pdfLimpio", T::Cadena, P::Nulo),
    ("croquis", T::ListaDeCadenas, P::ListaVacia),
    ("origen", T::Cadena, P::Nulo),
    ("uid", T::Cadena, P::Nulo),
    ("creado", T::Largo, P::Cero),
    ("aparato", T::Cadena, P::Nulo),
    ("quitadas", T::ListaDeCadenas, P::ListaVacia),
];

/// El mensaje como lo dejaria kotlinx, o None si kotlinx no podria leerlo
/// (le falta `id`, `cuando` o una `clase` que conozca).
pub fn normalizar_mensaje(m: &Json) -> Option<Json> {
    normalizar(m, MENSAJE).map(Json::Objeto)
}

/// El proyecto como lo dejaria kotlinx (`Proyectos.json`), con sus hojas.
pub fn normalizar_proyecto(p: &Json) -> Option<Json> {
    normalizar(p, PROYECTO).map(Json::Objeto)
}

/// Lo mismo desde texto.
pub fn mensaje_de_texto(texto: &str) -> Option<Json> {
    normalizar_mensaje(&Json::analizar(texto).ok()?)
}

pub fn proyecto_de_texto(texto: &str) -> Option<Json> {
    normalizar_proyecto(&Json::analizar(texto).ok()?)
}

/// `Disco.textoDeBase`: el mensaje sin lo que no cuenta (el recordatorio, que
/// es de cada aparato, y dos de los codigos, que se ponen al sellar lo de
/// antes y harian parecer cambiado todo lo ya sincronizado), canonico.
/// `m` es el portatil ya normalizado.
pub fn texto_de_base(m: &Json) -> String {
    let mut o = m.como_objeto().cloned().unwrap_or_default();
    for k in ["recuerdaEn", "uid", "aparato"] {
        if o.contiene(k) {
            o.poner(k, Json::Nulo);
        }
    }
    canonico::de(&Json::Objeto(o).a_texto())
}

/// `Disco.resumenDe`.
pub fn resumen_de(m: &Json) -> String {
    canonico::sha256_hex(texto_de_base(m).as_bytes())
}

/// Una cadena de un objeto.
pub fn cadena<'a>(o: &'a Json, clave: &str) -> Option<&'a str> {
    o.como_objeto()?.obtener(clave)?.como_cadena()
}

/// Un numero entero de un objeto.
pub fn numero(o: &Json, clave: &str) -> Option<i64> {
    o.como_objeto()?.obtener(clave)?.contenido()?.parse().ok()
}

/// `Codigos.deChat`: `47·K7Q2`, el de antes `47a`, o nada.
pub fn de_chat(m: &Json) -> Option<String> {
    let n = numero(m, "numero").unwrap_or(0);
    if n <= 0 {
        return None;
    }
    if let Some(a) = cadena(m, "aparato") {
        return Some(format!("{n}·{a}"));
    }
    cadena(m, "letra").map(|l| format!("{n}{l}"))
}

/// `Codigos.de(semilla)`: diez signos sacados de un SHA-256, los mismos en
/// cualquier aparato.
pub fn codigo_de(semilla: &str) -> String {
    const SIGNOS: &[u8] = crate::codigo::SIGNOS.as_bytes();
    use sha2::Digest;
    let h = sha2::Sha256::digest(semilla.as_bytes());
    (0..crate::codigo::LARGO)
        .map(|i| SIGNOS[h[i] as usize % SIGNOS.len()] as char)
        .collect()
}

/// `Codigos.unico(m)`: su `uid`, o el que sale de su id.
pub fn unico(m: &Json) -> String {
    match cadena(m, "uid") {
        Some(u) => u.to_string(),
        None => codigo_de(&format!("m:{}", cadena(m, "id").unwrap_or_default())),
    }
}

/// `Codigos.unico(h)` de una hoja.
pub fn unico_de_hoja(h: &Json) -> String {
    match cadena(h, "uid") {
        Some(u) => u.to_string(),
        None => codigo_de(&format!("h:{}", cadena(h, "id").unwrap_or_default())),
    }
}

/// `Codigos.unico(p)` de un proyecto.
pub fn unico_de_proyecto(p: &Json) -> String {
    match cadena(p, "uid") {
        Some(u) => u.to_string(),
        None => codigo_de(&format!("p:{}", cadena(p, "id").unwrap_or_default())),
    }
}

/// `Disco.chatDe`: el proyecto del mensaje, o la conversacion general.
pub fn chat_de(m: &Json) -> String {
    cadena(m, "proyecto")
        .unwrap_or(crate::disco::GENERAL)
        .to_string()
}

/// Pone o cambia un campo de un objeto.
pub fn poner(o: &mut Json, clave: &str, valor: Json) {
    if let Json::Objeto(ob) = o {
        ob.poner(clave, valor);
    }
}

fn normalizar(v: &Json, campos: &[Campo]) -> Option<Objeto> {
    let o = v.como_objeto()?;
    let mut salida = Objeto::nuevo();
    for (nombre, tipo, omision) in campos {
        let dado = o.obtener(nombre).filter(|x| !matches!(x, Json::Nulo));
        let valor = match dado.and_then(|x| convertir(x, *tipo)) {
            Some(x) => x,
            None => match omision {
                P::Obligado => return None,
                P::Nulo => Json::Nulo,
                P::CadenaVacia => Json::cadena(""),
                P::Cero => Json::numero(0),
                P::Falso => Json::booleano(false),
                P::Verdadero => Json::booleano(true),
                P::ListaVacia => Json::Lista(Vec::new()),
            },
        };
        salida.poner(*nombre, valor);
    }
    for (k, x) in o.iter() {
        if !campos.iter().any(|(n, _, _)| *n == k) {
            salida.poner(k, x.clone());
        }
    }
    Some(salida)
}

fn convertir(x: &Json, tipo: Tipo) -> Option<Json> {
    match tipo {
        T::Cadena => x.como_cadena().map(Json::cadena),
        T::Largo => entero(x, i64::MIN, i64::MAX),
        T::Entero => entero(x, i32::MIN as i64, i32::MAX as i64),
        T::Booleano => match x {
            Json::Literal(t) if t == "true" || t == "false" => Some(x.clone()),
            _ => None,
        },
        T::Clase => x
            .como_cadena()
            .filter(|c| CLASES.contains(c))
            .map(Json::cadena),
        T::ListaDeEnteros => {
            let l = x.como_lista()?;
            l.iter()
                .map(|e| entero(e, i32::MIN as i64, i32::MAX as i64))
                .collect::<Option<Vec<_>>>()
                .map(Json::Lista)
        }
        T::ListaDeCadenas => {
            let l = x.como_lista()?;
            l.iter()
                .map(|e| e.como_cadena().map(Json::cadena))
                .collect::<Option<Vec<_>>>()
                .map(Json::Lista)
        }
        T::Objeto(campos) => normalizar(x, campos).map(Json::Objeto),
        // Una entrada que kotlinx no leeria se quita: con ella no leeria la
        // lista entera, y sin la lista, tampoco el mensaje.
        T::ListaDeObjetos(campos) => x.como_lista().map(|l| {
            Json::Lista(
                l.iter()
                    .filter_map(|e| normalizar(e, campos).map(Json::Objeto))
                    .collect(),
            )
        }),
    }
}

/// Un entero escrito como lo escribe Kotlin (`1726000000000`, nunca `1.0`).
fn entero(x: &Json, min: i64, max: i64) -> Option<Json> {
    let Json::Literal(t) = x else {
        return None;
    };
    let n: i64 = t.parse().ok()?;
    (min..=max).contains(&n).then(|| Json::numero(n))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un mensaje tal como lo escribe `Disco.JSON` en el movil: los 32
    /// campos, con sus nulos.
    const DEL_MOVIL: &str = r#"{"id":"1757939357123","cuando":1757939357123,"clase":"NOTA","texto":"hola\nqué tal","ruta":null,"nombre":"","bytes":0,"referencia":null,"pagina":null,"duracionMs":0,"picos":[],"miniapp":null,"proyecto":null,"emoji":null,"soloLaFoto":true,"fijado":false,"respondeA":null,"enBuzon":false,"unido":false,"transcripcion":null,"estadoDelTexto":null,"hojaDelTexto":null,"marcas":[],"turnos":[],"numero":47,"letra":null,"uid":"RVK5YHKCX7","aparato":"K7Q2","origen":null,"recibidoDe":null,"vieneDe":null,"recuerdaEn":1760000000000}"#;

    #[test]
    fn un_mensaje_del_movil_sale_igual_al_normalizarlo() {
        let n = mensaje_de_texto(DEL_MOVIL).unwrap();
        assert_eq!(n.a_texto(), DEL_MOVIL, "ni un campo de mas ni de menos");
        // Y es un punto fijo.
        assert_eq!(normalizar_mensaje(&n).unwrap().a_texto(), DEL_MOVIL);
    }

    #[test]
    fn el_resumen_de_un_mensaje_del_movil_es_el_de_kotlin() {
        // Derivado a mano de `Disco.resumenDe` (no hay Kotlin aqui): sin
        // recuerdaEn, uid ni aparato; sin nulos; claves ordenadas como
        // `String.compareTo`; el texto con su escapado.
        let canonico = r#"{"bytes":0,"clase":"NOTA","cuando":1757939357123,"duracionMs":0,"enBuzon":false,"fijado":false,"id":"1757939357123","marcas":[],"nombre":"","numero":47,"picos":[],"soloLaFoto":true,"texto":"hola\nqué tal","turnos":[],"unido":false}"#;
        let m = mensaje_de_texto(DEL_MOVIL).unwrap();
        assert_eq!(texto_de_base(&m), canonico);
        assert_eq!(resumen_de(&m), canonico::sha256_hex(canonico.as_bytes()));
        assert_eq!(de_chat(&m).as_deref(), Some("47·K7Q2"));
        assert_eq!(unico(&m), "RVK5YHKCX7");
    }

    #[test]
    fn una_nota_escrita_en_el_pc_sale_con_todo_lo_que_pondria_kotlin() {
        // Como la escribe `cuaderno::anadir` del PC: sin picos, sin
        // soloLaFoto, con la clase y los nulos que pone serde.
        let del_pc = r#"{"id":"1","cuando":5,"clase":"NOTA","texto":"x","ruta":null,"nombre":"","bytes":0,"referencia":null,"pagina":null,"duracionMs":0,"proyecto":"p","emoji":null,"fijado":false,"enBuzon":false,"transcripcion":null,"miniapp":null,"respondeA":null,"numero":3,"letra":null,"uid":"AAAAAAAAAA","aparato":"K7Q2","origen":null}"#;
        let n = mensaje_de_texto(del_pc).unwrap();
        let o = n.como_objeto().unwrap();
        assert_eq!(o.len(), 32);
        assert_eq!(o.obtener("soloLaFoto"), Some(&Json::booleano(true)));
        assert_eq!(o.obtener("picos"), Some(&Json::Lista(vec![])));
        // El orden es el de la clase de Kotlin, no el del PC.
        let claves: Vec<&str> = o.claves().take(4).collect();
        assert_eq!(claves, ["id", "cuando", "clase", "texto"]);
        assert_eq!(normalizar_mensaje(&n), Some(n.clone()), "punto fijo");
    }

    #[test]
    fn lo_que_kotlin_no_leeria_no_pasa_y_lo_que_no_se_conoce_se_queda() {
        assert!(
            mensaje_de_texto(r#"{"id":"a","cuando":1}"#).is_none(),
            "sin clase"
        );
        assert!(mensaje_de_texto(r#"{"id":"a","cuando":1,"clase":"COSA"}"#).is_none());
        assert!(
            mensaje_de_texto(r#"{"cuando":1,"clase":"NOTA"}"#).is_none(),
            "sin id"
        );
        let m = mensaje_de_texto(r#"{"id":"a","cuando":1,"clase":"TABLA","futuro":{"x":1.50}}"#)
            .unwrap();
        assert_eq!(
            m.como_objeto()
                .unwrap()
                .obtener("futuro")
                .unwrap()
                .a_texto(),
            r#"{"x":1.50}"#
        );
        // Un tipo equivocado en un campo con valor por omision se arregla.
        let m = mensaje_de_texto(r#"{"id":"a","cuando":1,"clase":"NOTA","bytes":"12","fijado":1}"#)
            .unwrap();
        assert_eq!(numero(&m, "bytes"), Some(0));
        assert_eq!(
            m.como_objeto().unwrap().obtener("fijado"),
            Some(&Json::booleano(false))
        );
    }

    #[test]
    fn un_proyecto_sale_como_proyectos_json() {
        let p = proyecto_de_texto(
            r#"{"id":"pr-1","nombre":"Obra","hojas":[{"id":"h1","dibujo":"d1"}],"tocado":5}"#,
        )
        .unwrap();
        assert_eq!(
            p.a_texto(),
            r#"{"id":"pr-1","nombre":"Obra","hojas":[{"id":"h1","nombre":"","dibujo":"d1","marco":null,"nota":null,"pagina":null,"deMensaje":null,"croquis":null,"vista":null,"tabla":null,"origen":null,"padre":null,"uid":null}],"archivado":false,"tocado":5,"pdfOrigen":null,"pdfLimpio":null,"croquis":[],"origen":null,"uid":null,"creado":0,"aparato":null,"quitadas":[]}"#
        );
    }

    #[test]
    fn los_codigos_son_los_de_android() {
        // Los mismos que prueba `pixpin-proyecto::codigos` contra Android.
        assert_eq!(codigo_de("p:proyecto-1"), "VVT587BFCA");
        assert_eq!(codigo_de("h:hoja-7"), "2C8C39HXZ9");
    }
}
