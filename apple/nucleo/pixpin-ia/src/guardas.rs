//! **Las guardas deterministas del turno** (25-oct-2026): comprobaciones sin
//! modelo que el bucle del agente hace en cada vuelta, como los hooks y las
//! comprobaciones de Claude Code (leer antes de editar, no dar por acabado
//! lo que no lo esta). El analisis esta en
//! `docs/investigacion/2026-10-25-agente-como-claude-code.md`.
//!
//! - [`sin_anexos`]: el pedido tal como lo escribio el usuario, sin el
//!   resumen del lienzo ni las notas de archivos y de la hora que la app le
//!   pega. Con ellos, cualquier pedido con el lienzo abierto pasaba de 220
//!   letras y contaba como «de varias partes»: entraba el jefe, partia
//!   «agrandala a 30 actividades y 30 semanas» en tareas, y cada trabajador,
//!   con su contexto limpio, dibujaba su propio cronograma al lado.
//! - [`duplicaria`]: si el pedido es cambiar lo que ya hay y una herramienta
//!   de crear desde cero va a anadir otro al lado, un ERROR que explica que
//!   usar en su lugar, en vez de dibujar.
//! - [`anuncia_pendiente`]: una respuesta final que solo anuncia trabajo
//!   («ahora voy a limpiar el resto…») o reconoce que esta a medias no cierra
//!   el turno: se le devuelve [`EMPUJON_ANUNCIO`] y sigue.

use crate::acciones::normalizar;
use crate::herramientas::Llamada;

/// Como empieza el resumen del lienzo pegado al pedido
/// (`edicion::anadir_resumen`: `"\n\n[" + resumen`).
pub const MARCA_RESUMEN: &str = "\n\n[Lienzo destino:";

/// Las notas que la app pega detras del pedido (`asistente::nota_de_ahora`,
/// `en_el_chat::nota_de_archivos`, la de la imagen del lienzo).
const MARCAS_DE_NOTAS: [&str; 3] = [
    "\n\n(Ahora:",
    "\n\n(Archivos adjuntos en este chat",
    "\n\n(La imagen adjunta es",
];

/// Lo que dice el resumen cuando la plantilla de serie ya esta en el lienzo
/// (`edicion::resumen`).
pub const MARCA_PLANTILLAS: &str = "Plantillas de serie que YA estan en el lienzo";

/// Donde empieza el pedido del usuario en el encargo de un trabajador del
/// jefe (`orquesta::trabajar`): lo de antes son las palabras del jefe.
pub const MARCA_PEDIDO_ORIGINAL: &str = "— Pedido original del usuario —";

/// **El pedido sin lo que le pega la app**: se corta en el primer anexo
/// (resumen del lienzo o notas). Lo que decide si el pedido es complejo, de
/// varias partes o de que colores es lee esto, no los anexos.
pub fn sin_anexos(texto: &str) -> String {
    let corte = std::iter::once(MARCA_RESUMEN)
        .chain(MARCAS_DE_NOTAS)
        .filter_map(|m| texto.find(m))
        .min()
        .unwrap_or(texto.len());
    texto[..corte].trim_end().to_string()
}

/// **El pedido del usuario dentro de un encargo del jefe** (o el texto
/// entero si no lo es), sin anexos.
pub fn lo_que_pidio(texto: &str) -> String {
    let desde = texto
        .find(MARCA_PEDIDO_ORIGINAL)
        .map(|i| i + MARCA_PEDIDO_ORIGINAL.len())
        .unwrap_or(0);
    sin_anexos(texto[desde..].trim_start())
}

/// Si el resumen pegado dice que el lienzo tiene algo.
pub fn lienzo_con_algo(texto: &str) -> bool {
    texto.contains(MARCA_RESUMEN) && !texto.contains("[Lienzo destino: vacio")
}

/// Si el usuario pide expresamente otro (ademas del que hay).
pub fn pide_otro(pedido: &str) -> bool {
    let q = normalizar(pedido);
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    palabras.iter().any(|p| {
        [
            "otro",
            "otra",
            "nuevo",
            "nueva",
            "aparte",
            "copia",
            "duplica",
            "duplicalo",
            "duplicala",
            "segundo",
            "segunda",
        ]
        .contains(p)
    }) || q.contains("al lado")
        || q.contains("desde cero")
        || q.contains("de nuevo")
}

