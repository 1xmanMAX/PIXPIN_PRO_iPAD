// **Las herramientas del lienzo y como se agrupan en la barra**, copiadas de
// Windows: el catalogo y los grupos de `pixpin-ui/src/caja_herramientas.rs`
// (`BOTONES_EDITOR`, `GrupoBarra`, `BARRA_AGRUPADA`), la regla del clic y la
// memoria de la ultima usada de `apps/pixpin/src/dibujo/grupos.rs`, y las
// letras de `dibujo/teclas.rs`.
//
// Aqui solo hay datos y reglas, sin vistas: se prueban sin pantalla. La
// barra que los pinta es `BarraHerramientasLienzo.swift`.

import Foundation

/// **Los grupos de la barra** (`GrupoBarra` de Windows, que copia los
/// `GRUPOS_DE_FABRICA` del movil): lo que hace lo mismo, junto. La barra
/// ensena UN boton por grupo —la herramienta del grupo que este puesta, o la
/// ultima que se uso— y las demas salen en un desplegable al pulsarlo.
public enum GrupoHerramientaLienzo: String, CaseIterable {
    /// Elegir de otras formas: el lazo, la bolita y la zona.
    case elegir
    /// Lo que pinta a mano ademas del lapiz: el grafito, el resaltador y el
    /// bote.
    case trazar
    case formas
    case flechas
    /// Lo que arregla una raya ya trazada.
    case arreglar
    /// Lo que nombra cosas del dibujo: el texto, el numerito y el punto.
    case nombrar
    /// Lo que tapa o senala: pixelar, la lupa, el foco y el laser.
    case tapar
    case medir
    /// El marco y la imagen, como en el movil.
    case marco
    /// Sacar el dibujo: compartir e imprimir.
    case sacar

    public var titulo: String {
        switch self {
        case .elegir: return "Elegir de otra forma"
        case .trazar: return "Trazar a mano"
        case .formas: return "Formas"
        case .flechas: return "Flechas y líneas"
        case .arreglar: return "Arreglar lo trazado"
        case .nombrar: return "Texto y nombres"
        case .tapar: return "Tapar y señalar"
        case .medir: return "Medir"
        case .marco: return "Marco e imagen"
        case .sacar: return "Compartir e imprimir"
        }
    }

    /// Lo que hay dentro, en el orden del desplegable (el de Windows). La
    /// primera es la cara del grupo mientras no se haya usado otra.
    public var miembros: [HerramientaLienzo] {
        HerramientaLienzo.todas.filter { $0.grupo == self }
    }
}

/// Una herramienta de la barra: nombre del motor, titulo, icono y letra.
public struct HerramientaLienzo: Identifiable, Equatable {
    public let id: String
    public let titulo: String
    let icono: IconoLienzo
    /// La tecla que la elige (sin modificadores, fuera de un texto). Las de
    /// Windows y, donde Windows no tiene, la de Excalidraw si no choca.
    public let tecla: Character?
    /// Su grupo; `nil` es que va suelta (la mano, el lapiz y la goma: las de
    /// uso constante).
    public let grupo: GrupoHerramientaLienzo?
    /// Es una accion de un grupo (meter una imagen, compartir, imprimir) y
    /// no una herramienta: no queda puesta, se hace y ya (`BotonCaja::Imagen`
    /// o `Compartir` dentro de un `GrupoBarra` de Windows).
    public let accion: Bool

    init(_ id: String, _ titulo: String, _ icono: IconoLienzo, tecla: Character? = nil,
         grupo: GrupoHerramientaLienzo? = nil, accion: Bool = false) {
        self.id = id
        self.titulo = titulo
        self.icono = icono
        self.tecla = tecla
        self.grupo = grupo
        self.accion = accion
    }

    public static func == (a: HerramientaLienzo, b: HerramientaLienzo) -> Bool { a.id == b.id }

