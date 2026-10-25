// **Lo que pasa en el buscador, sin ventana** (D8): las pestanas, que fila
// esta elegida, que hace cada tecla, que botones y que menu lleva la fila
// elegida y las letras rapidas. Es `buscar_todo/modelo.rs` de Windows, con
// sus pruebas (`BuscarD8Tests.swift`).
//
// Los resultados los da el nucleo (`Nucleo.buscarEnTodo`, `buscar/todo.rs`)
// ya ordenados por lo que vale cada coincidencia; aqui solo se reparten en
// pestanas y grupos y se decide que hacer con ellos. Lo que no es un
// resultado (apuntar una tarea, capturar, abrir la galeria...) son las
// acciones (`AccionBuscar`), las de las letras rapidas de Windows: t, n, l,
// g, c, u y a.

import Foundation
import PixPinCore

// MARK: - Pestanas y grupos

/// Las pestanas, en su orden (las de Windows y la del timeline, que en el
/// Mac tambien se busca).
public enum PestanaBuscar: Int, CaseIterable, Identifiable, Sendable {
    case todo, archivos, tareas, lecciones, capturas, timeline, acciones

    public var id: Int { rawValue }

    public var rotulo: String {
        switch self {
        case .todo: return "Todo"
        case .archivos: return "Archivos"
        case .tareas: return "Tareas"
        case .lecciones: return "Lecciones"
        case .capturas: return "Capturas"
        case .timeline: return "Timeline"
        case .acciones: return "Acciones"
        }
    }

    /// Las clases de resultado que se le piden al nucleo con esta pestana
    /// (vacio: todas). «Acciones» no pide nada: son de aqui.
    public var clases: [ClaseDeResultado] {
        switch self {
        case .todo: return []
        case .archivos: return [.chat, .mensaje, .fichero]
        case .tareas: return [.tarea]
        case .lecciones: return [.leccion]
        case .capturas: return [.captura]
        case .timeline: return [.momento]
        case .acciones: return []
        }
    }

    /// La de al lado (con vuelta: de Acciones se pasa a Todo).
    public func siguiente(atras: Bool) -> PestanaBuscar {
        let n = PestanaBuscar.allCases.count
        return PestanaBuscar(rawValue: (rawValue + (atras ? n - 1 : 1)) % n) ?? .todo
    }

    /// A que pestana va un resultado.
    public static func de(_ clase: ClaseDeResultado) -> PestanaBuscar {
        switch clase {
        case .chat, .mensaje, .fichero: return .archivos
        case .tarea: return .tareas
        case .leccion: return .lecciones
        case .captura: return .capturas
        case .momento: return .timeline
        }
    }
}

/// Los grupos de «Todo», en el orden de la maqueta de Windows (y el
/// timeline antes de las acciones).
public enum GrupoBuscar: Int, CaseIterable, Sendable {
    case mejor, tareasYLecciones, archivos, capturas, timeline, acciones

    public var rotulo: String {
        switch self {
        case .mejor: return "Mejor resultado"
        case .tareasYLecciones: return "Tareas y lecciones"
        case .archivos: return "Archivos"
        case .capturas: return "Capturas"
        case .timeline: return "Timeline"
        case .acciones: return "Acciones"
        }
    }

    static func de(_ p: PestanaBuscar) -> GrupoBuscar {
        switch p {
        case .tareas, .lecciones: return .tareasYLecciones
        case .archivos: return .archivos
        case .capturas: return .capturas
        case .timeline: return .timeline
        case .todo, .acciones: return .acciones
        }
    }
}

// MARK: - Acciones (lo que no es un resultado)