/// **La guarda anti-duplicado.** `pedido` es el texto entero del ultimo
/// mensaje del usuario (con el resumen del lienzo, que dice si hay algo).
/// Devuelve el ERROR para el modelo si `llamada` haria otro al lado de lo
/// que el usuario pidio cambiar; `None` si se puede hacer.
///
/// Solo salta si a la vez: el pedido es agrandar o reorganizar
/// ([`crate::seleccion::es_cambiar_lo_que_hay`]), el lienzo tiene algo, el
/// usuario no pidio «otro» y la llamada crea desde cero (cronograma, SVG,
/// poner otra plantilla); dibujar piezas sueltas nunca se para.
pub fn duplicaria(pedido: &str, llamada: &Llamada) -> Option<String> {
    let dicho = lo_que_pidio(pedido);
    if !crate::seleccion::es_cambiar_lo_que_hay(&dicho)
        || !lienzo_con_algo(pedido)
        || pide_otro(&dicho)
    {
        return None;
    }
    let hay_plantilla = pedido.contains(MARCA_PLANTILLAS);
    let crea = match llamada {
        Llamada::Cronograma(_) | Llamada::Svg(_) => true,
        Llamada::Figuras(crate::figuras::PedidoFiguras::Poner { .. }) => hay_plantilla,
        _ => false,
    };
    if !crea {
        return None;
    }
    let como = if hay_plantilla {
        "Para agrandar la plantilla de serie que ya está: figuras_guardadas {\"accion\":\"alargar\", ...} (mismo diseño, en su sitio); \
para mover bloques o añadir filas: reorganizar_hoja; para cambiar textos: editar_texto."
    } else {
        "Cámbialo con sus propias piezas: transformar_elementos (tamaño, sitio), editar_texto, reorganizar_hoja (mover bloques, añadir filas)."
    };
    Some(format!(
        "ERROR: no se hizo {}: el usuario pidió cambiar lo que YA está en el lienzo (agrandar o reorganizar), y esto añadiría \
otro al lado. {como} No dibujes otra versión ni inventes contenido. Si de verdad hace falta uno nuevo además del que hay, \
pregúntalo antes con preguntar_usuario.",
        llamada.nombre()
    ))
}

// ---------------------------------------------------------------------------
// Anunciar en vez de hacer

/// Cuantos empujones como mucho en un turno (los de [`EMPUJON_ANUNCIO`] y
/// los de los pasos sin acabar, `pasos.rs`): con dos o tres el modelo acaba
/// o dice que falta; mas solo alarga un turno que no sabe hacerlo.
pub const EMPUJONES: usize = 3;

/// Lo que se le dice al modelo cuando contesta anunciando en vez de hacer.
pub const EMPUJON_ANUNCIO: &str = "[Aviso del turno] Tu respuesta anuncia trabajo o dice que quedó a medias, pero el turno se acaba cuando \
contestas sin herramientas: el usuario no verá nada más. No lo anuncies: hazlo AHORA con las herramientas y termina. \
Si de verdad no puedes, di exactamente qué falta y por qué, sin prometer nada.";

/// Lo que dice que queda trabajo (normalizado: minusculas, sin tildes).
const ANUNCIOS: &[&str] = &[
    "voy a ",
    "ahora voy",
    "a continuacion",
    "acto seguido",
    "quedo a medias",
    "quedo incompleto",
    "quedo incompleta",
    "esta a medias",
    "todavia no esta",
    "aun no esta",
    "todavia no he",
    "aun no he",
    "todavia falta",
    "aun falta",
    "me falta ",
    "falta por hacer",
    "faltan por hacer",
    "lo hare",
    "la hare",
    "los hare",
    "las hare",
    "lo hago ahora",
    "procedo a",
    "procedere",
    "empezare",
    "comenzare",
    "continuare",
    "seguire con",
];

/// Lo que ofrece en vez de anunciar («si quieres, lo hago»): eso si cierra.
const OFERTAS: &[&str] = &[
    "si quieres",
    "si lo prefieres",
    "si prefieres",
    "si te parece",
    "si lo deseas",
    "cuando quieras",
];

