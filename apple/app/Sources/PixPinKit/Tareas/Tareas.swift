// Tareas: las listas de tareas de TODOS los chats en una ventana, como la de
// Windows. No hay una segunda copia: cada lista es la mini-app `tareas` de
// un mensaje del chat y marcar reescribe ese mensaje (en el nucleo de Rust,
// con `mini::alternar`), que es lo que viaja al movil al sincronizar.
//
// El formato no tiene fecha de vencimiento: la unica fecha es la de
// creacion (`➕ AAAA-MM-DD`). Por eso lo pendiente se agrupa por lo que
// lleva esperando (hoy, esta semana, este mes, mas, sin fecha), la mas vieja
// arriba como en Windows; lo hecho va aparte y plegado.
//
// Lo que si puede llevar una tarea es **la hora de recordarla** (`⏰`, A4):
// se ve en su fila y se pone o se quita con su menu («Recordarmelo» de
// Windows). Va dentro de su texto, asi que viaja; la hace sonar el vigia
// (`Recordatorios`).
//
// En el iPad, el modelo es el del Mac (sin AppKit ni los pines de tareas,
// que en el iPad no flotan encima de otras apps); la vista es
// `VistaTareas.swift`.

import Foundation
import PixPinCore
import SwiftUI

/// Una tarea con su lista, para ensenarla fuera de ella.
public struct TareaConLista: Identifiable, Hashable {
    public let lista: ListaTareas
    public let tarea: Tarea
    public var id: String { "\(lista.chat)/\(lista.mensaje)/\(tarea.indice)" }
}

/// Las tareas pendientes de un grupo.
public struct GrupoPendiente: Identifiable {
    public let grupo: GrupoDeTareas
    public let tareas: [TareaConLista]
    public var id: GrupoDeTareas { grupo }
}

/// Un grupo de lo pendiente por lo que lleva esperando.
public enum GrupoDeTareas: Int, CaseIterable, Identifiable, Comparable {
    case masDeUnMes, esteMes, estaSemana, hoy, sinFecha

    public var id: Int { rawValue }

    public var titulo: String {
        switch self {
        case .masDeUnMes: "Más de un mes"
        case .esteMes: "Este mes"
        case .estaSemana: "Esta semana"
        case .hoy: "Hoy"
        case .sinFecha: "Sin fecha"
        }
    }

    /// El grupo de una tarea por los dias que lleva.
    public static func de(_ t: Tarea) -> GrupoDeTareas {
        guard let d = t.dias else { return .sinFecha }
        switch d {
        case 0: return .hoy
        case 1...7: return .estaSemana
        case 8...31: return .esteMes
        default: return .masDeUnMes
        }
    }

    public static func < (a: GrupoDeTareas, b: GrupoDeTareas) -> Bool { a.rawValue < b.rawValue }
}

@MainActor
public final class ModeloTareas: ObservableObject {
    @Published public private(set) var listas: [ListaTareas] = []
    @Published public private(set) var error: String?
    @Published public private(set) var cargado = false
    /// Lo ultimo borrado, para «Deshacer» (⌘Z).
    @Published public private(set) var borrada: TareaConLista?
    /// Lo que se busca (texto, `#etiqueta`); vacio: todo.
    @Published public var busqueda = ""

    private let nucleo: Nucleo

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    /// El nucleo, para leer fuera del hilo principal (las asignaturas).
    var nucleoParaLeer: Nucleo { nucleo }

    /// Lo pendiente de todas las listas por grupos, la mas vieja arriba (y
    /// a igual fecha, el orden de su lista), como `orden_de_pendiente` de
    /// Windows.
    public var pendientesPorGrupo: [GrupoPendiente] {
        var grupos: [GrupoDeTareas: [TareaConLista]] = [:]
        for l in listas {
            for t in l.tareas where !t.hecha && encaja(t) {
                grupos[GrupoDeTareas.de(t), default: []].append(TareaConLista(lista: l, tarea: t))
            }
        }
        return GrupoDeTareas.allCases.compactMap { g in
            guard let v = grupos[g], !v.isEmpty else { return nil }
            let ordenadas = v.sorted { a, b in
                let fa = a.tarea.creada ?? "", fb = b.tarea.creada ?? ""
                if fa != fb { return fa < fb }
                return a.tarea.indice < b.tarea.indice
            }
            return GrupoPendiente(grupo: g, tareas: ordenadas)
        }
    }

