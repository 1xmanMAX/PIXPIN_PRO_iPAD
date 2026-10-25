// App Intents del iPad: lo que PixPin ofrece a Siri, Atajos y Spotlight
// (las del Mac, `apple/xcode/App/Intenciones.swift`, sin «Capturar zona»:
// iOS no deja capturar otras apps). Viven en PixPinKit (no en la app) para
// usar el mismo almacen y el mismo sincronizador que la app:
//
// - «Nueva nota en proyecto», «Buscar en PixPin» y «Sincronizar ahora» van
//   por `AccionesApp` (funcionan sin abrir la app: si aun no hay almacen,
//   `AccionesApp` abre su propio nucleo sobre la carpeta de datos);
// - los pedidos de Windows (tarea, lienzo, abrir proyecto, preguntar al
//   asistente) arman el mismo pedido que `pixpinmax://` y lo hace
//   `HacerPedidos`, sin preguntar (quien usa Atajos ya esta delante).
//
// Para que el sistema las vea, la app declara el paquete:
// `struct PaqueteDeLaApp: AppIntentsPackage { static var includedPackages:
// [any AppIntentsPackage.Type] { [IntencionesPixPin.self] } }`.

import AppIntents
import Foundation
import PixPinCore

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

// MARK: - Frases para Siri y Spotlight

public struct AtajosPixPin: AppShortcutsProvider {
    public static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: NuevaNotaEnProyecto(),
            phrases: [
                "Nueva nota en \(.applicationName)",
                "Apuntar una nota en \(.applicationName)",
            ],
            shortTitle: "Nueva nota",
            systemImageName: "square.and.pencil")
        AppShortcut(
            intent: BuscarEnPixPin(),
            phrases: [
                "Buscar en \(.applicationName)",
            ],
            shortTitle: "Buscar",
            systemImageName: "magnifyingglass")
        AppShortcut(
            intent: SincronizarAhora(),
            phrases: [
                "Sincronizar \(.applicationName)",
                "Sincronizar \(.applicationName) ahora",
            ],
            shortTitle: "Sincronizar ahora",
            systemImageName: "arrow.triangle.2.circlepath")
        AppShortcut(
            intent: NuevaTarea(),
            phrases: [
                "Nueva tarea en \(.applicationName)",
                "Apuntar una tarea en \(.applicationName)",
            ],
            shortTitle: "Nueva tarea",
            systemImageName: "checklist")
        AppShortcut(
            intent: NuevoLienzo(),
            phrases: ["Nuevo lienzo en \(.applicationName)"],
            shortTitle: "Nuevo lienzo",
            systemImageName: "scribble.variable")
        AppShortcut(
            intent: AbrirProyecto(),
            phrases: ["Abrir un proyecto de \(.applicationName)"],
            shortTitle: "Abrir proyecto",
            systemImageName: "folder")
        AppShortcut(
            intent: PreguntarAlAsistente(),
            phrases: ["Preguntar al asistente de \(.applicationName)"],
            shortTitle: "Preguntar al asistente",
            systemImageName: "sparkles")
    }
}
