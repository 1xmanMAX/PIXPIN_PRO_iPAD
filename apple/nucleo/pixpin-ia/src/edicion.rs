//! **Editar lo que ya hay en el lienzo**: elegir por descripcion, recortar y
//! pegar imagenes, trazar a mano (lapiz, grafito, resaltador), geometria
//! precisa y cortar o extender lineas como en CAD.
//!
//! Pedido del usuario (14-oct-2026): «mejora el acceso del modelo a las
//! herramientas: hacer recorte de imagenes, pegar imagenes, usar el lapiz, el
//! grafito, el resaltador, las rectas, circulos, corte de lineas, extension y
//! mas». Hasta aqui el asistente solo CREABA (`dibujar_en_lienzo` y
//! compania escriben Excalidraw nuevo); ahora tambien toca lo que hay.
//!
//! ## Las reglas de la casa, y como se cumplen
//!
//! - **El mismo motor del lienzo, las mismas operaciones que el raton.** Nada
//!   se reimplementa aqui: el lapiz, el grafito, el resaltador, la recta, el
//!   ovalo y el arco nacen de un [`pixpin_motor2d::gesto::Gesto`] al que se le
//!   dan pulsar, mover y soltar —la maquina de estados del editor, sin
//!   ventana—; recortar y extender son `recorte::recortar_en_escena` y
//!   `extender_en_escena` (los mismos que la herramienta del editor);
//!   el estilo es `estilo::aplicar_a`; agrupar y ordenar, `organizar`;
//!   duplicar, `portapapeles`; girar y voltear, `transformar`; el recorte de
//!   una foto, `recorte_imagen` (nuevo en el motor, para que lo use tambien
//!   la UI).
//! - **Un componente con variables.** Las herramientas salen de UNA tabla
//!   ([`FILAS`]): nombre, descripcion, campos con su tipo. El JSON Schema y
//!   la validacion son los mismos para todas ([`herramienta`], [`validar`]).
//! - **Un solo paso de deshacer por llamada** ([`aplicar`] junta lo que
//!   hagan los gestos con `Escena::juntar_pasos`), y como todo va a la
//!   escena por su camino de siempre, se guarda y se sincroniza igual que lo
//!   dibujado a mano.
//! - **Minima latencia**: todo es Rust puro sobre la escena en memoria
//!   (microsegundos); nada de red ni de disco aqui.
//!
//! ## Elegir lo que ya hay
//!
//! Cada herramienta nombra su objetivo con una **referencia** ([`referencia`]):
//! un `#id` del resumen, «el circulo rojo», «el texto "Total"», «lo ultimo»,
//! «lo elegido», «la recta de abajo», o una zona. Si encaja mas de uno donde
//! hace falta uno, el modelo recibe la lista corta para elegir por su `#id`.
//! Cada turno el modelo recibe un **resumen compacto** del lienzo destino
//! ([`resumen`]): tipos, `#id`, cajas, colores y textos, recortado para no
//! gastar tokens.
//!
//! ## Las coordenadas
//!
//! Las del modelo son las de siempre (como en `dibujar_en_lienzo`), medidas
//! desde un **marco** ([`Contexto::marco`]): la esquina de lo que se ve si hay
//! un lienzo abierto, o la de lo dibujado si no. El resumen y los resultados
//! van en ese mismo marco, asi que lo que el modelo lee lo puede usar tal cual.

pub mod hacer;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_textos;
pub mod referencia;
pub mod textos;

use serde_json::{Map, Value, json};

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};

use crate::herramientas::{ErrorHerramienta, Herramienta};

pub const SELECCIONAR: &str = "seleccionar_en_lienzo";
pub const TRANSFORMAR: &str = "transformar_elementos";
pub const ESTILO: &str = "cambiar_estilo";
pub const TRAZAR: &str = "trazar_a_mano";
pub const GEOMETRIA: &str = "geometria_precisa";
pub const CORTAR: &str = "cortar_linea";
pub const EXTENDER: &str = "extender_linea";
pub const IMAGEN: &str = "imagen_en_lienzo";
pub const EDITAR_TEXTO: &str = "editar_texto";
pub const REORGANIZAR: &str = "reorganizar_hoja";

/// El tipo de un campo de la tabla.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tipo {
    Texto,
    Numero,
    /// Lista de numeros (puntos, una caja, una zona).
    Numeros,
    Opciones(&'static [&'static str]),
    /// Lo que ya hay en el lienzo, dicho como sea ([`referencia`]).
    Referencia,
    /// Un texto que se escribe tal cual en el lienzo (una traduccion, un
    /// parrafo): sin el recorte a 300 letras de [`Tipo::Texto`], que corta
    /// a medias un parrafo traducido sin avisar.
    TextoLargo,
    /// Una lista de cambios de texto `[{ref, texto}]`: traducir o corregir
    /// una figura entera es UNA llamada y no una por rotulo.
    Cambios,
}

#[derive(Debug, Clone, Copy)]
pub struct Campo {
    pub nombre: &'static str,
    pub tipo: Tipo,
    pub obligatorio: bool,
    pub descripcion: &'static str,
}

/// Una herramienta de edicion.
#[derive(Debug, Clone, Copy)]
pub struct Fila {
    pub nombre: &'static str,
    pub descripcion: &'static str,
    pub campos: &'static [Campo],
    /// Raices (sin tildes) que la delatan en el pedido, para elegir las
    /// pocas que ve el modelo local en cada turno (`seleccion.rs`).
    pub claves: &'static [&'static str],
}

const fn c(
    nombre: &'static str,
    tipo: Tipo,
    obligatorio: bool,
    descripcion: &'static str,
) -> Campo {
    Campo {
        nombre,
        tipo,
        obligatorio,
        descripcion,
    }
}

/// Lo que se dice de una referencia en todas las herramientas.
pub const QUE: &str = "lo que ya hay: «#12», «#3 #5», «el circulo rojo», «la recta de abajo», «el texto \"Total\"», «el titulo», «lo ultimo», «lo elegido», «todo»";

const COLOR: Campo = c(
    "color",
    Tipo::Texto,
    false,
    "rojo, azul, verde, amarillo, naranja, morado, rosa, gris, marron, negro, blanco o #rrggbb",
);
const GROSOR: Campo = c("grosor", Tipo::Numero, false, "grosor del trazo, 1 a 40");
const TRAZO: Campo = c(
    "trazo",
    Tipo::Opciones(&["solido", "discontinuo", "punteado", "raya_punto"]),
    false,
    "",
);
const DUREZA: Campo = c(
    "dureza",
    Tipo::Opciones(&["4H", "2H", "HB", "2B", "4B", "6B", "8B"]),
    false,
    "dureza del grafito",
);

