// La navegacion de la app del iPad: que seccion se ve en la barra lateral,
// que chat esta abierto y que visor cubre la pantalla.
//
// En Windows y el Mac cada cosa es una ventana aparte (Proyectos, Galeria,
// Timeline...) y cada documento un pin sin bordes. En el iPad:
//
// - las ventanas de la barra comun son **secciones** de la barra lateral
//   (`Seccion`), y con Stage Manager cada una se puede abrir en su ventana;
// - los documentos (lienzo, PDF, nota, hoja, plano, 3D...) se abren en un
//   **visor a pantalla completa** (`Visor`) con la barra flotante comun, el
//   equivalente del pin sin bordes de Windows;
// - lo que en Windows flota encima de todo (pines de captura, mini-apps)
//   flota dentro de la app, en la capa de pines (`Pines/`).

import Foundation
import PixPinCore
import SwiftUI

/// Las secciones de la barra lateral: las mismas ventanas que la bandeja de
/// Windows abre (Proyectos, Galeria, Timeline, Tareas, Lecciones, Buscar,
/// Asistente) mas Aparatos (sincronizar) y Ajustes.
public enum Seccion: String, CaseIterable, Identifiable, Sendable {
    case proyectos, galeria, timeline, tareas, lecciones, buscar, asistente, aparatos, ajustes

    public var id: String { rawValue }

    public var titulo: String {
        switch self {
        case .proyectos: "Proyectos"
        case .galeria: "Galería"
        case .timeline: "Timeline"
        case .tareas: "Tareas"
        case .lecciones: "Lecciones"
        case .buscar: "Buscar"
        case .asistente: "Asistente"
        case .aparatos: "Aparatos"
        case .ajustes: "Ajustes"
        }
    }

    public var icono: String {
        switch self {
        case .proyectos: "bubble.left.and.bubble.right"
        case .galeria: "photo.on.rectangle"
        case .timeline: "clock.arrow.circlepath"
        case .tareas: "checklist"
        case .lecciones: "graduationcap"
        case .buscar: "magnifyingglass"
        case .asistente: "sparkles"
        case .aparatos: "laptopcomputer.and.ipad"
        case .ajustes: "gearshape"
        }
    }
}

/// Lo que se abre a pantalla completa encima de todo (el «pin» del iPad).
public enum Visor: Identifiable {
    /// Un lienzo del chat (`DIBUJO` con su `referencia`), o uno nuevo si no
    /// hay mensaje. Con `foto`, el lienzo nace con esa foto para anotarla
    /// (una foto se abre en su lienzo, como en Windows y el movil).
    case lienzo(chat: String, mensaje: MensajeChat?, foto: URL?)
    /// Un documento: PDF, Word, PowerPoint, EPUB, HTML, Markdown, Excel, CSV,
    /// texto... `chat` y `mensaje` si viene de un chat (para su tinta).
    case documento(url: URL, titulo: String, chat: String?, mensaje: MensajeChat?)
    /// Una nota del chat (`NOTA`) en su editor de Markdown.
    case nota(chat: String, mensaje: MensajeChat)
    /// Una hoja de calculo del chat (`TABLA`, o `MINIAPP` tabla).
    case hoja(chat: String, mensaje: MensajeChat)
    /// Un plano CAD (DWG, DXF...).
    case plano(url: URL, titulo: String)
    /// Un modelo BIM o 3D (IFC, Revit, LandXML, nubes de puntos...).
    case modelo3d(url: URL, titulo: String)
    /// Un video o un audio.
    case medio(url: URL, titulo: String)
    /// Lo que no tiene visor propio: Vista Rapida (`QLPreviewController`).
    case vistaRapida(url: URL)

