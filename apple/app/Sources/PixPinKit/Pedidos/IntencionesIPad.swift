// App Intents del iPad: lo que PixPin ofrece a Siri, Atajos y Spotlight
// (las del Mac, `apple/xcode/App/Intenciones.swift`; «Capturar» abre la hoja
// de capturar del iPad, que iOS no deja capturar otras apps). Viven en
// PixPinKit (no en la app) para usar el mismo almacen y el mismo
// sincronizador que la app:
//
// - «Nueva nota en proyecto», «Buscar en PixPin» y «Sincronizar ahora» van
//   por `AccionesApp` (funcionan sin abrir la app: si aun no hay almacen,
//   `AccionesApp` abre su propio nucleo sobre la carpeta de datos);
// - los pedidos de Windows (tarea, lienzo, grabar, capturar, abrir proyecto,
//   timeline, al lienzo, al movil, preguntar al asistente) arman el mismo
//   pedido que `pixpinmax://` y lo hace `HacerPedidos`, sin preguntar (quien
//   usa Atajos ya esta delante).
//
// **Lo que pone la app** (`Sources/PixPin/PixPinApp.swift`), porque el
// extractor de metadatos de App Intents mira el destino de la app:
//
// - el paquete, para que el sistema vea las intenciones de aqui:
//   `struct PaqueteDeLaApp: AppIntentsPackage { static var includedPackages:
//   [any AppIntentsPackage.Type] { [IntencionesPixPin.self] } }`;
// - las frases de Siri y Spotlight (`AppShortcutsProvider`, una sola por app
//   y en la app): `AtajosDeLaApp`.

import AppIntents
import Foundation
import PixPinCore
import UniformTypeIdentifiers

/// El paquete de intenciones de PixPinKit (lo incluye la app).
public struct IntencionesPixPin: AppIntentsPackage {}

// MARK: - Proyectos

/// Un proyecto (un chat), para elegirlo en Atajos.
public struct ProyectoEntidad: AppEntity, Sendable {
    public static let typeDisplayRepresentation: TypeDisplayRepresentation = "Proyecto"
    public static let defaultQuery = ConsultaProyectos()

    public var id: String
    public var nombre: String

    public init(id: String, nombre: String) {
        self.id = id
        self.nombre = nombre
    }

    public var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(nombre)")
    }
}

public struct ConsultaProyectos: EntityStringQuery, Sendable {
    public init() {}

    public func entities(for identifiers: [String]) async throws -> [ProyectoEntidad] {
        try await todos().filter { identifiers.contains($0.id) }
    }

    public func entities(matching string: String) async throws -> [ProyectoEntidad] {
        let buscado = string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !buscado.isEmpty else { return try await todos() }
        return try await todos().filter {
            $0.nombre.range(of: buscado, options: [.caseInsensitive, .diacriticInsensitive]) != nil
        }
    }

    public func suggestedEntities() async throws -> [ProyectoEntidad] {
        try await todos()
    }

    private func todos() async throws -> [ProyectoEntidad] {
        let lista = try await AccionesApp.compartidas.proyectos()
        return lista.map { ProyectoEntidad(id: $0.id, nombre: $0.nombre) }
    }
}

// MARK: - Las de siempre

public struct NuevaNotaEnProyecto: AppIntent {
    public static let title: LocalizedStringResource = "Nueva nota en proyecto"
    public static let description = IntentDescription("Escribe una nota al final de un proyecto de PixPin.")

    @Parameter(title: "Proyecto")
    public var proyecto: ProyectoEntidad

    @Parameter(title: "Nota")
    public var texto: String

    public init() {}

    public static var parameterSummary: some ParameterSummary {
        Summary("Escribir \(\.$texto) en \(\.$proyecto)")
    }

    @MainActor
    public func perform() async throws -> some IntentResult & ProvidesDialog {
        try await AccionesApp.compartidas.escribirNota(texto, en: proyecto.id)
        let frase = "Nota guardada en «\(proyecto.nombre)»."
        return .result(dialog: "\(frase)")
    }
}

public struct BuscarEnPixPin: AppIntent {
    public static let title: LocalizedStringResource = "Buscar en PixPin"
    public static let description = IntentDescription("Busca en todos los proyectos, notas, tareas y momentos.")

    @Parameter(title: "Buscar")
    public var consulta: String

    public init() {}

    public static var parameterSummary: some ParameterSummary {
        Summary("Buscar \(\.$consulta) en PixPin")
    }