/// **La tabla.** Cada fila es una herramienta; [`herramienta`] la convierte
/// en JSON Schema y [`validar`] comprueba lo que pide el modelo.
pub const FILAS: &[Fila] = &[
    Fila {
        nombre: SELECCIONAR,
        descripcion: "Busca en el lienzo lo que ya hay y devuelve sus #id (para usarlos en las demas herramientas de editar). El resumen del lienzo ya trae los #id: usala solo si no lo encuentras ahi o para buscar en una zona.",
        campos: &[
            c("que", Tipo::Referencia, true, QUE),
            c(
                "zona",
                Tipo::Numeros,
                false,
                "[x, y, ancho, alto]: solo lo que cae dentro",
            ),
        ],
        claves: &[
            "elige",
            "selecciona",
            "busca en el lienzo",
            "cual es",
            "que hay en el lienzo",
        ],
    },
    Fila {
        nombre: TRANSFORMAR,
        descripcion: "Mueve, escala, gira, duplica, borra, agrupa, desagrupa, voltea o cambia el orden (al frente, al fondo) de lo que ya hay en el lienzo.",
        campos: &[
            c("que", Tipo::Referencia, true, QUE),
            c(
                "accion",
                Tipo::Opciones(&[
                    "mover",
                    "escalar",
                    "girar",
                    "duplicar",
                    "borrar",
                    "agrupar",
                    "desagrupar",
                    "al_frente",
                    "al_fondo",
                    "voltear_horizontal",
                    "voltear_vertical",
                ]),
                true,
                "",
            ),
            c(
                "dx",
                Tipo::Numero,
                false,
                "mover/duplicar: desplazamiento en x (px)",
            ),
            c(
                "dy",
                Tipo::Numero,
                false,
                "mover/duplicar: desplazamiento en y (px)",
            ),
            c(
                "a",
                Tipo::Numeros,
                false,
                "mover: [x, y] adonde va su esquina de arriba a la izquierda",
            ),
            c(
                "factor",
                Tipo::Numero,
                false,
                "escalar: 2 = el doble, 0.5 = la mitad (desde su centro)",
            ),
            c(
                "grados",
                Tipo::Numero,
                false,
                "girar: grados, positivo = antihorario",
            ),
        ],
        claves: &[
            "mueve",
            "mover",
            "desplaza",
            "escala",
            "agranda",
            "achica",
            "encoge",
            "gira",
            "rota",
            "duplica",
            "copia el",
            "copia la",
            "borra",
            "elimina",
            "quita",
            "agrupa",
            "desagrupa",
            "voltea",
            "espejo",
            "al frente",
            "al fondo",
            "detras",
            "delante",
            "mas grande",
            "mas pequen",
        ],
    },
    Fila {
        nombre: ESTILO,
        descripcion: "Cambia el color, el relleno, el grosor, el trazo, la opacidad o el material de lo que ya hay en el lienzo.",
        campos: &[
            c("que", Tipo::Referencia, true, QUE),
            COLOR,
            c(
                "relleno",
                Tipo::Texto,
                false,
                "color de relleno, o «ninguno» para quitarlo",
            ),
            GROSOR,
            TRAZO,
            c("opacidad", Tipo::Numero, false, "0 a 100"),
            c(
                "material",
                Tipo::Opciones(&["lisa", "tiza", "grafito"]),
                false,
                "de que esta hecha la tinta",
            ),
            DUREZA,
        ],
        claves: &[
            "color",
            "pinta de",
            "pintalo",
            "pintala",
            "rellena",
            "relleno",
            "grosor",
            "mas grueso",
            "mas fino",
            "discontinu",
            "punteado",
            "opacidad",
            "transparente",
            "cambia",
        ],
    },
    Fila {
        nombre: TRAZAR,
        descripcion: "Traza a mano alzada con el lapiz, el grafito o el resaltador, como con el raton: por unos puntos, o sobre algo del lienzo para subrayarlo, resaltarlo, rodearlo, tacharlo o repasarlo.",
        campos: &[
            c(
                "herramienta",
                Tipo::Opciones(&["lapiz", "grafito", "resaltador"]),
                true,
                "",
            ),
            c(
                "puntos",
                Tipo::Numeros,
                false,
                "[x1, y1, x2, y2, ...] por donde pasa el trazo",
            ),
            c("sobre", Tipo::Referencia, false, QUE),
            c(
                "como",
                Tipo::Opciones(&["subrayar", "resaltar", "rodear", "tachar", "repasar"]),
                false,
                "con «sobre»; omitir: resaltar con el resaltador, subrayar con lo demas",
            ),
            COLOR,
            GROSOR,
            DUREZA,
        ],
        claves: &[
            "subraya",
            "resalta",
            "marca",
            "rodea",
            "encierra",
            "tacha",
            "repasa",
            "a mano",
            "lapiz",
            "grafito",
            "resaltador",
            "marcador",
            "fosforito",
        ],
    },
    Fila {
        nombre: GEOMETRIA,
        descripcion: "Dibuja geometria exacta de lamina: recta (con flecha si se pide) por dos puntos o por punto, angulo y longitud; circulo por centro y radio o por tres puntos; arco por centro y angulos o por tres puntos; semicircunferencia sobre un diametro; cuadrante, sector y segmento circular (rellenos, tambien con degradado); marca de angulo entre dos lados (arquitos, relleno translucido, valor) y la del angulo recto; punto con su letra; perpendicular o paralela a una recta que ya esta. Angulos en grados: 0 a la derecha, 90 arriba.",
        campos: &[
            c(
                "figura",
                Tipo::Opciones(&[
                    "recta",
                    "circulo",
                    "arco",
                    "semicircunferencia",
                    "cuadrante",
                    "sector",
                    "segmento",
                    "marca_angulo",
                    "angulo_recto",
                    "punto",
                    "perpendicular",
                    "paralela",
                ]),
                true,
                "",
            ),
            c(
                "puntos",
                Tipo::Numeros,
                false,
                "recta: [x1, y1, x2, y2]; circulo por tres puntos: [x1, y1, x2, y2, x3, y3]; arco por tres puntos: [inicio, por donde pasa, fin]; semicircunferencia: [x1, y1, x2, y2] del diametro; cuadrante con «centro»: [ax, ay, bx, by] las puntas de sus dos radios; marca_angulo y angulo_recto: [x1, y1, x2, y2] un punto de cada lado; punto: [x, y]",
            ),
            c(
                "desde",
                Tipo::Numeros,
                false,
                "recta: [x, y] de donde sale, con «angulo» y «longitud»",
            ),
            c("angulo", Tipo::Numero, false, "recta: grados"),
            c("longitud", Tipo::Numero, false, "recta, perpendicular: px"),
            c(
                "centro",
                Tipo::Numeros,
                false,
                "circulo, arco, cuadrante, sector, segmento: [x, y]; punto: [x, y]",
            ),
            c(
                "radio",
                Tipo::Numero,
                false,
                "circulo, arco, cuadrante, sector, segmento; marca_angulo: radio de la marca (22); angulo_recto: lado del cuadradito (14)",
            ),
            c(
                "inicio",
                Tipo::Numero,
                false,
                "arco, cuadrante, sector, segmento: grados donde empieza",
            ),
            c(
                "fin",
                Tipo::Numero,
                false,
                "arco, sector, segmento: grados donde acaba",
            ),
            c(
                "sentido",
                Tipo::Opciones(&["antihorario", "horario"]),
                false,
                "arco, cuadrante, sector, segmento: de «inicio» a «fin» (omitir: antihorario)",
            ),
            c(
                "entre",
                Tipo::Texto,
                false,
                "marca_angulo, angulo_recto y punto: las DOS rectas que ya estan y se cruzan, por su #id («#3 #5»): el vertice (o el punto) y los lados salen exactos; MEJOR que dar coordenadas",
            ),
            c(
                "cerca",
                Tipo::Numeros,
                false,
                "marca_angulo con «entre»: [x, y] un punto dentro del angulo que se marca (de los cuatro del cruce); omitir: el de los extremos mas lejanos",
            ),
            c(
                "vertice",
                Tipo::Numeros,
                false,
                "marca_angulo y angulo_recto sin «entre»: [x, y] del vertice",
            ),
            c(
                "lado",
                Tipo::Numeros,
                false,
                "semicircunferencia: [x, y] un punto del lado al que va (omitir: arriba)",
            ),
            c("arcos", Tipo::Numero, false, "marca_angulo: 1 a 3 arquitos"),
            c(
                "valor",
                Tipo::Texto,
                false,
                "marca_angulo: lo que se escribe al lado («45°», «θ», «x»); «griega»: la siguiente letra griega libre (α, β, θ...); «medida»: los grados medidos. Con «entre», un angulo de 90° sale solo con su cuadradito",
            ),
            c(
                "letra",
                Tipo::Texto,
                false,
                "punto: su letra («A», «P», «O»); «siguiente»: la siguiente libre (con «entre» y sin letra, tambien)",
            ),
            c(
                "letra_hacia",
                Tipo::Opciones(&[
                    "arriba",
                    "abajo",
                    "izquierda",
                    "derecha",
                    "arriba_izquierda",
                    "arriba_derecha",
                    "abajo_izquierda",
                    "abajo_derecha",
                ]),
                false,
                "punto: donde va la letra (omitir: donde no estorba)",
            ),
            c(
                "tam_letra",
                Tipo::Numero,
                false,
                "punto y marca_angulo: tamano de la letra",
            ),
            c(
                "relleno",
                Tipo::Texto,
                false,
                "cuadrante, sector, segmento, circulo: color de relleno; marca_angulo: el suyo (translucido; omitir: el del color)",
            ),
            c(
                "degradado",
                Tipo::Opciones(&["lineal", "radial"]),
                false,
                "relleno degradado: de «relleno» a «relleno_hasta»",
            ),
            c(
                "relleno_hasta",
                Tipo::Texto,
                false,
                "con «degradado»: el otro color (omitir: casi blanco)",
            ),
            c(
                "flecha",
                Tipo::Opciones(&["ninguna", "fin", "inicio", "ambas"]),
                false,
                "recta: puntas de flecha",
            ),
            c(
                "punta",
                Tipo::Opciones(&["rellena", "abierta"]),
                false,
                "con «flecha»: la forma de la punta (omitir: rellena)",
            ),
            c(
                "a",
                Tipo::Referencia,
                false,
                "perpendicular/paralela: la recta que ya esta",
            ),
            c(
                "por",
                Tipo::Numeros,
                false,
                "perpendicular/paralela: [x, y] por donde pasa",
            ),
            c(
                "distancia",
                Tipo::Numero,
                false,
                "paralela: a cuantos px (positivo: a la izquierda de la recta)",
            ),
            COLOR,
            GROSOR,
            TRAZO,
        ],
        claves: &[
            "recta",
            "circulo",
            "circunferencia",
            "arco",
            "perpendicular",
            "paralela",
            "radio",
            "centro",
            "angulo",
            "grados",
            "tres puntos",
            "exact",
            "segmento",
            "semicircunferencia",
            "semicirculo",
            "cuadrante",
            "sector",
            "cuerda",
            "diametro",
            "tangente",
            "marca",
            "angulo recto",
            "degradado",
            "punteada",
            "raya punto",
            "lamina",
            "geometria",
        ],
    },
    Fila {
        nombre: CORTAR,
        descripcion: "Corta (recorta, trim de CAD) una linea donde la cruza otra figura y quita el trozo que sobra. Sin «quitar» se quita el trozo mas corto de un extremo.",
        campos: &[
            c("que", Tipo::Referencia, true, "la linea que se corta"),
            c(
                "con",
                Tipo::Referencia,
                false,
                "contra que figura se corta (omitir: contra todas)",
            ),
            c(
                "quitar",
                Tipo::Opciones(&["inicio", "fin", "medio", "corto", "largo"]),
                false,
                "que trozo se va: el del inicio, el del fin, el del medio, el mas corto o el mas largo",
            ),
            c(
                "punto",
                Tipo::Numeros,
                false,
                "[x, y] dentro del trozo que se quita (en vez de «quitar»)",
            ),
        ],
        claves: &[
            "corta",
            "cortar",
            "recorta la",
            "recorta el",
            "trim",
            "quita el trozo",
            "quita lo que sobra",
            "en el cruce",
            "interseccion",
        ],
    },
    Fila {
        nombre: EXTENDER,
        descripcion: "Extiende (alarga, extend de CAD) una linea por su extremo hasta que toca otra figura.",
        campos: &[
            c("que", Tipo::Referencia, true, "la linea que se alarga"),
            c(
                "hasta",
                Tipo::Referencia,
                false,
                "hasta que figura (omitir: la primera que encuentre)",
            ),
            c(
                "extremo",
                Tipo::Opciones(&["inicio", "fin"]),
                false,
                "omitir: el mas cercano a «hasta», o el fin",
            ),
        ],
        claves: &[
            "extiende",
            "extender",
            "alarga",
            "prolonga",
            "estira",
            "hasta que toque",
            "extend",
        ],
    },
    Fila {
        nombre: IMAGEN,
        descripcion: "Pega en el lienzo una imagen adjunta o la del portapapeles, o recorta una imagen que ya esta (conservar un trozo, o quitar arriba, abajo, izquierda o derecha). Para moverla, escalarla o girarla, transformar_elementos.",
        campos: &[
            c("accion", Tipo::Opciones(&["pegar", "recortar"]), true, ""),
            c(
                "imagen",
                Tipo::Texto,
                false,
                "pegar: nombre del adjunto, o «portapapeles»; omitir: la ultima imagen adjunta",
            ),
            c(
                "caja",
                Tipo::Numeros,
                false,
                "pegar: [x, y, ancho, alto] donde va (omitir el alto: se conserva la proporcion)",
            ),
            c(
                "que",
                Tipo::Referencia,
                false,
                "recortar: la imagen (omitir: la ultima)",
            ),
            c(
                "conservar",
                Tipo::Numeros,
                false,
                "recortar: [x, y, ancho, alto] en fracciones 0 a 1 de la imagen; [0, 0, 0.5, 1] = la mitad izquierda",
            ),
            c(
                "quitar",
                Tipo::Opciones(&["arriba", "abajo", "izquierda", "derecha"]),
                false,
                "recortar: el lado que se quita",
            ),
            c(
                "cuanto",
                Tipo::Numero,
                false,
                "con «quitar»: fraccion que se quita (0.5 = la mitad)",
            ),
        ],
        claves: &[
            "pega",
            "pegar",
            "inserta",
            "recorta",
            "recortar",
            "foto",
            "imagen",
            "portapapeles",
            "captura",
        ],
    },
    Fila {
        nombre: EDITAR_TEXTO,
        descripcion: "Cambia lo que DICEN los textos que ya estan en el lienzo (tambien los rotulos dentro de cajas) sin moverlos ni cambiar su letra, color o alineacion: traducir, corregir, renombrar. Para traducir o corregir una figura entera manda TODOS sus textos en UNA llamada con «cambios»; cada caja se vuelve a medir y lo centrado sigue centrado, y el resultado dice que #id cambiaron y cuales ya no caben. Con «ajustar» encoge la letra o parte renglones para que quepa en su caja; con accion «listar» devuelve el texto completo y el #id de cada texto (usala si el resumen no los trae todos). No la uses para crear textos nuevos (dibujar_en_lienzo) ni para el tamano o el color (cambiar_estilo). Ejemplo: {\"cambios\": [{\"ref\": \"#3\", \"texto\": \"开始\"}, {\"ref\": \"#5\", \"texto\": \"结束\"}], \"ajustar\": \"auto\"}",
        campos: &[
            c(
                "cambios",
                Tipo::Cambios,
                false,
                "[{\"ref\": \"#3\", \"texto\": \"nuevo\"}, ...]: cada texto (o caja con rotulo) por su #id y lo que pasa a decir, completo",
            ),
            c(
                "que",
                Tipo::Referencia,
                false,
                "un solo texto (con «texto»), o lo que se ajusta o se lista: «#3», «todo», «los textos»",
            ),
            c(
                "texto",
                Tipo::TextoLargo,
                false,
                "con «que»: lo que pasa a decir",
            ),
            c(
                "accion",
                Tipo::Opciones(&["cambiar", "ajustar", "listar"]),
                false,
                "omitir: cambiar si hay «cambios» o «texto», ajustar si solo hay «ajustar», listar si no",
            ),
            c(
                "ajustar",
                Tipo::Opciones(&["no", "encoger", "partir", "auto"]),
                false,
                "si no cabe en su caja (o en «ancho_max»): encoger la letra, partir en renglones, o auto (partir y, si aun no cabe, encoger). Omitir: no se toca y se avisa",
            ),
            c(
                "ancho_max",
                Tipo::Numero,
                false,
                "con «ajustar»: el ancho maximo en px (omitir: el de la caja donde esta)",
            ),
            c(
                "desde",
                Tipo::Numero,
                false,
                "listar: saltarse los primeros N (para listas largas)",
            ),
        ],
        claves: &[
            "traduc",
            "renombr",
            "tilde",
            "ortograf",
            "reescrib",
            "cambia el texto",
            "cambia los textos",
            "corrige el texto",
            "corrige los textos",
            "al ingles",
            "al chino",
            "al espanol",
            "no cabe",
            "se sale",
        ],
    },
    // 25-oct-2026, el usuario: «quiero poner el recuadro que esta arriba
    // abajo y reorganizar la pagina … que utilice el mismo codigo … que no lo
    // rehaga usando las herramientas del canvas».
    Fila {
        nombre: REORGANIZAR,
        descripcion: "Reorganiza una hoja que YA esta en el lienzo (una plantilla, una figura, un esquema) moviendo SUS PROPIAS piezas por bloques (cada recuadro con lo que lleva y su titulo): mover un bloque arriba o abajo, ponerlo antes o despues de otro, intercambiar dos, duplicar, borrar, estirarlo o anadirle filas copiando la ultima. Todo conserva su diseño y la hoja queda ordenada (lo de debajo se recoloca y la pagina crece si hace falta). Usala SIEMPRE para reorganizar, ampliar o anadir filas a algo que ya existe: NUNCA lo borres para redibujarlo con dibujar_en_lienzo. Primero «ver» da la lista de bloques (B1, B2...) con su titulo; luego nombra el bloque por su B# o su titulo. Ejemplos: {\"accion\":\"ver\"}, {\"accion\":\"mover\",\"bloque\":\"Prioridades\",\"donde\":\"abajo\"}, {\"accion\":\"intercambiar\",\"bloque\":\"B1\",\"con\":\"B3\"}, {\"accion\":\"anadir_filas\",\"bloque\":\"Tareas\",\"filas\":10}",
        campos: &[
            c(
                "accion",
                Tipo::Opciones(&[
                    "ver",
                    "mover",
                    "intercambiar",
                    "duplicar",
                    "borrar",
                    "estirar",
                    "anadir_filas",
                ]),
                true,
                "",
            ),
            c(
                "bloque",
                Tipo::Texto,
                false,
                "el bloque: «B2» o (parte de) su titulo",
            ),
            c(
                "con",
                Tipo::Texto,
                false,
                "intercambiar, o mover con donde antes/despues: el otro bloque",
            ),
            c(
                "donde",
                Tipo::Opciones(&["arriba", "abajo", "antes", "despues"]),
                false,
                "mover: al principio o al final de su columna, o antes/despues (encima/debajo) de «con»",
            ),
            c(
                "cuanto",
                Tipo::Numero,
                false,
                "estirar: cuanto mas alto (px)",
            ),
            c("filas", Tipo::Numero, false, "anadir_filas: cuantas"),
            c(
                "que",
                Tipo::Referencia,
                false,
                "la hoja, si hay varias en el lienzo: «lo elegido», «#12»... Omitir: la elegida o la unica",
            ),
        ],
        claves: &[
            "reorganiz",
            "reordena",
            "reacomod",
            "intercambi",
            "cambia de sitio",
            "cambia de lugar",
            "pon arriba",
            "ponlo arriba",
            "ponlo abajo",
            "pon abajo",
            "muevelo",
            "mueve el recuadro",
            "mueve la seccion",
            "mueve el bloque",
            "recuadro",
            "seccion",
            "bloque",
            "anade filas",
            "mas filas",
            "agrega filas",
            "duplica la seccion",
            "duplica el recuadro",
        ],
    },
];

