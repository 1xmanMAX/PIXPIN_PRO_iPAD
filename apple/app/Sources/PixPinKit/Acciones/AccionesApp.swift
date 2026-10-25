// Lo que piden Atajos, Siri y Spotlight (App Intents, en
// `apple/xcode/App/Intenciones.swift`): capturar una zona, escribir una
// nota en un proyecto, buscar en todo y sincronizar ahora. Vive aqui, y no
// en las intenciones, para usar el mismo almacen y el mismo sincronizador
// que la app (y para poder probarlo con `swift test`).
//
// La app registra lo suyo al arrancar (`registrar`). Si una intencion llega
// antes (el sistema lanza la app para atenderla), lo que no necesita la
// interfaz abre su propio nucleo sobre la carpeta de datos.

import Foundation
import PixPinCore

@MainActor
public final class AccionesApp {
    public static let compartidas = AccionesApp()

    /// Lo que puede salir mal, en palabras para Atajos y Siri.
    public enum Fallo: Error, LocalizedError, Equatable {
        case sinDatos(String)
        case sinApp
        case vacio

        public var errorDescription: String? {
            switch self {
            case let .sinDatos(detalle): return "No se pudo abrir la carpeta de datos de PixPin: \(detalle)"
            case .sinApp: return "PixPin aún se está abriendo; inténtalo otra vez en un momento."
            case .vacio: return "No hay nada que escribir."
            }
        }
    }

    /// Un proyecto, para elegirlo en Atajos.
    public struct Proyecto: Equatable, Sendable {
        public var id: String
        public var nombre: String
    }

    /// Un resultado de buscar, en una linea.
    public struct Encontrado: Equatable, Sendable {
        public var titulo: String
        public var detalle: String
    }

    private weak var almacen: Almacen?
    private weak var sincro: Sincronizador?
    private var capturar: (() -> Void)?
    /// El nucleo propio si la app aun no registro el suyo.
    private var nucleoPropio: Nucleo?

    public init() {}

    /// La app dice con que trabaja (una vez, al arrancar).
    public func registrar(almacen: Almacen?, sincro: Sincronizador?, capturar: @escaping () -> Void) {
        self.almacen = almacen
        self.sincro = sincro
        self.capturar = capturar
    }

    /// El nucleo de la app o, si aun no lo hay, uno sobre la carpeta de datos.
    public func nucleo() throws -> Nucleo {
        if let almacen { return almacen.nucleo }
        if let nucleoPropio { return nucleoPropio }
        do {
            let raiz = carpetaDeDatos()
            try FileManager.default.createDirectory(at: raiz, withIntermediateDirectories: true)
            let n = try Nucleo.abrir(raiz: raiz.path, nombreDelAparato: Host.current().localizedName ?? "Mac")
            nucleoPropio = n
            return n
        } catch {
            throw Fallo.sinDatos("\(error)")
        }
    }

    /// «Capturar zona»: lo mismo que el atajo de teclado.
    public func capturarZona() throws {
        guard let capturar else { throw Fallo.sinApp }
        capturar()
    }

    /// Los proyectos, en el orden de la lista.
    public func proyectos() async throws -> [Proyecto] {
        let nucleo = try self.nucleo()
        return try await Task.detached(priority: .userInitiated) {
            try nucleo.chats().filter { !$0.archivado }.map { Proyecto(id: $0.id, nombre: $0.nombre) }
        }.value
    }

    /// «Nueva nota en proyecto»: una linea al final del chat, como la caja
    /// de escribir. Si el chat se esta viendo, aparece en el acto.
    @discardableResult
    public func escribirNota(_ texto: String, en chat: String) async throws -> MensajeChat {
        let limpio = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !limpio.isEmpty else { throw Fallo.vacio }
        let nucleo = try self.nucleo()
        let m = try await Task.detached(priority: .userInitiated) {
            try nucleo.escribirNota(chat: chat, texto: limpio, respondeA: nil)
        }.value
        if let almacen {
            if almacen.mensajes[chat] != nil {
                almacen.anadir([m], en: chat)
            } else {
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            }
            await almacen.cargarChats()
        }
        return m
    }

    /// «Buscar en PixPin»: los primeros resultados, en una linea cada uno.
    public func buscar(_ consulta: String, cuantos: UInt32 = 10) async throws -> [Encontrado] {
        let nucleo = try self.nucleo()
        let pagina = try await Task.detached(priority: .userInitiated) {
            try nucleo.buscar(consulta: consulta, desde: 0, cuantos: cuantos, busqueda: nil)
        }.value
        return pagina.resultados.map { r in
            let titulo = r.clase == .chat || r.titulo.isEmpty ? r.nombreChat : "\(r.nombreChat): \(r.titulo)"
            return Encontrado(titulo: titulo, detalle: r.fragmento)
        }
    }

    /// «Sincronizar ahora» con los aparatos del grupo que contesten.
    /// Devuelve lo que paso, en una frase.
    public func sincronizarAhora() async throws -> String {
        guard let sincro else { throw Fallo.sinApp }
        await sincro.sincronizarAhora()
        if let error = sincro.error { return error }
        return sincro.ultimoResultado ?? "Sincronización hecha."
    }
}