/// Lo que se hace sin un resultado: las letras rapidas y las funciones de
/// la app por su nombre (`resultado_funcion` del plugin de Windows).
public enum AccionBuscar: Hashable, Sendable {
    /// «t comprar pan»: al Inbox.
    case apuntarTarea(String)
    /// «n idea»: una nota en «Mensajes guardados», en el editor.
    case nuevaNota(String)
    /// «l»: un lienzo en blanco en «Mensajes guardados».
    case nuevoLienzo
    /// «c»: capturar una zona.
    case capturar
    /// «u»: la ultima captura como pin.
    case ultimaCaptura
    /// Una ventana de la app por su id («g», la galeria; «a», el asistente).
    case ventana(id: String, titulo: String)

    public var titulo: String {
        switch self {
        case let .apuntarTarea(t): return t.isEmpty ? "Nueva tarea" : "Apuntar tarea: \(t)"
        case let .nuevaNota(t): return t.isEmpty ? "Nueva nota" : "Nueva nota: \(t)"
        case .nuevoLienzo: return "Nuevo lienzo"
        case .capturar: return "Capturar"
        case .ultimaCaptura: return "Pinear la última captura"
        case let .ventana(_, titulo): return titulo
        }
    }

    public var subtitulo: String {
        switch self {
        case .apuntarTarea: return "Tarea · Inbox · ↩ apuntarla"
        case .nuevaNota: return "Nota · en Mensajes guardados"
        case .nuevoLienzo: return "Lienzo · en blanco, en Mensajes guardados"
        case .capturar: return "Una zona de la pantalla"
        case .ultimaCaptura: return "La más nueva de la galería"
        case .ventana: return "Abrir la ventana"
        }
    }

    public var icono: String {
        switch self {
        case .apuntarTarea: return "plus.circle"
        case .nuevaNota: return "square.and.pencil"
        case .nuevoLienzo: return "scribble.variable"
        case .capturar: return "camera.viewfinder"
        case .ultimaCaptura: return "pin"
        case let .ventana(id, _):
            switch id {
            case IdVentana.galeria: return "photo.on.rectangle"
            case IdVentana.timeline: return "clock"
            case IdVentana.tareas: return "checklist"
            case IdVentana.proyectos: return "bubble.left.and.bubble.right"
            case "lecciones": return "lightbulb"
            case "asistente": return "sparkles"
            default: return "macwindow"
            }
        }
    }

    /// El rotulo del boton azul (`rotulo_principal`).
    public var rotuloPrincipal: String {
        switch self {
        case .apuntarTarea: return "Apuntar"
        case .nuevaNota, .nuevoLienzo: return "Crear"
        case .capturar: return "Capturar"
        case .ultimaCaptura: return "Pinear"
        case .ventana: return "Abrir"
        }
    }

    /// Las ventanas que se abren por su nombre, con lo que las llama (ya
    /// sin tildes ni mayusculas, como `Funcion::alias`).
    static let ventanas: [(AccionBuscar, [String])] = [
        (.ventana(id: IdVentana.galeria, titulo: "Galería de capturas"), ["galeria", "capturas", "gallery"]),
        (.ventana(id: IdVentana.timeline, titulo: "Timeline"), ["timeline", "momentos", "diario"]),
        (.ventana(id: IdVentana.tareas, titulo: "Tareas"), ["tareas", "pendientes", "todo", "tasks"]),
        (.ventana(id: "lecciones", titulo: "Lecciones"), ["lecciones", "repasar", "lessons"]),
        (.ventana(id: IdVentana.proyectos, titulo: "Proyectos"), ["proyectos", "chats", "chat"]),
        (.ventana(id: "asistente", titulo: "Asistente"), ["asistente", "ia", "ai", "assistant"]),
        (.capturar, ["capturar", "captura", "screenshot", "recortar"]),
        (.ultimaCaptura, ["ultima", "last"]),
        (.nuevaNota(""), ["nota", "note"]),
        (.nuevoLienzo, ["lienzo", "canvas", "dibujo", "pizarra"]),
    ]
}

/// Una letra rapida de la caja vacia («Escribe una letra y espacio»).
public struct LetraRapida: Hashable, Sendable {
    public let letra: Character
    public let titulo: String
    public let sub: String
}