pub fn filas() -> &'static [Fila] {
    FILAS
}

pub fn fila(nombre: &str) -> Option<&'static Fila> {
    FILAS.iter().find(|f| f.nombre == nombre)
}

/// El JSON Schema de una fila.
pub fn herramienta(f: &Fila) -> Herramienta {
    let mut propiedades = Map::new();
    let mut requeridos = Vec::new();
    for campo in f.campos {
        let mut p = match campo.tipo {
            Tipo::Texto | Tipo::Referencia | Tipo::TextoLargo => json!({"type": "string"}),
            Tipo::Cambios => json!({
                "type": "array",
                "maxItems": MAX_CAMBIOS,
                "items": {
                    "type": "object",
                    "required": ["ref", "texto"],
                    "properties": {
                        "ref": {"type": "string", "description": "#id del texto o de la caja que lo lleva"},
                        "texto": {"type": "string", "description": "lo que pasa a decir"}
                    }
                }
            }),
            Tipo::Numero => json!({"type": "number"}),
            Tipo::Numeros => {
                json!({"type": "array", "items": {"type": "number"}, "maxItems": crate::herramientas::MAX_NUMEROS_PUNTOS})
            }
            Tipo::Opciones(o) => json!({"type": "string", "enum": o}),
        };
        if !campo.descripcion.is_empty() {
            p["description"] = json!(campo.descripcion);
        }
        propiedades.insert(campo.nombre.into(), p);
        if campo.obligatorio {
            requeridos.push(campo.nombre);
        }
    }
    Herramienta {
        nombre: f.nombre.into(),
        descripcion: f.descripcion.into(),
        parametros: json!({"type": "object", "required": requeridos, "properties": propiedades}),
    }
}