    public var id: String {
        switch self {
        case let .lienzo(chat, m, foto): "lienzo:\(chat):\(m?.id ?? "nuevo"):\(foto?.path ?? "")"
        case let .documento(url, _, _, _): "doc:\(url.path)"
        case let .nota(chat, m): "nota:\(chat):\(m.id)"
        case let .hoja(chat, m): "hoja:\(chat):\(m.id)"
        case let .plano(url, _): "plano:\(url.path)"
        case let .modelo3d(url, _): "3d:\(url.path)"
        case let .medio(url, _): "medio:\(url.path)"
        case let .vistaRapida(url): "ql:\(url.path)"
        }
    }
}

/// La navegacion de toda la app (una por proceso; cada ventana de Stage
/// Manager lleva la suya en `EstadoVentana` si hiciera falta).
@MainActor
public final class Navegacion: ObservableObject {
    public static let compartida = Navegacion()

    /// La seccion elegida en la barra lateral.
    @Published public var seccion: Seccion? = .proyectos
    /// El chat abierto en Proyectos (su `id`).
    @Published public var chatAbierto: String?
    /// Saltar a un mensaje (desde Buscar, Timeline, Tareas...): el chat lo
    /// abre centrado en el y lo marca.
    @Published public var saltoAMensaje: (chat: String, mensaje: String)?
    /// El visor a pantalla completa (o nada).
    @Published public var visor: Visor?

    public init() {}

    public func abrir(_ visor: Visor) { self.visor = visor }

    /// Abre `visor` en una ventana suya (Stage Manager), o aqui si iPadOS no
    /// deja varias ventanas (`Sistema/VentanasDeVisor.swift`).
    public func abrirEnOtraVentana(_ visor: Visor) { VentanasDeVisor.abrir(visor) }

    public func cerrarVisor() { visor = nil }

    /// Abrir un chat (y, si se da, ir a un mensaje): pasa a Proyectos.
    public func irAlChat(_ chat: String, mensaje: String? = nil) {
        seccion = .proyectos
        chatAbierto = chat
        if let mensaje { saltoAMensaje = (chat, mensaje) }
    }

    /// Abre un fichero con su visor, segun su extension (como el doble clic
    /// de Windows): planos y 3D a Metal, documentos al lector, video y audio
    /// al reproductor, lo demas a Vista Rapida.
    public func abrirFichero(_ url: URL, titulo: String? = nil, chat: String? = nil, mensaje: MensajeChat? = nil) {
        let nombre = titulo ?? url.lastPathComponent
        switch TipoDeFichero(url) {
        case .plano: abrir(.plano(url: url, titulo: nombre))
        case .modelo3d: abrir(.modelo3d(url: url, titulo: nombre))
        case .documento: abrir(.documento(url: url, titulo: nombre, chat: chat, mensaje: mensaje))
        case .medio: abrir(.medio(url: url, titulo: nombre))
        case .imagen:
            if let chat { abrir(.lienzo(chat: chat, mensaje: mensaje, foto: url)) } else { abrir(.vistaRapida(url: url)) }
        case .otro: abrir(.vistaRapida(url: url))
        }
    }
}

/// Que visor le toca a un fichero por su extension.
public enum TipoDeFichero: Equatable, Sendable {
    case plano, modelo3d, documento, medio, imagen, otro

    public init(_ url: URL) {
        self.init(extension: url.pathExtension)
    }

    public init(extension e: String) {
        switch e.lowercased() {
        case "dwg", "dxf", "dwf", "dgn": self = .plano
        case "ifc", "ifczip", "rvt", "rfa", "landxml", "las", "laz", "e57", "ply", "obj", "stl", "gltf", "glb": self = .modelo3d
        case "pdf", "docx", "doc", "pptx", "ppt", "epub", "html", "htm", "md", "markdown", "txt",
             "xlsx", "xls", "csv", "tsv", "odt", "ods", "odp", "rtf": self = .documento
        case "mp4", "mov", "m4v", "mp3", "m4a", "wav", "aac", "ogg", "opus", "webm", "3gp": self = .medio
        case "png", "jpg", "jpeg", "heic", "heif", "gif", "webp", "bmp", "tif", "tiff": self = .imagen
        default: self = .otro
        }
    }
}