    /// Lo hecho, en el orden de sus listas.
    public var hechas: [TareaConLista] {
        listas.flatMap { l in l.tareas.filter(\.hecha).filter(encaja).map { TareaConLista(lista: l, tarea: $0) } }
    }

    /// Si una tarea encaja con lo buscado: todas las palabras, sin tildes
    /// ni mayusculas, en su texto o en su lista.
    public func encaja(_ t: Tarea) -> Bool {
        ModeloTareas.encaja(t.crudo, busqueda)
    }

    nonisolated static func encaja(_ texto: String, _ busqueda: String) -> Bool {
        let opciones: String.CompareOptions = [.caseInsensitive, .diacriticInsensitive]
        return busqueda.split(separator: " ").allSatisfy { texto.range(of: $0, options: opciones) != nil }
    }

    /// Las listas que se ven (las de lo buscado), con sus tareas filtradas.
    public var listasFiltradas: [ListaTareas] {
        guard !busqueda.trimmingCharacters(in: .whitespaces).isEmpty else { return listas }
        return listas.compactMap { l in
            var x = l
            x.tareas = l.tareas.filter(encaja)
            return x.tareas.isEmpty ? nil : x
        }
    }

    /// Hace un cambio en el nucleo y relee; si la lista cambio fuera, lo
    /// dice.
    private func cambiar(chat: String, _ hacer: @escaping @Sendable (Nucleo) throws -> Bool) async -> Bool {
        let nucleo = self.nucleo
        var aviso: String?
        var hecho = false
        do {
            hecho = try await Task.detached(priority: .userInitiated) { try hacer(nucleo) }.value
            if hecho {
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: chat)
            } else {
                aviso = "La lista cambió: se ha vuelto a leer"
            }
        } catch {
            aviso = "\(error)"
        }
        await cargar()
        if let aviso { error = aviso }
        return hecho
    }

    /// **Corrige el texto** de una tarea (F2), entendido como al apuntarla.
    public func editar(_ t: TareaConLista, a nuevo: String) async {
        let limpio = nuevo.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !limpio.isEmpty, limpio != t.tarea.texto else { return }
        let (chat, lista, i, crudo, d) = (t.lista.chat, t.lista.mensaje, t.tarea.indice, t.tarea.crudo, desfaseLocalMs())
        _ = await cambiar(chat: chat) {
            try $0.editarTarea(chat: chat, lista: lista, indice: i, crudo: crudo, nuevo: limpio, desfaseMs: d)
        }
    }

    /// **Borra** una tarea; se puede deshacer hasta el siguiente borrado.
    public func borrar(_ t: TareaConLista) async {
        let (chat, lista, i, crudo) = (t.lista.chat, t.lista.mensaje, t.tarea.indice, t.tarea.crudo)
        if await cambiar(chat: chat, { try $0.borrarTarea(chat: chat, lista: lista, indice: i, crudo: crudo) }) {
            borrada = t
        }
    }

    /// Vuelve a poner lo ultimo borrado en su sitio.
    public func deshacer() async {
        guard let t = borrada else { return }
        borrada = nil
        let (chat, lista, i, crudo, hecha) = (t.lista.chat, t.lista.mensaje, t.tarea.indice, t.tarea.crudo, t.tarea.hecha)
        _ = await cambiar(chat: chat) {
            try $0.devolverTarea(chat: chat, lista: lista, indice: i, crudo: crudo, hecha: hecha)
            return true
        }
    }

    /// **Pasa una tarea a otra lista** («Mover a…», o soltarla en otra
    /// columna del Kanban).
    public func mover(_ t: TareaConLista, a destino: ListaTareas) async {
        guard destino.mensaje != t.lista.mensaje || destino.chat != t.lista.chat else { return }
        let (chat, lista, i, crudo) = (t.lista.chat, t.lista.mensaje, t.tarea.indice, t.tarea.crudo)
        let (aChat, aLista) = (destino.chat, destino.mensaje)
        if await cambiar(chat: chat, {
            try $0.moverTarea(chat: chat, lista: lista, indice: i, crudo: crudo, aChat: aChat, aLista: aLista)
        }) {
            NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: aChat)
        }
    }

    /// Las asignaturas del horario (donde tambien se puede apuntar).
    public var asignaturas: [String] { nucleo.nombresDeAsignaturas(desfaseMs: desfaseLocalMs()) }

    /// La tarea de un id (`chat/lista/indice`), para soltarla en el Kanban.
    public func tarea(id: String) -> TareaConLista? {
        for l in listas {
            for t in l.tareas where TareaConLista(lista: l, tarea: t).id == id {
                return TareaConLista(lista: l, tarea: t)
            }
        }
        return nil
    }

    public func cargar() async {
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        do {
            listas = try await Task.detached(priority: .userInitiated) {
                try nucleo.tareas(desfaseMs: desfase)
            }.value
            error = nil
        } catch {
            self.error = "\(error)"
        }
        cargado = true
    }

    /// Pone una tarea como hecha o pendiente en su mensaje. Si la lista
    /// cambio fuera (el numero ya es otra tarea), no se toca y se relee.
    public func marcar(_ t: TareaConLista, hecha: Bool) async {
        let nucleo = self.nucleo
        let (chat, mensaje, indice, crudo) = (t.lista.chat, t.lista.mensaje, t.tarea.indice, t.tarea.crudo)
        // El aviso se pone DESPUES de releer: `cargar` limpia el error al
        // leer bien, y el usuario tiene que saber que su clic no se aplico.
        var aviso: String?
        do {
            let hecho = try await Task.detached(priority: .userInitiated) {
                try nucleo.marcarTarea(chat: chat, mensaje: mensaje, indice: indice, crudo: crudo, hecha: hecha)
            }.value
            if hecho {
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                // Las tareas, al instante a los otros aparatos.
                NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: chat)
            } else {
                aviso = "La lista cambió: se ha vuelto a leer"
            }
        } catch {
            aviso = "\(error)"
        }
        await cargar()
        if let aviso { error = aviso }
    }

    /// **Pone o quita la hora de recordar** una tarea (`nil` la quita).
    /// Como al marcar, si la lista cambio fuera no se toca y se relee.
    public func recordar(_ t: TareaConLista, en cuando: Date?) async {
        let nucleo = self.nucleo
        let (chat, mensaje, indice, crudo) = (t.lista.chat, t.lista.mensaje, t.tarea.indice, t.tarea.crudo)
        let local = cuando.map(milisegundosLocales)
        var aviso: String?
        do {
            let hecho = try await Task.detached(priority: .userInitiated) {
                try nucleo.ponerHoraTarea(chat: chat, lista: mensaje, indice: indice, crudo: crudo,
                                          cuandoLocalMs: local)
            }.value
            if hecho {
                // Al vigia (que relee al oirlo) y a los otros aparatos.
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: chat)
            } else {
                aviso = "La lista cambió: se ha vuelto a leer"
            }
        } catch {
            aviso = "\(error)"
        }
        await cargar()
        if let aviso { error = aviso }
    }

    /// Apunta una tarea en el Inbox de «Mensajes guardados», con sus
    /// imagenes (pegadas o soltadas en la caja) si las hay.
    public func apuntar(_ texto: String, imagenes: [URL] = [], asignatura: String = "") async {
        let limpio = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !limpio.isEmpty || !imagenes.isEmpty else { return }
        let rutas = imagenes.map(\.path)
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        do {
            let lista = try await Task.detached(priority: .userInitiated) {
                asignatura.isEmpty
                    ? try nucleo.apuntarTareaConImagenes(texto: limpio, imagenes: rutas, desfaseMs: desfase)
                    : try nucleo.apuntarEnAsignatura(asignatura: asignatura, texto: limpio, imagenes: rutas,
                                                     desfaseMs: desfase)
            }.value
            error = nil
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: lista.chat)
        } catch {
            self.error = "\(error)"
        }
        await cargar()
    }
}