    /// El catalogo, en el orden de los desplegables de Windows
    /// (`GrupoBarra::miembros`), con las acciones de los grupos. Las letras
    /// M L R T B A E G P F Q C Z J son las de Windows
    /// (`tecla_a_herramienta`); S O D X W N, las de Excalidraw que alli no
    /// se usan.
    public static let todas: [HerramientaLienzo] = [
        .init("seleccion", "Seleccionar", .selection, tecla: "m"),
        .init("lazo", "Lazo", .lasso, tecla: "s", grupo: .elegir),
        // La bolita, junto al lazo: las dos eligen sin marquesina.
        .init("bolita", "Bolita", .selectAll, grupo: .elegir),
        .init("zona", "Zona (copia recortada)", .crop, tecla: "z", grupo: .elegir),
        .init("lapiz", "Lápiz", .freedraw, tecla: "l"),
        .init("grafito", "Grafito", .grafito, tecla: "p", grupo: .trazar),
        .init("resaltador", "Resaltador", .penMode, tecla: "r", grupo: .trazar),
        .init("relleno", "Bote de relleno", .bucketFill, grupo: .trazar),
        .init("rectangulo", "Rectángulo", .rectangle, grupo: .formas),
        .init("elipse", "Elipse", .ellipse, tecla: "o", grupo: .formas),
        .init("rombo", "Rombo", .diamond, tecla: "d", grupo: .formas),
        .init("arco", "Arco", .angle, grupo: .formas),
        .init("flecha", "Flecha", .arrow, grupo: .flechas),
        .init("flecha_libre", "Flecha a pulso", .roundArrow, grupo: .flechas),
        .init("linea", "Línea", .line, grupo: .flechas),
        .init("flecha_codos", "Flecha de codos", .elbowArrow, grupo: .flechas),
        .init("recortar", "Recortar", .cut, tecla: "x", grupo: .arreglar),
        .init("extender", "Extender", .arrowRight, tecla: "w", grupo: .arreglar),
        // Soldar vertices: la chincheta de Excalidraw, que es un clavo.
        .init("nudo", "Soldar vértices", .pin, grupo: .arreglar),
        .init("copiar_estilo", "Copiar estilo", .eyeDropper, grupo: .arreglar),
        .init("texto", "Texto", .text, tecla: "t", grupo: .nombrar),
        .init("serie", "Numerito", .abacus, grupo: .nombrar),
        .init("punto", "Punto", .dots, tecla: "n", grupo: .nombrar),
        // El ojo cerrado: lo que un mosaico hace es dejar de ensenar.
        .init("mosaico", "Pixelar (mosaico)", .eyeClosed, grupo: .tapar),
        .init("lupa", "Lupa", .search, tecla: "q", grupo: .tapar),
        .init("foco", "Foco", .presentation, tecla: "f", grupo: .tapar),
        .init("laser", "Puntero láser", .laserPointer, tecla: "j", grupo: .tapar),
        .init("cota", "Cota", .lineEditor, tecla: "a", grupo: .medir),
        .init("escalar", "Calibrar escala", .resize, tecla: "e", grupo: .medir),
        .init("escala_grafica", "Escala gráfica", .grid, tecla: "g", grupo: .medir),
        // «C» de marCo: la «F» de «frame» ya la usa el foco.
        .init("marco", "Marco", .marco, tecla: "c", grupo: .marco),
        .init("imagen", "Imagen", .image, grupo: .marco, accion: true),
        .init("borrador", "Goma", .eraser, tecla: "b"),
        .init("compartir", "Compartir…", .shareIos, grupo: .sacar, accion: true),
        .init("imprimir", "Imprimir… (⌘P)", .impresora, grupo: .sacar, accion: true),
    ]

    /// Solo las herramientas (lo que se puede dejar puesto), sin las
    /// acciones de los grupos.
    public static let herramientas: [HerramientaLienzo] = todas.filter { !$0.accion }

    public static func con(id: String) -> HerramientaLienzo? {
        todas.first { $0.id == id }
    }