    @MainActor
    public func perform() async throws -> some IntentResult & ReturnsValue<String> & ProvidesDialog {
        let encontrados = try await AccionesApp.compartidas.buscar(consulta)
        let lineas = encontrados.map { $0.detalle.isEmpty ? $0.titulo : "\($0.titulo) — \($0.detalle)" }
        let texto = lineas.joined(separator: "\n")
        let frase = lineas.isEmpty
            ? "No encontré nada con «\(consulta)»."
            : "Encontré \(lineas.count) resultados con «\(consulta)»:\n\(texto)"
        return .result(value: texto, dialog: "\(frase)")
    }
}

public struct SincronizarAhora: AppIntent {
    public static let title: LocalizedStringResource = "Sincronizar ahora"
    public static let description = IntentDescription("Sincroniza con los aparatos de tu grupo que estén en la Wi-Fi.")
    /// En el iPad la sincronizacion vive con la app abierta.
    public static let openAppWhenRun: Bool = true

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult & ProvidesDialog {
        let resumen = try await AccionesApp.compartidas.sincronizarAhora()
        return .result(dialog: "\(resumen)")
    }
}

// MARK: - Los pedidos de Windows

/// Hace un pedido con la app abierta.
@MainActor
private func hacerPedido(_ accion: String, _ campos: [String: ValorPedido]) {
    HacerPedidos.compartidos.hacer(Pedido(accion, campos))
}

public struct NuevaTarea: AppIntent {
    public static let title: LocalizedStringResource = "Nueva tarea"
    public static let description = IntentDescription(
        "Apunta una tarea: sin proyecto, en el Inbox de Mensajes guardados; con proyecto, en su última lista.")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Tarea")
    public var texto: String

    @Parameter(title: "Proyecto")
    public var proyecto: ProyectoEntidad?

    public init() {}

    public static var parameterSummary: some ParameterSummary {
        Summary("Apuntar \(\.$texto) en \(\.$proyecto)")
    }

    @MainActor
    public func perform() async throws -> some IntentResult {
        var campos: [String: ValorPedido] = ["texto": .texto(texto)]
        if let proyecto { campos["proyecto"] = .texto(proyecto.id) }
        hacerPedido("anadir_tarea", campos)
        return .result()
    }
}

public struct NuevoLienzo: AppIntent {
    public static let title: LocalizedStringResource = "Nuevo lienzo"
    public static let description = IntentDescription("Crea un lienzo en un proyecto y lo abre.")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Nombre")
    public var nombre: String?

    @Parameter(title: "Proyecto")
    public var proyecto: ProyectoEntidad?

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        hacerPedido("lienzo_nuevo", ["nombre": .t(nombre), "proyecto": .t(proyecto?.id)])
        return .result()
    }
}

public struct AbrirProyecto: AppIntent {
    public static let title: LocalizedStringResource = "Abrir proyecto"
    public static let description = IntentDescription("Abre el chat de un proyecto en PixPin.")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Proyecto")
    public var proyecto: ProyectoEntidad

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        hacerPedido("abrir", ["que": .objeto(["tipo": .texto("proyecto"), "proyecto": .texto(proyecto.id)])])
        return .result()
    }
}

public struct PreguntarAlAsistente: AppIntent {
    public static let title: LocalizedStringResource = "Preguntar al asistente"
    public static let description = IntentDescription(
        "Le pide algo al asistente de PixPin (Claude, DeepSeek o Apple Intelligence) y dice lo que contesta.")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Pregunta")
    public var texto: String

    public init() {}

    public static var parameterSummary: some ParameterSummary {
        Summary("Preguntar \(\.$texto) al asistente")
    }

    @MainActor
    public func perform() async throws -> some IntentResult {
        hacerPedido("asistente", ["texto": .texto(texto)])
        return .result()
    }
}

public struct GrabarNotaDeVoz: AppIntent {
    public static let title: LocalizedStringResource = "Grabar nota de voz"
    public static let description = IntentDescription(
        "Abre el chat y graba una nota de voz con el micrófono flotante («Guardar» la deja en el chat).")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Proyecto")
    public var proyecto: ProyectoEntidad?

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        hacerPedido("grabar", ["nombre": .texto("Nota de voz"), "proyecto": .t(proyecto?.id)])
        return .result()
    }
}