/// Todas las herramientas de editar.
pub fn herramientas() -> Vec<Herramienta> {
    FILAS.iter().map(herramienta).collect()
}

/// **Los campos que solo ve la nube** (banco de documentos, 15-oct: tras
/// pasar una tabla al lienzo, «el texto se sale de las celdas» y no habia
/// como cambiar el tamano de letra). Son de la misma fila: misma validacion
/// y mismo `hacer`; el esquema del local no cambia.
pub const CAMPOS_NUBE: &[(&str, Campo)] = &[(
    ESTILO,
    c(
        "tam_letra",
        Tipo::Numero,
        false,
        "tamano de la letra de los textos, en px (14 normal, 24 titulo)",
    ),
)];

/// Pone en el esquema de la nube sus [`CAMPOS_NUBE`] (lo llama
/// `herramientas::para_la_nube`).
pub fn para_la_nube(h: &mut Herramienta) {
    for (_, campo) in CAMPOS_NUBE.iter().filter(|(n, _)| *n == h.nombre) {
        let Some(p) = h
            .parametros
            .pointer_mut("/properties")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        if p.contains_key(campo.nombre) {
            continue;
        }
        let tipo = match campo.tipo {
            Tipo::Numero => "number",
            _ => "string",
        };
        p.insert(
            campo.nombre.into(),
            json!({"type": tipo, "description": campo.descripcion}),
        );
    }
}

/// Una llamada de editar ya validada: su fila y sus argumentos limpios.
#[derive(Debug, Clone, PartialEq)]
pub struct Edicion {
    pub herramienta: &'static str,
    pub args: Map<String, Value>,
}

impl Edicion {
    pub fn texto(&self, campo: &str) -> Option<&str> {
        self.args.get(campo).and_then(Value::as_str)
    }
    pub fn numero(&self, campo: &str) -> Option<f64> {
        self.args.get(campo).and_then(Value::as_f64)
    }
    pub fn numeros(&self, campo: &str) -> Option<Vec<f64>> {
        self.args
            .get(campo)
            .and_then(Value::as_array)
            .map(|v| v.iter().filter_map(Value::as_f64).collect())
    }
}

/// Cuantos cambios de texto como mucho en una llamada (una figura grande
/// traducida entera cabe de sobra).
pub const MAX_CAMBIOS: usize = 300;
/// Cuantas letras como mucho en un texto nuevo.
pub const MAX_LETRAS_TEXTO: usize = 4000;