public enum LetrasRapidas {
    /// En el orden de la maqueta de Windows.
    public static let todas: [LetraRapida] = [
        LetraRapida(letra: "t", titulo: "Nueva tarea", sub: "al Inbox · «t comprar cemento»"),
        LetraRapida(letra: "n", titulo: "Nueva nota", sub: "en Mensajes guardados · «n idea»"),
        LetraRapida(letra: "l", titulo: "Nuevo lienzo", sub: "en blanco"),
        LetraRapida(letra: "g", titulo: "Galería de capturas", sub: "«g muro» busca dentro"),
        LetraRapida(letra: "c", titulo: "Capturar", sub: "una zona de la pantalla"),
        LetraRapida(letra: "u", titulo: "Pinear la última captura", sub: "la más nueva de la galería"),
        LetraRapida(letra: "a", titulo: "Asistente", sub: "pedirle algo a la IA"),
    ]

    /// La letra y lo de detras, si lo escrito es una letra rapida: la letra
    /// sola o la letra y un blanco («t», «t comprar pan»). Una letra pegada
    /// a otras («tx», «nota») es buscar.
    public static func analizar(_ consulta: String) -> (letra: Character, resto: String)? {
        let s = consulta.drop { $0 == " " }
        guard let l = s.first, todas.contains(where: { $0.letra == Character(l.lowercased()) }) else { return nil }
        let detras = s.dropFirst()
        guard detras.isEmpty || detras.first == " " else { return nil }
        return (Character(l.lowercased()), detras.trimmingCharacters(in: .whitespaces))
    }

    /// Lo que hace cada letra.
    public static func accion(_ letra: Character, _ resto: String) -> AccionBuscar? {
        switch letra {
        case "t": return .apuntarTarea(resto)
        case "n": return .nuevaNota(resto)
        case "l": return .nuevoLienzo
        case "g": return .ventana(id: IdVentana.galeria, titulo: "Galería de capturas")
        case "c": return .capturar
        case "u": return .ultimaCaptura
        case "a": return .ventana(id: "asistente", titulo: "Asistente")
        default: return nil
        }
    }
}

/// Minusculas y sin tildes, como `normalizar` del plugin.
func plegarBuscar(_ s: String) -> String {
    s.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil).lowercased()
}

/// **Las acciones para lo escrito**: con una letra rapida, la suya; si no,
/// las funciones cuyo nombre empieza por lo escrito y, al final, apuntarlo
/// como tarea (el «Apuntar «grie»» de la maqueta de Windows).
public func accionesBuscar(para consulta: String) -> [AccionBuscar] {
    let limpia = consulta.trimmingCharacters(in: .whitespaces)
    if let (l, resto) = LetrasRapidas.analizar(consulta), let a = LetrasRapidas.accion(l, resto) {
        return [a]
    }
    guard !limpia.isEmpty else { return [] }
    let q = plegarBuscar(limpia)
    var v: [AccionBuscar] = AccionBuscar.ventanas
        .filter { _, alias in alias.contains { $0.hasPrefix(q) || (q.count >= 4 && q.hasPrefix($0)) } }
        .map(\.0)
    if !v.contains(.apuntarTarea(limpia)) {
        v.append(.apuntarTarea(limpia))
    }
    return v
}

// MARK: - Filas

/// Una fila de la lista: un resultado del nucleo o una accion.
public enum FilaBuscar: Hashable, Identifiable, Sendable {
    case resultado(ResultadoBusqueda)
    case accion(AccionBuscar)

    public var id: String {
        switch self {
        case let .resultado(r): return r.clave.isEmpty ? "r/\(r.titulo)/\(r.cuando)" : r.clave
        case let .accion(a): return "accion/\(a.titulo)"
        }
    }

    public var resultado: ResultadoBusqueda? {
        if case let .resultado(r) = self { return r }
        return nil
    }

    public var pestana: PestanaBuscar {
        switch self {
        case let .resultado(r): return .de(r.clase)
        case .accion: return .acciones
        }
    }
}