public struct CapturarConPixPin: AppIntent {
    public static let title: LocalizedStringResource = "Capturar"
    public static let description = IntentDescription(
        "Abre la hoja de capturar de PixPin: cámara, escáner de documentos, Fotos, pegar o la propia app.")
    public static let openAppWhenRun: Bool = true

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        hacerPedido("capturar", [:])
        return .result()
    }
}

public struct ApuntarEnElTimeline: AppIntent {
    public static let title: LocalizedStringResource = "Apuntar en el timeline"
    public static let description = IntentDescription(
        "Apunta un momento en el timeline de PixPin (su primera frase es el título).")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Qué pasó")
    public var texto: String

    public init() {}

    public static var parameterSummary: some ParameterSummary {
        Summary("Apuntar \(\.$texto) en el timeline")
    }

    @MainActor
    public func perform() async throws -> some IntentResult {
        hacerPedido("timeline", ["texto": .texto(texto)])
        return .result()
    }
}

/// Los ficheros de Atajos, copiados a una carpeta temporal de PixPin: lo
/// que llega de otra app puede no tener ruta, o tenerla solo un rato.
@MainActor
private func rutasDe(_ ficheros: [IntentFile], carpeta: String) throws -> [String] {
    let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(carpeta, isDirectory: true)
    try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
    var rutas: [String] = []
    for (i, f) in ficheros.enumerated() {
        let nombre = f.filename.isEmpty ? "fichero-\(i + 1)" : f.filename
        let url = tmp.appendingPathComponent(String(UUID().uuidString.prefix(8)) + "-" + nombre)
        try f.data.write(to: url)
        rutas.append(url.path)
    }
    return rutas
}

public struct MandarAlLienzo: AppIntent {
    public static let title: LocalizedStringResource = "Mandar al lienzo"
    public static let description = IntentDescription(
        "Apila fotos y texto en una columna libre del lienzo de ese nombre (si no existe, lo crea).")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Lienzo")
    public var lienzo: String

    @Parameter(title: "Texto")
    public var texto: String?

    @Parameter(title: "Fotos", supportedContentTypes: [.image])
    public var fotos: [IntentFile]?

    @Parameter(title: "Proyecto")
    public var proyecto: ProyectoEntidad?

    public init() {}

    public static var parameterSummary: some ParameterSummary {
        Summary("Mandar al lienzo \(\.$lienzo)") {
            \.$texto
            \.$fotos
            \.$proyecto
        }
    }

    @MainActor
    public func perform() async throws -> some IntentResult {
        let rutas = try rutasDe(fotos ?? [], carpeta: "pixpin-al-lienzo")
        var campos: [String: ValorPedido] = ["lienzo": .texto(lienzo), "texto": .texto(texto ?? ""),
                                             "imagenes": .textos(rutas)]
        if let proyecto { campos["proyecto"] = .texto(proyecto.id) }
        hacerPedido("al_lienzo", campos)
        return .result()
    }
}

public struct MandarAlMovil: AppIntent {
    public static let title: LocalizedStringResource = "Mandar al móvil"
    public static let description = IntentDescription(
        "Manda ficheros a tus aparatos del grupo: las imágenes a su lienzo, lo demás a su chat.")
    public static let openAppWhenRun: Bool = true

    @Parameter(title: "Ficheros", supportedContentTypes: [.item])
    public var ficheros: [IntentFile]

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        let rutas = try rutasDe(ficheros, carpeta: "pixpin-al-movil")
        hacerPedido("enviar_al_movil", ["aparato": .texto("todos"),
                                        "imagenes": .textos(rutas.filter(Verbos.esImagen)),
                                        "archivos": .textos(rutas.filter { !Verbos.esImagen($0) })])
        return .result()
    }
}

public struct PinearUltimaCaptura: AppIntent {
    public static let title: LocalizedStringResource = "Pinear la última captura"
    public static let description = IntentDescription("Deja flotando como pin la captura más reciente de la galería.")
    public static let openAppWhenRun: Bool = true

    public init() {}

    @MainActor
    public func perform() async throws -> some IntentResult {
        hacerPedido("pinear_ultima", [:])
        return .result()
    }
}

// MARK: - Frases para Siri y Spotlight

// Las frases (`AppShortcutsProvider`) van en la app, no aqui: solo puede
// haber una por app y el extractor de metadatos la busca en el destino de
// la app. Ver `AtajosDeLaApp` en `Sources/PixPin/PixPinApp.swift`.