/// Un texto del modelo, recortado a [`MAX_LETRAS_TEXTO`] y sin espacios a
/// los lados (los saltos de renglon de dentro se respetan).
fn texto_largo(s: &str) -> String {
    s.trim().chars().take(MAX_LETRAS_TEXTO).collect()
}

const FORMA_DE_CAMBIOS: &str =
    "«cambios» tiene que ser una lista [{\"ref\": \"#3\", \"texto\": \"...\"}]";

/// **Los cambios de texto como los mande el modelo**, a `[{ref, texto}]`:
/// la lista de objetos (con `ref`, `que`, `id` o `referencia`; `texto`,
/// `text` o `nuevo`), un mapa `{"#3": "hola"}`, o todo eso metido en un
/// texto JSON (los modelos pequenos lo hacen).
fn cambios_de(v: &Value) -> Result<Vec<Value>, String> {
    let como_ref = |x: &Value| match x {
        Value::Number(n) => Some(format!("#{n}")),
        Value::String(s) if !s.trim().is_empty() => {
            let t = s.trim();
            // «12» suelto es el #12.
            Some(if t.chars().all(|c| c.is_ascii_digit()) {
                format!("#{t}")
            } else {
                t.to_string()
            })
        }
        _ => None,
    };
    let lista: Vec<(Option<String>, Option<String>)> = match v {
        Value::String(s) => {
            let dentro: Value =
                serde_json::from_str(s.trim()).map_err(|_| FORMA_DE_CAMBIOS.to_string())?;
            if dentro.is_string() {
                return Err(FORMA_DE_CAMBIOS.into());
            }
            return cambios_de(&dentro);
        }
        Value::Array(a) => a
            .iter()
            .map(|x| {
                let r = ["ref", "que", "id", "referencia"]
                    .iter()
                    .find_map(|k| x.get(*k))
                    .and_then(como_ref);
                let t = ["texto", "text", "nuevo"]
                    .iter()
                    .find_map(|k| x.get(*k))
                    .and_then(Value::as_str)
                    .map(texto_largo);
                (r, t)
            })
            .collect(),
        Value::Object(m) => m
            .iter()
            .map(|(k, x)| (como_ref(&json!(k)), x.as_str().map(texto_largo)))
            .collect(),
        _ => return Err(FORMA_DE_CAMBIOS.into()),
    };
    if lista.len() > MAX_CAMBIOS {
        return Err(format!(
            "«cambios» trae {} y caben {MAX_CAMBIOS}: partelos en dos llamadas",
            lista.len()
        ));
    }
    let mut salida = Vec::with_capacity(lista.len());
    for (i, (r, t)) in lista.into_iter().enumerate() {
        let r =
            r.ok_or_else(|| format!("al cambio {} le falta «ref» (el #id del texto)", i + 1))?;
        let t = t.filter(|t| !t.is_empty()).ok_or_else(|| {
            format!("al cambio {} ({r}) le falta «texto» o esta vacio; para borrar un texto usa transformar_elementos con accion borrar", i + 1)
        })?;
        salida.push(json!({"ref": r, "texto": t}));
    }
    if salida.is_empty() {
        return Err(
            "«cambios» esta vacio: pon al menos uno, o usa accion «listar» para ver los textos"
                .into(),
        );
    }
    Ok(salida)
}

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

/// Un numero, aceptando `"120"` (los modelos pequenos lo hacen).
fn como_numero(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        _ => None,
    }
    .filter(|x: &f64| x.is_finite())
}