/// Una linea de la lista: la cabecera de un grupo o una fila.
public enum LineaBuscar: Hashable {
    case cabecera(GrupoBuscar)
    case fila(FilaBuscar)
}

/// Los botones de la fila elegida.
public enum BotonBuscar: Hashable, Sendable {
    /// Lo de ↩ (azul, siempre el primero).
    case principal
    /// ⌘↩.
    case verEnChat
    /// ⌘C.
    case copiar
    /// →: el menu de mas acciones.
    case mas
}

/// El rotulo del boton azul de una fila (`rotulo_principal`).
public func rotuloPrincipal(_ f: FilaBuscar) -> String {
    switch f {
    case let .accion(a): return a.rotuloPrincipal
    case let .resultado(r):
        switch r.clase {
        case .captura: return "Pinear"
        case .tarea: return r.hecha ? "Desmarcar" : "Hecha"
        case .chat: return "Entrar"
        case .mensaje: return "Ver en el chat"
        case .fichero: return r.tipo == "VOZ" ? "Reproducir" : "Abrir"
        case .leccion, .momento: return "Abrir"
        }
    }
}

/// «Ver en el chat»: lo que vive en un mensaje de un chat (no el chat
/// mismo: Intro ya lo abre).
public func tieneVerEnChat(_ f: FilaBuscar) -> Bool {
    guard let r = f.resultado else { return false }
    return r.chat != nil && r.posicion != nil && r.clase != .chat && r.clase != .mensaje
}

/// Si ⌘C copia algo de la fila.
public func tieneCopiar(_ f: FilaBuscar) -> Bool {
    guard let r = f.resultado else { return false }
    return r.clase != .chat
}

/// Los botones de la fila, en orden: el azul primero, «…» al final.
public func botonesDe(_ f: FilaBuscar) -> [BotonBuscar] {
    var v: [BotonBuscar] = [.principal]
    if tieneVerEnChat(f) { v.append(.verEnChat) }
    if tieneCopiar(f) { v.append(.copiar) }
    if !opcionesDeMenu(f).isEmpty { v.append(.mas) }
    return v
}

// MARK: - El menu de mas acciones (→)

/// Una opcion del menu de la flecha derecha.
public enum OpcionBuscar: Hashable, Sendable {
    case verEnChat
    case abrir
    case abrirEnEditor
    case pinear
    case copiar
    case copiarRuta
    case conservar
    case borrar
    case mostrarEnFinder
    case vistaRapida
    case abrirCon(app: URL)
    case otraApp
    case marcar(hecha: Bool)
    case abrirVentana(id: String, titulo: String)

    public var titulo: String {
        switch self {
        case .verEnChat: return "Ver en el chat"
        case .abrir: return "Abrir"
        case .abrirEnEditor: return "Abrir en el editor de notas"
        case .pinear: return "Pinear"
        case .copiar: return "Copiar"
        case .copiarRuta: return "Copiar la ruta"
        case .conservar: return "Conservar"
        case .borrar: return "Borrar (se puede deshacer)"
        case .mostrarEnFinder: return "Mostrar en el Finder"
        case .vistaRapida: return "Vista rápida"
        case let .abrirCon(app): return "Abrir con \(FileManager.default.displayName(atPath: app.path))"
        case .otraApp: return "Abrir con otra app…"
        case let .marcar(hecha): return hecha ? "Marcar como hecha" : "Marcar como pendiente"
        case let .abrirVentana(_, titulo): return "Abrir \(titulo)"
        }
    }

    public var icono: String {
        switch self {
        case .verEnChat: return "bubble.left"
        case .abrir, .abrirVentana: return "arrow.up.forward.app"
        case .abrirEnEditor: return "square.and.pencil"
        case .pinear: return "pin"
        case .copiar: return "doc.on.doc"
        case .copiarRuta: return "link"
        case .conservar: return "square.and.arrow.down"
        case .borrar: return "trash"
        case .mostrarEnFinder: return "folder"
        case .vistaRapida: return "eye"
        case .abrirCon, .otraApp: return "arrow.up.right.square"
        case let .marcar(hecha): return hecha ? "checkmark.square" : "square"
        }
    }
}