    /// La que elige la letra `c` (da igual mayuscula o minuscula).
    public static func deTecla(_ c: Character) -> HerramientaLienzo? {
        let c = Character(c.lowercased())
        return herramientas.first { $0.tecla == c }
    }

    /// Las que trazan a mano: solo con ellas hay gesto de pararse.
    static let aMano: Set<String> = ["lapiz", "resaltador", "grafito"]

    /// Las que no dibujan nada nuevo: con ellas puestas y nada elegido, el
    /// panel de estilo sobra (Excalidraw lo esconde igual).
    static let sinEstilo: Set<String> = ["seleccion", "lazo", "bolita", "zona", "borrador", "escalar",
                                         "recortar", "extender", "nudo", "copiar_estilo", "laser"]
}

/// Lo que se ve en la barra: herramientas sueltas, grupos y acciones.
public enum BotonBarraLienzo: Hashable {
    case herramienta(String)
    case grupo(GrupoHerramientaLienzo)
    /// **Las figuras** (el `Figuras` de Windows): la biblioteca y las de
    /// fabrica —la tabla en blanco, la grafica de una funcion—.
    case figuras
    case deshacer
    case rehacer
    /// Cerrar el lienzo (el `Salir` de Windows; se guarda solo).
    case salir

    /// **La barra agrupada** (`BARRA_AGRUPADA` de Windows): elegir, lo que
    /// pinta, las formas, las flechas, lo que arregla, lo que nombra, lo que
    /// tapa, lo que mide, las figuras, el marco y la goma; y al final lo que
    /// saca el dibujo y las acciones. El `Atravesar` de Windows es solo del
    /// anotador de pantalla viva: aqui no sale.
    public static let barra: [BotonBarraLienzo] = [
        .herramienta("seleccion"), .grupo(.elegir),
        .herramienta("lapiz"), .grupo(.trazar), .grupo(.formas), .grupo(.flechas), .grupo(.arreglar),
        .grupo(.nombrar),
        .grupo(.tapar), .grupo(.medir), .figuras, .grupo(.marco), .herramienta("borrador"),
        .grupo(.sacar), .deshacer, .rehacer, .salir,
    ]

    /// **La seccion** (`caja_herramientas::grupo` de Windows): entre dos
    /// secciones va un separador. Elegir; lo que dibuja; lo que tapa, mide,
    /// enmarca o borra; y lo que saca el dibujo con las acciones.
    public var seccion: Int {
        switch self {
        case .herramienta("seleccion"), .grupo(.elegir): return 0
        case .herramienta("lapiz"), .grupo(.trazar), .grupo(.formas), .grupo(.flechas), .grupo(.arreglar),
             .grupo(.nombrar): return 1
        case .grupo(.tapar), .grupo(.medir), .grupo(.marco), .figuras, .herramienta("borrador"): return 2
        case .grupo(.sacar), .deshacer, .rehacer, .salir: return 3
        case .herramienta: return 1
        }
    }

    /// **La barra del anotador de la captura**: la misma, sin las figuras
    /// (su biblioteca es del lienzo) ni compartir e imprimir (lo anotado sale
    /// con las acciones de la captura). Cerrar descarta la captura.
    public static let barraDeCaptura: [BotonBarraLienzo] = barra.filter { $0 != .figuras && $0 != .grupo(.sacar) }

    /// **Encima de un documento** (`Anfitrion::Lector` de Windows): sin las
    /// figuras, sin sacar el dibujo y sin salir (se sale con el ojo de la
    /// barra del pin).
    public static let barraDelLector: [BotonBarraLienzo] = barraDeCaptura.filter { $0 != .salir }

    /// Si entre este y el siguiente de `botones` va un separador.
    static func corte(tras i: Int, en botones: [BotonBarraLienzo] = barra) -> Bool {
        i + 1 < botones.count && botones[i].seccion != botones[i + 1].seccion
    }

