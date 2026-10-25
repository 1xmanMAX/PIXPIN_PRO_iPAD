// **Un visor en otra ventana** (Stage Manager, Split View): lo que en
// Windows y el Mac es abrir cada documento en su propia ventana
// (`grupos_ventanas.rs`, `Planos/VentanasDePlano.swift`).
//
// La app declara un segundo `WindowGroup(id: VentanasDeVisor.grupo, for:
// PeticionVentana.self)` (`Sources/PixPin/PixPinApp.swift`) que pinta
// `VentanaDeVisor`. Lo que se abre viaja como `PeticionVentana` (Codable:
// iOS la guarda para devolver la ventana tal cual al volver a abrir la app),
// con las rutas relativas a la carpeta de datos (la ruta del contenedor de
// la app cambia al actualizarla) y los mensajes por su id.
//
// Cada ventana lleva su propia `Navegacion` en el entorno: su boton de
// cerrar (`BarraVisorIPad`) cierra ESA ventana y no el visor de la
// principal.

import Foundation
import PixPinCore
import SwiftUI
import UIKit

/// Lo que se abre en otra ventana.
public struct PeticionVentana: Codable, Hashable, Sendable {
    public enum Clase: String, Codable, Sendable {
        case lienzo, documento, nota, hoja, plano, modelo3d, medio, vistaRapida
    }

    public var clase: Clase
    /// El fichero: relativo a la carpeta de datos si esta dentro, absoluto
    /// si no.
    public var ruta: String?
    public var titulo: String?
    public var chat: String?
    /// El id del mensaje (se busca otra vez al abrir).
    public var mensaje: String?

    public init(clase: Clase, ruta: String? = nil, titulo: String? = nil, chat: String? = nil, mensaje: String? = nil) {
        self.clase = clase
        self.ruta = ruta
        self.titulo = titulo
        self.chat = chat
        self.mensaje = mensaje
    }

    /// La peticion de un visor (la foto de un lienzo nuevo va por su ruta).
    public init(_ visor: Visor, datos: URL = carpetaDeDatos()) {
        func r(_ u: URL?) -> String? { u.map { PeticionVentana.guardar($0, datos: datos) } }
        switch visor {
        case let .lienzo(chat, m, foto): self.init(clase: .lienzo, ruta: r(foto), chat: chat, mensaje: m?.id)
        case let .documento(url, titulo, chat, m): self.init(clase: .documento, ruta: r(url), titulo: titulo, chat: chat, mensaje: m?.id)
        case let .nota(chat, m): self.init(clase: .nota, chat: chat, mensaje: m.id)
        case let .hoja(chat, m): self.init(clase: .hoja, chat: chat, mensaje: m.id)
        case let .plano(url, titulo): self.init(clase: .plano, ruta: r(url), titulo: titulo)
        case let .modelo3d(url, titulo): self.init(clase: .modelo3d, ruta: r(url), titulo: titulo)
        case let .medio(url, titulo): self.init(clase: .medio, ruta: r(url), titulo: titulo)
        case let .vistaRapida(url): self.init(clase: .vistaRapida, ruta: r(url))
        }
    }

    /// Una ruta para guardar: relativa (`proyectos/…`) si esta en la
    /// carpeta de datos.
    public static func guardar(_ url: URL, datos: URL) -> String {
        let base = datos.standardizedFileURL.path
        let p = url.standardizedFileURL.path
        if p.hasPrefix(base + "/") { return String(p.dropFirst(base.count + 1)) }
        return p
    }

    /// Y al reves.
    public static func url(de ruta: String, datos: URL) -> URL {
        ruta.hasPrefix("/") ? URL(fileURLWithPath: ruta) : datos.appendingPathComponent(ruta)
    }

    /// El nombre de la ventana (en el selector de ventanas de iPadOS).
    public var nombre: String {
        if let titulo, !titulo.isEmpty { return titulo }
        if let ruta { return (ruta as NSString).lastPathComponent }
        switch clase {
        case .lienzo: return "Lienzo"
        case .nota: return "Nota"
        case .hoja: return "Hoja de cálculo"
        default: return "PixPin"
        }
    }
}

/// Abrir ventanas desde fuera de una vista (la barra del visor, los menus).
@MainActor
public enum VentanasDeVisor {
    /// El id del `WindowGroup` de los visores.
    public static let grupo = "visor"

    /// El `openWindow` de SwiftUI, que deja `conVentanasDeVisor()` (sin el,
    /// la app no tiene el grupo de ventanas y todo se abre en la misma).
    static var abrirVentana: ((PeticionVentana) -> Void)?

    /// Si se puede abrir otra ventana (iPadOS con varias escenas y el grupo
    /// declarado).
    public static var sePuede: Bool {
        abrirVentana != nil && UIApplication.shared.supportsMultipleScenes
    }

    /// Abre `visor` en una ventana nueva; si no se puede, en la misma.
    public static func abrir(_ visor: Visor) {
        guard sePuede, let abrirVentana else {
            Navegacion.compartida.abrir(visor)
            return
        }
        abrirVentana(PeticionVentana(visor))
    }

    /// Pasa el visor de la ventana principal a una ventana suya: se cierra
    /// aqui (y se guarda al desaparecer) y se abre alli un momento despues,
    /// para que la ventana nueva lea lo ya guardado.
    public static func sacarAOtraVentana(_ nav: Navegacion) {
        guard sePuede, let visor = nav.visor, let abrirVentana else { return }
        let peticion = PeticionVentana(visor)
        nav.cerrarVisor()
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(600))
            abrirVentana(peticion)
        }
    }
}