/// **El menu de una fila** (lo que `menu::menu` del plugin da a cada cosa).
public func opcionesDeMenu(_ f: FilaBuscar) -> [OpcionBuscar] {
    guard let r = f.resultado else { return [] }
    let enChat: [OpcionBuscar] = r.chat != nil && r.posicion != nil ? [.verEnChat] : []
    switch r.clase {
    case .chat:
        return []
    case .mensaje:
        return [.verEnChat] + (r.tipo == "NOTA" ? [.abrirEnEditor] : []) + [.copiar]
    case .fichero:
        guard r.ruta != nil else { return enChat }
        let pin: [OpcionBuscar] = r.tipo == "IMAGEN" ? [.pinear] : []
        return [.abrir] + pin + enChat + [.vistaRapida, .mostrarEnFinder, .copiar, .copiarRuta, .otraApp]
    case .captura:
        return [.pinear, .copiar] + (r.conservada ? [] : [.conservar])
            + [.vistaRapida, .mostrarEnFinder, .otraApp, .abrirVentana(id: IdVentana.galeria, titulo: "la galería"), .borrar]
    case .tarea:
        return [.marcar(hecha: !r.hecha)] + enChat + [.copiar, .abrirVentana(id: IdVentana.tareas, titulo: "Tareas")]
    case .leccion:
        return [.abrirVentana(id: "lecciones", titulo: "Lecciones"), .copiar]
    case .momento:
        return [.abrirVentana(id: IdVentana.timeline, titulo: "el timeline"), .copiar]
            + (r.ruta != nil ? [.pinear] : [])
    }
}

/// El menu abierto, sobre la vista previa.
public struct MenuBuscar: Equatable {
    public var titulo: String
    public var opciones: [OpcionBuscar]
    public var elegida = 0
}

// MARK: - Teclas y ordenes

/// Las teclas que entiende el buscador (las letras van a la caja).
public enum TeclaBuscar: Sendable {
    case arriba, abajo, rePag, avPag, tab, derecha, izquierda, intro, escape, suprimir, c, z
}

/// Las teclas modificadoras que importan.
public struct ModsBuscar: OptionSet, Sendable {
    public let rawValue: Int
    public init(rawValue: Int) { self.rawValue = rawValue }
    public static let mayus = ModsBuscar(rawValue: 1)
    public static let comando = ModsBuscar(rawValue: 2)
    public static let opcion = ModsBuscar(rawValue: 4)
    public static let control = ModsBuscar(rawValue: 8)
}

/// Lo que la ventana tiene que hacer despues de una tecla o un clic.
public enum OrdenBuscar: Equatable {
    /// La tecla no era del buscador: que la atienda la caja de escribir.
    case noEsMia
    /// Era suya y no hay nada mas que hacer.
    case nada
    /// Cambio la pestana con la caja vacia: hay que volver a preguntar.
    case buscar
    /// Se vacia lo escrito.
    case vaciar
    case cerrar
    /// Hacer lo de la fila: ↩ (`recordar`: es la eleccion del usuario).
    case hacer(FilaBuscar, verEnChat: Bool)
    case opcion(OpcionBuscar, FilaBuscar)
    case copiar(FilaBuscar)
    /// Supr sobre una captura: a la papelera de PixPin, con deshacer.
    case borrarCaptura(ResultadoBusqueda)
    case deshacer
    /// ⌥↩: el menu con las apps que saben abrirlo.
    case abrirCon(FilaBuscar)
    /// →: el menu de mas acciones.
    case abrirMenu(FilaBuscar)
}

// MARK: - El estado