/// **Valida una llamada de editar** contra su fila: tipos, obligatorios y
/// opciones. Lo que no esta en la fila se ignora (los modelos anaden campos).
pub fn validar(f: &'static Fila, args: &Map<String, Value>) -> Result<Edicion, ErrorHerramienta> {
    let mut limpio = Map::new();
    // Los campos de la nube ([`CAMPOS_NUBE`]) se validan igual; el local no
    // los ve en su esquema y no los manda.
    let de_la_nube = CAMPOS_NUBE
        .iter()
        .filter(|(h, _)| *h == f.nombre)
        .map(|(_, c)| c);
    for campo in f.campos.iter().chain(de_la_nube) {
        let Some(v) = args.get(campo.nombre).filter(|v| !v.is_null()) else {
            if campo.obligatorio {
                return Err(mal(format!("falta «{}»", campo.nombre)));
            }
            continue;
        };
        let n = campo.nombre;
        let bueno = match campo.tipo {
            Tipo::Texto | Tipo::Referencia => match v {
                Value::String(s) if !s.trim().is_empty() => {
                    json!(s.trim().chars().take(300).collect::<String>())
                }
                Value::Number(x) => json!(format!("#{x}")),
                // Una lista de ids: [12, 13] -> «#12 #13».
                Value::Array(a) if campo.tipo == Tipo::Referencia && !a.is_empty() => json!(
                    a.iter()
                        .map(|x| match x {
                            Value::Number(n) => format!("#{n}"),
                            Value::String(s) => s.clone(),
                            otro => otro.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
                _ if campo.obligatorio => return Err(mal(format!("«{n}» tiene que ser un texto"))),
                _ => continue,
            },
            Tipo::TextoLargo => match v {
                Value::String(s) if !s.trim().is_empty() => json!(texto_largo(s)),
                Value::Number(x) => json!(x.to_string()),
                _ if campo.obligatorio => return Err(mal(format!("«{n}» tiene que ser un texto"))),
                _ => continue,
            },
            Tipo::Cambios => json!(cambios_de(v).map_err(mal)?),
            Tipo::Numero => {
                json!(como_numero(v).ok_or_else(|| mal(format!("«{n}» tiene que ser un numero")))?)
            }
            Tipo::Numeros => {
                let lista: Vec<f64> = match v {
                    Value::Array(a) => a
                        .iter()
                        .flat_map(|x| match x {
                            // [[x, y], [x, y]] tambien vale.
                            Value::Array(par) => {
                                par.iter().filter_map(como_numero).collect::<Vec<_>>()
                            }
                            otro => como_numero(otro).into_iter().collect(),
                        })
                        .collect(),
                    Value::String(s) => s
                        .split(|ch: char| {
                            ch == ',' || ch == ';' || ch.is_whitespace() || ch == '[' || ch == ']'
                        })
                        .filter_map(|t| t.trim().parse::<f64>().ok())
                        .collect(),
                    _ => Vec::new(),
                };
                if lista.is_empty() {
                    return Err(mal(format!("«{n}» tiene que ser una lista de numeros")));
                }
                if lista.len() > crate::herramientas::MAX_NUMEROS_PUNTOS {
                    return Err(mal(format!("«{n}» tiene demasiados numeros")));
                }
                json!(lista)
            }
            Tipo::Opciones(o) => {
                let t = crate::acciones::normalizar(v.as_str().unwrap_or_default().trim())
                    .replace(' ', "_");
                match o.iter().find(|x| crate::acciones::normalizar(x) == t) {
                    Some(x) => json!(x),
                    None if campo.obligatorio => {
                        return Err(mal(format!("«{n}» tiene que ser una de: {}", o.join(", "))));
                    }
                    None => continue,
                }
            }
        };
        limpio.insert(n.into(), bueno);
    }
    Ok(Edicion {
        herramienta: f.nombre,
        args: limpio,
    })
}

// ---------------------------------------------------------------------------
// Lo que se sabe del lienzo en este turno

/// Lo que acompana a cada edicion: desde donde se miden las coordenadas del
/// modelo, lo que el usuario tiene elegido y lo ultimo que toco el asistente.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contexto {
    /// Lo que se suma a una coordenada del modelo para tenerla en la escena.
    pub marco: (f32, f32),
    /// Lo elegido en el editor abierto.
    pub elegidos: Vec<u64>,
    /// Lo que creo o toco la ultima llamada de este turno («lo ultimo»).
    pub ultimos: Vec<u64>,
}

impl Contexto {
    /// Un punto del modelo, en la escena.
    pub fn a_escena(&self, x: f64, y: f64) -> pixpin_motor2d::vector::Punto2 {
        pixpin_motor2d::vector::Punto2::nuevo(x as f32 + self.marco.0, y as f32 + self.marco.1)
    }
}

/// **El marco para un lienzo**: la esquina de lo dibujado (con un margen), o
/// el cero si esta vacio. Con un lienzo abierto, la app pasa la esquina de lo
/// que se ve.
pub fn marco_de(escena: &Escena) -> (f32, f32) {
    escena
        .caja()
        .filter(|c| c.0.is_finite() && c.1.is_finite())
        .map(|c| ((c.0 - 40.0).floor(), (c.1 - 40.0).floor()))
        .unwrap_or((0.0, 0.0))
}

/// Lo que hace falta de quien tiene las imagenes (el editor abierto, la hoja
/// cerrada o el banco): el motor no sabe de bitmaps.
pub trait Anfitrion {
    /// **Mete los pixeles de `fuente`** (un adjunto por su nombre, o
    /// «portapapeles»; vacio = la ultima imagen adjunta) en el almacen de
    /// imagenes del lienzo y devuelve el elemento `Imagen` listo (en 0,0, a
    /// su tamano natural) para colocarlo.
    fn imagen_nueva(&mut self, fuente: &str) -> Result<Elemento, String>;
    /// El tamano en pixeles del bitmap de una imagen del lienzo, si se sabe.
    fn tamano_de(&self, _id_objeto: u64) -> Option<(u32, u32)> {
        None
    }
}

/// Un anfitrion sin imagenes (pegar contesta que no se puede).
pub struct SinImagenes;

impl Anfitrion for SinImagenes {
    fn imagen_nueva(&mut self, _: &str) -> Result<Elemento, String> {
        Err("aqui no se pueden pegar imagenes".into())
    }
}

/// Lo que hizo una edicion: el texto para el modelo y los ids que tocó o
/// creó (pasan a ser «lo ultimo»).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hecho {
    pub texto: String,
    pub ids: Vec<u64>,
}

/// **Aplica una edicion a la escena**, en UN paso de deshacer. `Err` lleva el
/// motivo para el modelo (la referencia que no encaja, con la lista corta; la
/// linea que no se cruza con nada...) y entonces la escena queda como estaba.
pub fn aplicar(
    escena: &mut Escena,
    e: &Edicion,
    ctx: &Contexto,
    anfitrion: &mut dyn Anfitrion,
) -> Result<Hecho, String> {
    let desde = escena.pasos_cerrados();
    let r = hacer::hacer(escena, e, ctx, anfitrion);
    escena.juntar_pasos(desde);
    if r.is_err() && escena.pasos_cerrados() > desde {
        // Algo a medias (no deberia): se deshace para no dejar basura.
        escena.deshacer();
    }
    let mut r = r;
    // **La revision de siempre, gratis** (`lint.rs`): lo que se ve mal en
    // lo que toco esta llamada, para que el modelo lo arregle sin tener que
    // mirar. Buscar no toca nada y no la lleva (listar textos devuelve sin
    // ids, por lo mismo).
    if let Ok(h) = r.as_mut()
        && e.herramienta != SELECCIONAR
        && !h.ids.is_empty()
    {
        anadir_revision(&mut h.texto, escena, &h.ids);
    }
    r
}

/// Pega al «hecho» lo que encuentra `lint::revisar` en lo tocado, si algo.
fn anadir_revision(texto: &mut String, escena: &Escena, tocados: &[u64]) {
    let avisos = crate::lint::revisar(escena, Some(tocados));
    let r = crate::lint::para_el_modelo(&avisos);
    if !r.is_empty() {
        texto.push('\n');
        texto.push_str(&r);
    }
}

// ---------------------------------------------------------------------------
// El resumen del lienzo para el modelo

/// Cuantos elementos como mucho se describen: con mas, el resumen dice
/// cuantos faltan (y `seleccionar_en_lienzo` con una zona los encuentra).
pub const MAX_RESUMEN: usize = 30;

/// El nombre de una figura para el modelo.
pub fn nombre_de(e: &Elemento) -> &'static str {
    match &e.figura {
        Figura::Lapiz { .. } if e.material.es_grafito() => "trazo de grafito",
        Figura::Lapiz { .. } => "trazo a lapiz",
        Figura::Resaltador { .. } => "resaltado",
        Figura::Linea { puntos } if puntos.len() > 3 => "polilinea",
        Figura::Linea { .. } => "recta",
        Figura::Flecha { .. } => "flecha",
        Figura::Rectangulo => "rectangulo",
        Figura::Rombo => "rombo",
        Figura::Elipse if (e.ancho - e.alto).abs() <= 0.02 * e.ancho.max(e.alto) => "circulo",
        Figura::Elipse => "elipse",
        Figura::Arco { .. } => "arco",
        Figura::Texto { .. } => "texto",
        Figura::Imagen { .. } => "imagen",
        Figura::Cota { .. } => "cota",
        Figura::Punto { .. } => "punto",
        Figura::Region { .. } => "relleno",
        Figura::Instrumento(_) => "plano cartesiano",
        Figura::Serie { .. } => "numero",
        Figura::Marco { .. } => "marco",
        Figura::Emoji { .. } => "emoji",
        _ => "figura",
    }
}

/// El nombre del color de la paleta mas parecido.
pub fn nombre_de_color(c: ColorRgba) -> &'static str {
    const PALETA: [(&str, (f32, f32, f32)); 12] = [
        ("negro", (0.12, 0.12, 0.12)),
        ("blanco", (1.0, 1.0, 1.0)),
        ("rojo", (0.88, 0.19, 0.19)),
        ("verde", (0.18, 0.62, 0.27)),
        ("azul", (0.10, 0.44, 0.76)),
        ("amarillo", (0.94, 0.55, 0.0)),
        ("naranja", (0.91, 0.35, 0.05)),
        ("morado", (0.40, 0.25, 0.85)),
        ("rosa", (0.76, 0.15, 0.36)),
        ("gris", (0.53, 0.56, 0.59)),
        ("marron", (0.55, 0.35, 0.17)),
        ("celeste", (0.05, 0.52, 0.60)),
    ];
    // Amarillo de verdad (#ffd43b, el del resaltador) tambien es amarillo.
    if c.r > 0.85 && c.g > 0.7 && c.b < 0.45 {
        return "amarillo";
    }
    PALETA
        .iter()
        .min_by(|a, b| {
            let d = |(r, g, b): (f32, f32, f32)| {
                (r - c.r).powi(2) + (g - c.g).powi(2) + (b - c.b).powi(2)
            };
            d(a.1)
                .partial_cmp(&d(b.1))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|p| p.0)
        .unwrap_or("negro")
}