public extension View {
    /// Deja a mano el `openWindow` de esta escena para `VentanasDeVisor`
    /// (se pone en la raiz de cada ventana).
    func conVentanasDeVisor() -> some View {
        modifier(RegistrarVentanas())
    }
}

private struct RegistrarVentanas: ViewModifier {
    @Environment(\.openWindow) private var openWindow

    func body(content: Content) -> some View {
        content.onAppear {
            let abrir = openWindow
            VentanasDeVisor.abrirVentana = { abrir(id: VentanasDeVisor.grupo, value: $0) }
        }
    }
}

/// El boton «Abrir en otra ventana» de la barra de los visores (solo en la
/// ventana principal y si iPadOS deja varias ventanas).
public struct BotonOtraVentana: View {
    @EnvironmentObject private var nav: Navegacion

    public init() {}

    public var body: some View {
        if nav === Navegacion.compartida, nav.visor != nil, VentanasDeVisor.sePuede {
            Button { VentanasDeVisor.sacarAOtraVentana(nav) } label: {
                Image(systemName: "macwindow.on.rectangle")
            }
            .help("Abrir en otra ventana")
        }
    }
}

/// Una ventana con un visor (el `WindowGroup` de los visores).
public struct VentanaDeVisor: View {
    let peticion: PeticionVentana?
    @ObservedObject var entorno: EntornoApp
    /// La navegacion de ESTA ventana: cerrar el visor cierra la ventana.
    @StateObject private var nav = Navegacion()
    @State private var abierto = false
    @State private var noSeEncontro = false
    @Environment(\.dismissWindow) private var cerrarVentana

    public init(peticion: PeticionVentana?, entorno: EntornoApp) {
        self.peticion = peticion
        self.entorno = entorno
    }

    public var body: some View {
        ConAlmacen(entorno.almacen, error: entorno.error) { almacen in
            Group {
                if let visor = nav.visor {
                    // Sin la capa de pines: esa vive en la ventana principal.
                    VistaDelVisor(visor: visor, almacen: almacen)
                } else if noSeEncontro {
                    ContentUnavailableView("Ya no está", systemImage: "questionmark.folder",
                                           description: Text("Lo que se abrió en esta ventana se borró o se movió."))
                        .overlay(alignment: .topTrailing) {
                            Button("Cerrar") { cerrarVentana() }.padding()
                        }
                } else {
                    ProgressView()
                }
            }
            .task(id: peticion) { await abrir(almacen) }
        }
        .environmentObject(entorno)
        .environmentObject(nav)
        .conVentanasDeVisor()
        .navigationTitle(peticion?.nombre ?? "PixPin")
        .onChange(of: nav.visor == nil) { _, cerrado in
            // El boton de cerrar del visor: fuera la ventana.
            if cerrado, abierto { cerrarVentana() }
        }
    }

    private func abrir(_ almacen: Almacen) async {
        guard let peticion else {
            noSeEncontro = true
            return
        }
        guard let visor = await VentanaDeVisor.visor(de: peticion, almacen: almacen) else {
            noSeEncontro = true
            return
        }
        nav.visor = visor
        abierto = true
    }

    /// Lo que pide la peticion, con sus mensajes buscados otra vez. `nil` si
    /// ya no esta.
    static func visor(de p: PeticionVentana, almacen: Almacen) async -> Visor? {
        let datos = carpetaDeDatos()
        let url = p.ruta.map { PeticionVentana.url(de: $0, datos: datos) }
        let titulo = p.titulo ?? url?.lastPathComponent ?? ""
        var mensaje: MensajeChat?
        if let chat = p.chat, let id = p.mensaje {
            mensaje = await buscar(id, en: chat, almacen: almacen)
            if mensaje == nil { return nil }
        }
        if let url, !FileManager.default.fileExists(atPath: url.path) { return nil }
        switch p.clase {
        case .lienzo:
            guard let chat = p.chat else { return nil }
            return .lienzo(chat: chat, mensaje: mensaje, foto: url)
        case .documento:
            guard let url else { return nil }
            return .documento(url: url, titulo: titulo, chat: p.chat, mensaje: mensaje)
        case .nota:
            guard let chat = p.chat, let mensaje else { return nil }
            return .nota(chat: chat, mensaje: mensaje)
        case .hoja:
            guard let chat = p.chat, let mensaje else { return nil }
            return .hoja(chat: chat, mensaje: mensaje)
        case .plano:
            return url.map { .plano(url: $0, titulo: titulo) }
        case .modelo3d:
            return url.map { .modelo3d(url: $0, titulo: titulo) }
        case .medio:
            return url.map { .medio(url: $0, titulo: titulo) }
        case .vistaRapida:
            return url.map { .vistaRapida(url: $0) }
        }
    }

    /// Un mensaje por su id: en lo cargado o, si no, en el nucleo (su
    /// posicion y una pagina de uno).
    static func buscar(_ id: String, en chat: String, almacen: Almacen) async -> MensajeChat? {
        if let m = almacen.mensajes[chat]?.first(where: { $0.id == id }) { return m }
        let nucleo = almacen.nucleo
        return await Task.detached(priority: .userInitiated) { () -> MensajeChat? in
            guard let pos = try? nucleo.posicionDeMensaje(chat: chat, mensaje: id) else { return nil }
            return try? nucleo.mensajes(chat: chat, desde: pos, cuantos: 1).mensajes.first { $0.id == id }
        }.value
    }
}