public struct EstadoBuscar: Equatable {
    public var consulta = ""
    public var pestana: PestanaBuscar = .todo
    /// Lo que salio (o, con la caja vacia en «Todo», lo abierto hace poco).
    public var resultados: [ResultadoBusqueda] = []
    /// Las acciones de lo escrito (`accionesBuscar`).
    public var acciones: [AccionBuscar] = []
    /// La fila elegida: su posicion en `visibles`.
    public var elegido = 0
    public var menu: MenuBuscar?
    /// Si hay algo que deshacer (una captura borrada).
    public var seDeshace = false

    public init() {}

    /// La caja esta vacia y en «Todo»: se ensenan recientes y letras.
    public var esInicio: Bool {
        consulta.trimmingCharacters(in: .whitespaces).isEmpty && pestana == .todo
    }

    /// Lo escrito es una letra rapida («t pan», «g»).
    public var conLetra: Bool { LetrasRapidas.analizar(consulta) != nil }

    /// Las filas de todo: las de los resultados y las acciones.
    var todas: [FilaBuscar] {
        resultados.map(FilaBuscar.resultado) + acciones.map(FilaBuscar.accion)
    }

    /// Cuantas hay en cada pestana (la de «Todo», todas).
    public func cuenta(_ p: PestanaBuscar) -> Int {
        p == .todo ? todas.count : todas.filter { $0.pestana == p }.count
    }

    /// **Las lineas de la lista.** En «Todo» con algo escrito: el mejor
    /// resultado arriba y el resto por grupos, cada grupo en el orden del
    /// nucleo. Con una letra rapida, su accion primero. En otra pestana,
    /// solo lo suyo y sin cabeceras.
    public func lineas() -> [LineaBuscar] {
        let filas = todas
        if pestana != .todo {
            return filas.filter { $0.pestana == pestana }.map(LineaBuscar.fila)
        }
        if filas.isEmpty { return [] }
        if esInicio {
            // Lo abierto hace poco: una lista, sin grupos.
            return filas.map(LineaBuscar.fila)
        }
        // Con una letra, lo que pide es lo mejor.
        let mejor = conLetra ? (filas.firstIndex { if case .accion = $0 { return true }; return false } ?? 0) : 0
        var v: [LineaBuscar] = [.cabecera(.mejor), .fila(filas[mejor])]
        for g in GrupoBuscar.allCases where g != .mejor {
            let del = filas.indices.filter { $0 != mejor && GrupoBuscar.de(filas[$0].pestana) == g }
            if !del.isEmpty {
                v.append(.cabecera(g))
                v += del.map { .fila(filas[$0]) }
            }
        }
        return v
    }

    /// Las filas que se ven, en el orden en que se ven.
    public func visibles() -> [FilaBuscar] {
        lineas().compactMap { if case let .fila(f) = $0 { return f }; return nil }
    }

    /// La fila elegida.
    public var filaElegida: FilaBuscar? {
        let v = visibles()
        guard !v.isEmpty else { return nil }
        return v[min(elegido, v.count - 1)]
    }

    /// Pone resultados nuevos y vuelve a la primera fila (o se queda donde
    /// estaba, con `mantener`: lo que se refresca tras marcar una tarea).
    public mutating func poner(_ r: [ResultadoBusqueda], mantener: Bool = false) {
        let antes = filaElegida?.id
        resultados = r
        acciones = pestana == .todo || pestana == .acciones ? accionesBuscar(para: consulta) : []
        menu = nil
        if mantener, let antes, let i = visibles().firstIndex(where: { $0.id == antes }) {
            elegido = i
        } else {
            elegido = mantener ? min(elegido, max(visibles().count - 1, 0)) : 0
        }
    }

    @discardableResult
    public mutating func mover(_ paso: Int) -> Bool {
        if var m = menu {
            guard !m.opciones.isEmpty else { return false }
            m.elegida = max(0, min(m.opciones.count - 1, m.elegida + paso))
            menu = m
            return true
        }
        let n = visibles().count
        guard n > 0 else { return false }
        let nuevo = max(0, min(n - 1, elegido + paso))
        guard nuevo != elegido else { return false }
        elegido = nuevo
        return true
    }