/// Una linea del resumen: `#12 recta roja (100,40)->(300,40)`.
pub fn describir(e: &Elemento, marco: (f32, f32)) -> String {
    let (mx, my) = marco;
    let r = |v: f32| v.round() as i64;
    let mut s = format!("#{} {} {}", e.id, nombre_de(e), nombre_de_color(e.trazo));
    if let Some(f) = e.relleno.filter(|f| f.a > 0.05) {
        s.push_str(&format!(" relleno {}", nombre_de_color(f)));
    }
    match (&e.figura, e.puntos()) {
        (Figura::Linea { .. } | Figura::Flecha { .. } | Figura::Cota { .. }, Some(p))
            if p.len() >= 2 =>
        {
            let (a, b) = (p[0], p[p.len() - 1]);
            s.push_str(&format!(
                " ({},{})->({},{})",
                r(a.x - mx),
                r(a.y - my),
                r(b.x - mx),
                r(b.y - my)
            ));
        }
        _ => {
            let (x0, y0, x1, y1) = e.caja();
            s.push_str(&format!(
                " [{},{},{},{}]",
                r(x0 - mx),
                r(y0 - my),
                r(x1 - x0),
                r(y1 - y0)
            ));
        }
    }
    if let Figura::Elipse = e.figura {
        let (x0, y0, x1, y1) = e.caja();
        s.push_str(&format!(
            " centro ({},{})",
            r((x0 + x1) / 2.0 - mx),
            r((y0 + y1) / 2.0 - my)
        ));
    }
    if let Figura::Texto { texto, .. } = &e.figura {
        // Entero hasta [`MAX_LETRAS_RESUMEN`]: con 40 letras el modelo no
        // podia traducir ni corregir un parrafo que no veia.
        let n = texto.chars().count();
        let corto: String = texto.chars().take(MAX_LETRAS_RESUMEN).collect();
        s.push_str(&format!(" \"{}\"", corto.replace('\n', "\\n")));
        if n > MAX_LETRAS_RESUMEN {
            s.push_str(&format!(
                " (+{} letras: editar_texto accion listar)",
                n - MAX_LETRAS_RESUMEN
            ));
        }
    }
    if let Some(d) = e.extras.dureza.filter(|_| e.material.es_grafito()) {
        s.push_str(&format!(" {}", d.rotulo()));
    }
    if !e.grupos.is_empty() {
        s.push_str(" (en grupo)");
    }
    s
}

/// Cuantas letras de cada texto lleva el resumen (las de mas, con
/// `editar_texto` accion listar).
pub const MAX_LETRAS_RESUMEN: usize = 200;

/// Cuantas letras como mucho ocupan los textos que no caben entre los
/// [`MAX_RESUMEN`] del resumen.
const PRESUPUESTO_TEXTOS_VIEJOS: usize = 1500;

/// **La linea de un elemento con lo que solo sabe la escena**: de que caja
/// es rotulo un texto, y que rotulo lleva una caja. Sin esto el modelo veia
/// «#7 texto "Inicio"» y «#5 rectangulo» sueltos y no sabia que traducir
/// la caja era cambiar el #7.
pub fn describir_en(escena: &Escena, e: &Elemento, marco: (f32, f32)) -> String {
    let mut s = describir(e, marco);
    if let Some(c) = textos::caja_del_rotulo(escena, e) {
        s.push_str(&format!(" (rotulo de #{c})"));
    } else if let Some(t) = textos::rotulo_de(escena, e) {
        s.push_str(&format!(" (rotulo #{t})"));
    }
    s
}