    /// **Lo que mide a lo ancho una fila** con esos botones
    /// (`ancho_de_fila` de Windows): los botones, sus huecos, el relleno de
    /// la isla y cada separador con sus margenes.
    static func anchoDeFila(_ botones: ArraySlice<BotonBarraLienzo>) -> CGFloat {
        let n = CGFloat(botones.count)
        let b = Array(botones)
        let cortes = CGFloat(zip(b, b.dropFirst()).filter { $0.seccion != $1.seccion }.count)
        return 2 * MedidasBarraLienzo.relleno + n * MedidasBarraLienzo.lado
            + max(0, n - 1) * MedidasBarraLienzo.hueco
            + cortes * (MedidasBarraLienzo.separador + MedidasBarraLienzo.margenSeparador + MedidasBarraLienzo.hueco)
    }

    /// **Donde partir la barra en dos filas** si en una no cabe en `ancho`
    /// (`corte_para` de Windows): por el separador de seccion que deja las
    /// dos filas mas parejas. `barra.count` si cabe en una.
    static func cortePara(ancho: CGFloat, en barra: [BotonBarraLienzo] = barra) -> Int {
        let n = barra.count
        if anchoDeFila(barra[...]) <= ancho { return n }
        return (1..<n)
            .filter { barra[$0 - 1].seccion != barra[$0].seccion }
            .min { a, b in
                max(anchoDeFila(barra[..<a]), anchoDeFila(barra[a...]))
                    < max(anchoDeFila(barra[..<b]), anchoDeFila(barra[b...]))
            } ?? n
    }
}

/// **La memoria y el clic de los grupos** (`dibujo/grupos.rs` de Windows).
/// La memoria es una para toda la aplicacion y no una por ventana: es una
/// costumbre de quien dibuja («el rombo es mi forma»). Se pierde al cerrar.
@MainActor
public enum GruposBarraLienzo {
    private static var recordadas: [GrupoHerramientaLienzo: String] = [:]

    /// Se uso una accion desde el desplegable (imprimir, la imagen): las
    /// herramientas se recuerdan solas al quedar puestas (`cara`); las
    /// acciones no quedan puestas y si deben quedar de cara (`grupos::usado`).
    public static func usado(_ id: String) {
        if let h = HerramientaLienzo.con(id: id), let g = h.grupo { recordadas[g] = id }
    }

    /// Para las pruebas: olvidarlo todo.
    static func olvidar() { recordadas = [:] }

    /// **La cara del grupo** con la herramienta `activa` puesta: la puesta si
    /// es del grupo (y pasa a ser la recordada); si no, la ultima usada; si
    /// no, la primera.
    public static func cara(_ g: GrupoHerramientaLienzo, activa: String) -> HerramientaLienzo? {
        let miembros = g.miembros
        if let puesta = miembros.first(where: { $0.id == activa }) {
            recordadas[g] = puesta.id
            return puesta
        }
        if let r = recordadas[g], let h = miembros.first(where: { $0.id == r }) { return h }
        return miembros.first
    }

    /// Lo que hace un clic en el boton de un grupo.
    public struct Clic: Equatable {
        /// La herramienta que hay que coger, si hay que cambiar.
        public let elegir: String?
        /// Que grupo queda desplegado despues.
        public let desplegado: GrupoHerramientaLienzo?
    }

    /// **El clic en un grupo**: coge su cara si no estaba puesta Y ensena las
    /// hermanas en el mismo clic. Pulsar el grupo ya abierto lo cierra, y uno
    /// de una sola herramienta no se abre. La cara de un grupo de acciones
    /// (compartir, imprimir) no se dispara: imprimir por sorpresa no.
    public static func pulsar(_ g: GrupoHerramientaLienzo, activa: String,
                              desplegado: GrupoHerramientaLienzo?) -> Clic {
        var elegir: String?
        if let c = cara(g, activa: activa), c.id != activa, !c.accion { elegir = c.id }
        let abierto: GrupoHerramientaLienzo? = desplegado == g || g.miembros.count <= 1 ? nil : g
        return Clic(elegir: elegir, desplegado: abierto)
    }
}