    /// Elige otra pestana. Con algo escrito, los resultados ya estan: solo
    /// se filtran; con la caja vacia cada pestana pregunta lo suyo.
    public mutating func ir(a p: PestanaBuscar) -> OrdenBuscar {
        guard p != pestana else { return .nada }
        pestana = p
        elegido = 0
        menu = nil
        if consulta.trimmingCharacters(in: .whitespaces).isEmpty { return .buscar }
        acciones = p == .todo || p == .acciones ? accionesBuscar(para: consulta) : []
        return .nada
    }

    /// Abre el menu de mas acciones de la elegida con `opciones`.
    @discardableResult
    public mutating func abrirMenu(_ opciones: [OpcionBuscar]) -> Bool {
        guard let f = filaElegida, !opciones.isEmpty else { return false }
        menu = MenuBuscar(titulo: titulo(de: f), opciones: opciones)
        return true
    }

    /// Lo que hace un boton de la fila elegida.
    public func boton(_ b: BotonBuscar) -> OrdenBuscar {
        guard let f = filaElegida else { return .nada }
        switch b {
        case .principal: return .hacer(f, verEnChat: false)
        case .verEnChat: return tieneVerEnChat(f) ? .hacer(f, verEnChat: true) : .nada
        case .copiar: return tieneCopiar(f) ? .copiar(f) : .nada
        case .mas: return .abrirMenu(f)
        }
    }

    /// **Una tecla.** `cajaAlFinal`: la rayita esta al final de lo escrito y
    /// no hay nada elegido (si no, → y Supr son de la caja); `textoElegido`:
    /// hay letras elegidas en la caja (⌘C las copia ella).
    public mutating func tecla(_ t: TeclaBuscar, _ m: ModsBuscar = [], cajaAlFinal: Bool = true,
                               textoElegido: Bool = false) -> OrdenBuscar {
        switch t {
        case .escape:
            if menu != nil {
                menu = nil
                return .nada
            }
            return consulta.isEmpty ? .cerrar : .vaciar
        case .arriba: return mover(-1) ? .nada : .nada
        case .abajo: return mover(1) ? .nada : .nada
        case .rePag:
            mover(-6)
            return .nada
        case .avPag:
            mover(6)
            return .nada
        case .izquierda:
            guard menu != nil else { return .noEsMia }
            menu = nil
            return .nada
        case .derecha:
            guard menu == nil, cajaAlFinal, !m.contains(.mayus), !m.contains(.comando), !esInicio,
                  let f = filaElegida else { return .noEsMia }
            return opcionesDeMenu(f).isEmpty ? .noEsMia : .abrirMenu(f)
        case .tab:
            return ir(a: pestana.siguiente(atras: m.contains(.mayus)))
        case .intro:
            if let menu {
                guard menu.opciones.indices.contains(menu.elegida), let f = filaElegida else { return .nada }
                return .opcion(menu.opciones[menu.elegida], f)
            }
            guard let f = filaElegida else { return .nada }
            if m.contains(.opcion) {
                guard f.resultado?.ruta != nil else { return .nada }
                return .abrirCon(f)
            }
            return boton(m.contains(.comando) ? .verEnChat : .principal)
        case .suprimir:
            guard menu == nil, cajaAlFinal || m.contains(.comando), let r = filaElegida?.resultado,
                  r.clase == .captura else { return .noEsMia }
            return .borrarCaptura(r)
        case .c:
            guard m.contains(.comando), !textoElegido, let f = filaElegida, tieneCopiar(f) else { return .noEsMia }
            return .copiar(f)
        case .z:
            guard m.contains(.comando), seDeshace else { return .noEsMia }
            return .deshacer
        }
    }

    func titulo(de f: FilaBuscar) -> String {
        switch f {
        case let .resultado(r): return r.titulo.isEmpty ? r.nombreChat : r.titulo
        case let .accion(a): return a.titulo
        }
    }
}