/// **La lista corta de unos ids**, una linea por elemento.
pub fn listar(escena: &Escena, ids: &[u64], marco: (f32, f32)) -> String {
    ids.iter()
        .filter_map(|id| escena.buscar(*id).filter(|e| !e.borrado))
        .map(|e| describir_en(escena, e, marco))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Las plantillas de serie que hay en la escena (por su grupo de fuera, el de
/// estampar), con su nombre del menu: «Cronograma», «Lista de tareas».
pub fn plantillas_en(escena: &Escena) -> Vec<String> {
    let mut grupos: Vec<(String, Vec<Elemento>)> = Vec::new();
    for e in escena.visibles() {
        if let Some(g) = e.grupos.last() {
            match grupos.iter_mut().find(|(n, _)| n == g) {
                Some((_, v)) => v.push(e.clone()),
                None => grupos.push((g.clone(), vec![e.clone()])),
            }
        }
    }
    let mut v: Vec<String> = grupos
        .iter()
        .filter_map(|(_, piezas)| pixpin_motor2d::figuras_de_serie::cual_es(piezas))
        .map(|c| format!("«{}»", pixpin_motor2d::figuras_de_serie::nombre_de(&c)))
        .collect();
    v.dedup();
    v
}

/// **El resumen compacto del lienzo destino**, para el modelo: cuantos hay,
/// lo elegido y una linea por elemento (los de encima primero, hasta
/// [`MAX_RESUMEN`]). Las guias escondidas y lo borrado no salen.
pub fn resumen(escena: &Escena, ctx: &Contexto) -> String {
    let vivos: Vec<&Elemento> = escena
        .a_la_vista()
        .filter(|e| !e.extras.referencia)
        .collect();
    if vivos.is_empty() {
        return "Lienzo destino: vacio.".into();
    }
    let mut s = format!(
        "Lienzo destino: {} elementos (coordenadas como en dibujar_en_lienzo; usa sus #id para editarlos).",
        vivos.len()
    );
    let elegidos: Vec<String> = ctx
        .elegidos
        .iter()
        .filter(|id| vivos.iter().any(|e| e.id == **id))
        .map(|id| format!("#{id}"))
        .collect();
    if !elegidos.is_empty() {
        s.push_str(&format!(" Elegido por el usuario: {}.", elegidos.join(" ")));
    }
    // **Las plantillas de serie que ya hay** (25-oct-2026): pedir «el
    // cronograma» con uno puesto es casi siempre cambiar ese, no hacer otro.
    let hay = plantillas_en(escena);
    if !hay.is_empty() {
        s.push_str(&format!(
            "\nPlantillas de serie que YA estan en el lienzo: {}. Para agrandarlas usa figuras_guardadas «alargar» (mismo diseño, en su sitio); \
para moverles bloques o anadir filas, reorganizar_hoja. Si el pedido las nombra y no queda claro si es cambiar esta o hacer otra, pregunta.\n\
Cómo pensarla: es una página con bloques con nombre (título, tabla, listas, notas...). Cambiarla es cambiar sus bloques: su serie (alargar, \
que también reduce), sus secciones (personalizar: quitar, añadir, reordenar, ancho completo, lateral, tipo notas o libre; pide antes su guía), \
su sitio (reorganizar_hoja), sus textos (editar_texto). Algo nuevo para ella (una zona de notas, un espacio libre) es una sección suya, \
no un dibujo suelto: así queda dentro de la página y con su diseño. Ejemplo: «hazlo de 10 semanas y abajo una zona gris para anotar» es UNA \
llamada a personalizar del cronograma con semanas_hasta 10 y una sección de tipo notas a todo el ancho («lateral» si la quiere a un lado). \
Si lo pedido no lo permite la plantilla, haz lo más cercano y dilo, o pregunta entre las opciones que sí hay.",
            hay.join(", ")
        ));
    }
    // Los de encima (los ultimos) son los que mas se nombran.
    let desde = vivos.len().saturating_sub(MAX_RESUMEN);
    for e in &vivos[desde..] {
        s.push('\n');
        s.push_str(&describir_en(escena, e, ctx.marco));
    }
    if desde > 0 {
        s.push_str(&format!("\n(+{desde} mas antiguos sin listar)"));
        // **Los textos de los de abajo, aunque no quepan sus lineas**: para
        // «traducelo todo» el modelo necesita cada texto y su #id, y las
        // cajas y colores de lo viejo le sobran. Con mas de lo que cabe, se
        // dice cuantos faltan y como pedirlos.
        let viejos: Vec<&Elemento> = vivos[..desde]
            .iter()
            .copied()
            .filter(|e| matches!(e.figura, Figura::Texto { .. }))
            .collect();
        if !viejos.is_empty() {
            let mut linea = String::from("\nTextos de esos:");
            let mut puestos = 0;
            for e in &viejos {
                let Figura::Texto { texto, .. } = &e.figura else {
                    continue;
                };
                let corto: String = texto
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .replace('\n', "\\n");
                let trozo = format!(
                    " #{} \"{}\"{}",
                    e.id,
                    corto,
                    if texto.chars().count() > 80 {
                        "..."
                    } else {
                        ""
                    }
                );
                if linea.len() + trozo.len() > PRESUPUESTO_TEXTOS_VIEJOS {
                    break;
                }
                linea.push_str(&trozo);
                puestos += 1;
            }
            s.push_str(&linea);
            if puestos < viejos.len() {
                s.push_str(&format!(
                    " (+{} textos mas: editar_texto accion listar)",
                    viejos.len() - puestos
                ));
            }
        }
    }
    s
}

/// **Si el resumen del lienzo viene a cuento en este pedido**: si habla de
/// dibujar o de editar (sus herramientas salen en la seleccion por
/// palabras) o nombra el lienzo. Un pedido de tareas o de notas no lo lleva:
/// gastaria tokens y estorbaria al camino rapido (`rapido.rs`), que lee el
/// pedido tal cual.
pub fn viene_a_cuento(pedido: &str) -> bool {
    let q = crate::acciones::normalizar(pedido);
    if q.contains("lienzo") {
        return true;
    }
    crate::seleccion::para_el_pedido(pedido, false)
        .iter()
        .any(|h| {
            fila(&h.nombre).is_some()
                || [
                    crate::herramientas::DIBUJAR,
                    crate::herramientas::SVG,
                    crate::herramientas::FUNCION,
                ]
                .contains(&h.nombre.as_str())
        })
}

/// **Pega el resumen del lienzo al pedido del usuario** (el ultimo mensaje
/// suyo), entre corchetes y detras: el modelo lo lee con la pregunta, y el
/// prefijo de la conversacion (instrucciones y herramientas) no cambia, asi
/// que la cache de la nube y de llama.cpp sigue valiendo.
pub fn anadir_resumen(historial: &mut [crate::motor::Mensaje], resumen: &str) {
    use crate::motor::{Parte, Rol};
    let Some(m) = historial.iter_mut().rev().find(|m| m.rol == Rol::Usuario) else {
        return;
    };
    let extra = format!("\n\n[{resumen}]");
    match m.partes.iter_mut().find_map(|p| match p {
        Parte::Texto(t) => Some(t),
        _ => None,
    }) {
        Some(t) => t.push_str(&extra),
        None => m.partes.push(Parte::Texto(extra.trim_start().to_string())),
    }
}

/// Cuantos de lo recien dibujado se listan en el «hecho».
const MAX_ANADIDOS: usize = 12;

/// **El «hecho» de lo dibujado** (`dibujar_en_lienzo` y compania): cuantos
/// entraron y sus `#id`, para que la llamada siguiente los pueda editar
/// («dibuja dos rectas... y corta la de abajo»).
pub fn resumen_de_lo_anadido(escena: &Escena, ids: &[u64], ctx: &Contexto) -> String {
    let mut s = format!("hecho: {} elementos anadidos al lienzo", ids.len());
    let lista = listar(escena, &ids[..ids.len().min(MAX_ANADIDOS)], ctx.marco);
    if !lista.is_empty() {
        s.push('\n');
        s.push_str(&lista);
    }
    if ids.len() > MAX_ANADIDOS {
        s.push_str(&format!("\n(+{} mas)", ids.len() - MAX_ANADIDOS));
    }
    if !ids.is_empty() {
        anadir_revision(&mut s, escena, ids);
    }
    s
}

/// **En que orden se hacen varias llamadas de una misma respuesta**: lo que
/// dibuja, antes que lo demas. Un modelo pequeno pide a veces en la misma
/// vuelta «corta la de abajo» y «dibuja dos rectas», en ese orden (medido el
/// 14-oct con el 2B), y al reves no hay nada que cortar. Lo demas va en su
/// orden; los resultados se devuelven en el de la respuesta.
pub fn orden_de_ejecucion(llamadas: &[crate::herramientas::Llamada]) -> Vec<usize> {
    let mut orden: Vec<usize> = (0..llamadas.len()).collect();
    orden.sort_by_key(|&i| !llamadas[i].dibuja());
    orden
}

/// Hace `llamadas` en [`orden_de_ejecucion`] con `hacer` y devuelve los
/// resultados en el orden de la respuesta.
pub fn en_orden<T>(
    llamadas: &[crate::herramientas::Llamada],
    mut hacer: impl FnMut(&crate::herramientas::Llamada) -> T,
) -> Vec<T> {
    let mut salida: Vec<Option<T>> = (0..llamadas.len()).map(|_| None).collect();
    for i in orden_de_ejecucion(llamadas) {
        salida[i] = Some(hacer(&llamadas[i]));
    }
    salida
        .into_iter()
        .map(|r| r.expect("todas hechas"))
        .collect()
}

/// **Lo que el modelo olvido y el pedido dice** (como `colores_del_pedido`
/// con el dibujo): con «rodea el rectangulo con grafito 8B», un 2B llama a
/// `trazar_a_mano` sin «como» ni dureza. Solo rellena lo que falta; lo que
/// el modelo puso, se respeta.
pub fn ajustar_al_pedido(e: &mut Edicion, pedido: &str) {
    let q = crate::acciones::normalizar(pedido);
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    let empieza = |raices: &[&str]| {
        palabras
            .iter()
            .any(|p| raices.iter().any(|r| p.starts_with(r)))
    };
    let falta = |e: &Edicion, campo: &str| !e.args.contains_key(campo);
    if e.herramienta == TRAZAR && e.args.contains_key("sobre") && falta(e, "como") {
        let como = if empieza(&["rode", "encierr", "circul"]) {
            Some("rodear")
        } else if empieza(&["tach"]) {
            Some("tachar")
        } else if empieza(&["repas"]) {
            Some("repasar")
        } else if empieza(&["subray"]) {
            Some("subrayar")
        } else if empieza(&["resalt"]) {
            Some("resaltar")
        } else {
            None
        };
        if let Some(c) = como {
            e.args.insert("como".into(), Value::String(c.into()));
        }
    }
    if e.herramienta == TRAZAR && falta(e, "dureza") {
        let d = pedido
            .split(|c: char| !c.is_alphanumeric())
            .find_map(|p| pixpin_motor2d::tinta::grafito::Dureza::desde_rotulo(&p.to_uppercase()));
        if let Some(d) = d {
            e.args
                .insert("dureza".into(), Value::String(d.rotulo().into()));
        }
    }
    // «Recorta la mitad izquierda» sin decir cual: quedarse con ese lado
    // (como lo entienden Claude y DeepSeek); «quita la de arriba»: quitarla.
    if e.herramienta == IMAGEN
        && e.texto("accion") == Some("recortar")
        && falta(e, "conservar")
        && falta(e, "quitar")
    {
        let lado = palabras.iter().find_map(|p| match *p {
            "izquierda" | "izquierdo" => Some("izquierda"),
            "derecha" | "derecho" => Some("derecha"),
            "arriba" | "superior" => Some("arriba"),
            "abajo" | "inferior" => Some("abajo"),
            _ => None,
        });
        let c = if q.contains("tercio") {
            1.0 / 3.0
        } else if q.contains("cuarto") {
            0.25
        } else {
            0.5
        };
        if let Some(lado) = lado {
            if empieza(&["quit", "elimin", "borr", "sac"]) {
                e.args.insert("quitar".into(), Value::String(lado.into()));
                e.args.insert("cuanto".into(), json!(c));
            } else {
                let caja = match lado {
                    "izquierda" => [0.0, 0.0, c, 1.0],
                    "derecha" => [1.0 - c, 0.0, c, 1.0],
                    "arriba" => [0.0, 0.0, 1.0, c],
                    _ => [0.0, 1.0 - c, 1.0, c],
                };
                e.args.insert("conservar".into(), json!(caja));
            }
        }
    }
    if matches!(e.herramienta, TRAZAR | GEOMETRIA) && falta(e, "color") {
        if let Some(c) = palabras
            .iter()
            .find(|p| crate::herramientas::color(p).is_some())
        {
            e.args
                .insert("color".into(), Value::String((*c).to_string()));
        }
    }
}