/// **Si la respuesta anuncia trabajo pendiente o reconoce que no esta
/// acabado.** Se mira solo la cola (lo ultimo que dice: un resumen largo
/// puede contar de pasada lo que «iba a» hacer), frase a frase; una
/// pregunta al usuario o una oferta («si quieres, …») no cuentan.
pub fn anuncia_pendiente(texto: &str) -> bool {
    let t = normalizar(texto);
    let n = t.chars().count();
    let cola: String = t.chars().skip(n.saturating_sub(600)).collect();
    let mut frases: Vec<String> = Vec::new();
    let mut actual = String::new();
    for c in cola.chars() {
        actual.push(c);
        if matches!(c, '.' | '!' | '?' | '\n' | '…' | ';') {
            frases.push(std::mem::take(&mut actual));
        }
    }
    frases.push(actual);
    frases.iter().any(|f| {
        let f = f.trim();
        if f.is_empty() || f.ends_with('?') || f.starts_with('¿') {
            return false;
        }
        // Con un espacio delante: «voy a» al principio de la frase cuenta
        // igual, y «llevoy a» no.
        let f = format!(" {f} ");
        !OFERTAS.iter().any(|o| f.contains(o))
            && ANUNCIOS.iter().any(|a| f.contains(&format!(" {a}")))
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const RESUMEN_CON_PLANTILLA: &str = "\n\n[Lienzo destino: 120 elementos (coordenadas como en dibujar_en_lienzo).\n\
Plantillas de serie que YA estan en el lienzo: «Cronograma». Para agrandarlas usa figuras_guardadas «alargar».\n#1 rectangulo]";

    /// La guarda no mira las figuras: con la lista vacia basta.
    fn dibujar() -> Llamada {
        Llamada::Dibujar(Vec::new())
    }

    #[test]
    fn sin_anexos_deja_solo_lo_que_escribio() {
        let entero = format!(
            "agrándala a 30 actividades y 30 semanas{RESUMEN_CON_PLANTILLA}\n\n(Ahora: sáb 24-oct-2026 22:05, hora local del usuario.)"
        );
        assert_eq!(
            sin_anexos(&entero),
            "agrándala a 30 actividades y 30 semanas"
        );
        // Con el resumen pegado era «de varias partes» (y entraba el jefe);
        // sin el, no.
        assert!(crate::agente::pedido_compuesto(&entero));
        assert!(!crate::agente::pedido_compuesto(&sin_anexos(&entero)));
        // Negativo: un pedido sin anexos queda igual.
        assert_eq!(sin_anexos("dibuja una casa"), "dibuja una casa");
        // En el encargo de un trabajador, lo del usuario va tras la marca.
        let encargo = format!(
            "Tu tarea (1 de 2): ampliar. Lo ya hecho por otros trabajadores: nada\n\n{MARCA_PEDIDO_ORIGINAL}\nagranda el cronograma{RESUMEN_CON_PLANTILLA}"
        );
        assert_eq!(lo_que_pidio(&encargo), "agranda el cronograma");
    }

    #[test]
    fn agrandar_lo_que_hay_no_dibuja_otro_al_lado() {
        let pedido = format!("agrándala a 30 actividades y 30 semanas{RESUMEN_CON_PLANTILLA}");
        let poner = Llamada::Figuras(crate::figuras::PedidoFiguras::Poner {
            nombre: "cronograma".into(),
        });
        let error = duplicaria(&pedido, &poner).expect("poner otro se para");
        assert!(
            error.starts_with("ERROR") && error.contains("alargar"),
            "{error}"
        );
        // Dibujar piezas sueltas (una zona, una nota) no se para: que no sea
        // otra version lo decide el modelo entendiendo el pedido.
        assert!(duplicaria(&pedido, &dibujar()).is_none());
        // Lo demas de la biblioteca (alargar, la lista) si se hace.
        let alargar = Llamada::Figuras(crate::figuras::PedidoFiguras::Lista);
        assert!(duplicaria(&pedido, &alargar).is_none());
    }

    #[test]
    fn la_guarda_no_estorba_lo_que_si_es_nuevo() {
        // Lienzo vacio: no hay nada que duplicar.
        assert!(
            duplicaria(
                "agranda el cronograma\n\n[Lienzo destino: vacio.]",
                &dibujar()
            )
            .is_none()
        );
        // Sin resumen (no se sabe que hay): no se adivina.
        assert!(duplicaria("agranda el cronograma", &dibujar()).is_none());
        // Pide otro expresamente.
        let otro = format!("haz otro cronograma más grande al lado{RESUMEN_CON_PLANTILLA}");
        assert!(duplicaria(&otro, &dibujar()).is_none());
        // No es cambiar lo que hay.
        let casa = format!("dibuja una casa{RESUMEN_CON_PLANTILLA}");
        assert!(duplicaria(&casa, &dibujar()).is_none());
        // Agrandar y anadirle piezas: dibujar sueltas si vale.
        let tejado = format!("agranda esta figura y ponle un tejado{RESUMEN_CON_PLANTILLA}");
        assert!(duplicaria(&tejado, &dibujar()).is_none());
        // Una zona nueva junto a lo que hay, sin verbo de anadir.
        let zona = format!(
            "hazlo de 10 semanas y abajo una zona gris para anotaciones{RESUMEN_CON_PLANTILLA}"
        );
        assert!(duplicaria(&zona, &dibujar()).is_none());
    }

    /// Las marcas son las que pone de verdad la app: si el resumen o
    /// `anadir_resumen` cambian su texto, esto lo dice.
    #[test]
    fn las_marcas_son_las_del_resumen_de_verdad() {
        let mut h = vec![crate::motor::Mensaje::usuario("agranda el cronograma")];
        let vacio = crate::edicion::resumen(
            &pixpin_motor2d::Escena::nueva(),
            &crate::edicion::Contexto::default(),
        );
        crate::edicion::anadir_resumen(&mut h, &vacio);
        let t = h[0].texto_entero();
        assert!(t.contains(MARCA_RESUMEN), "{t}");
        assert!(!lienzo_con_algo(&t), "vacio no es «con algo»");
        assert_eq!(sin_anexos(&t), "agranda el cronograma");
        // Los avisos del turno no se confunden con el pedido.
        assert!(EMPUJON_ANUNCIO.starts_with(crate::agente::MARCA_DEL_TURNO));
        assert!(
            crate::pasos::empujon(&[crate::pasos::Paso {
                paso: "x".into(),
                estado: crate::orquesta::EstadoTarea::Pendiente,
                nota: String::new(),
            }])
            .unwrap()
            .starts_with(crate::agente::MARCA_DEL_TURNO)
        );
    }

    #[test]
    fn anunciar_no_es_acabar() {
        // El caso del usuario (25-oct-2026), tras la revision final.
        assert!(anuncia_pendiente(
            "Todavía no está listo el cronograma de 19 semanas y 30 actividades, quedó a medias y desordenado… \
ahora voy a limpiar el resto y hacerlo por completo, alineado y revisado"
        ));
        assert!(anuncia_pendiente(
            "Agrandé las filas. A continuación ajusto las semanas."
        ));
        assert!(anuncia_pendiente("Procedo a numerar las actividades."));
        assert!(anuncia_pendiente("Voy a corregir la columna de semanas."));
        // Negativos: lo acabado, una oferta y una pregunta cierran.
        assert!(!anuncia_pendiente(
            "Listo: el cronograma ya tiene 30 actividades y 30 semanas."
        ));
        assert!(!anuncia_pendiente(
            "Hecho. Si quieres, voy a colorear las semanas."
        ));
        assert!(!anuncia_pendiente(
            "¿Quieres que lo haga también con las semanas?"
        ));
        assert!(!anuncia_pendiente("Sigo con lo más razonable."));
        assert!(!anuncia_pendiente("Faltaban los atajos: ya están."));
        assert!(!anuncia_pendiente("No pude: el lienzo esta cerrado."));
        // Lo que «iba a» hacer al principio de un resumen largo no cuenta.
        let largo = format!(
            "Voy a explicarlo. {} Listo, quedó como pediste.",
            "Todo bien. ".repeat(80)
        );
        assert!(!anuncia_pendiente(&largo));
    }
}
