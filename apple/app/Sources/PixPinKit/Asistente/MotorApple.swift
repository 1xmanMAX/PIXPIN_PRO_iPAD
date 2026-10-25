// El motor por omision en Mac: **Foundation Models** (el modelo de Apple en
// el aparato, gratis y sin red), solo en macOS 26 con Apple Intelligence.
//
// La CI compila con Xcode 16.4 (SDK de macOS 15), donde el framework no
// existe: todo lo suyo va dentro de `#if canImport(FoundationModels)` y
// `@available(macOS 26, *)`, y fuera solo quedan dos funciones que dicen
// «no esta» (`motorAppleDisponible`, `preguntarAApple`).
//
// Las herramientas son las de siempre, sin copiar nada a Swift: el JSON
// Schema de `iaCatalogoPara` (el nucleo de Rust, `pixpin_ia::puente`) se
// traduce a `DynamicGenerationSchema`, y cuando el modelo llama una, el
// nombre y los argumentos (`GeneratedContent.jsonString`) van a `iaLlamar`,
// que valida con las mismas reglas que Windows y devuelve los elementos del
// lienzo. Una llamada al puente por herramienta (§4bis.1).

import Foundation
import PixPinCore

#if canImport(FoundationModels)
import FoundationModels

@available(macOS 26, *)
enum MotorApple {
    static var disponible: Bool {
        if case .available = SystemLanguageModel.default.availability { return true }
        return false
    }

    static func preguntar(_ pedido: String, historial: [MensajeIa]) async throws -> RespuestaIa {
        let recolector = Recolector()
        let herramientas = herramientasPara(pedido, recolector: recolector)
        let sesion = LanguageModelSession(tools: herramientas, instructions: iaInstrucciones())
        let respuesta = try await sesion.respond(to: textoConHistorial(pedido, historial: historial))
        return RespuestaIa(texto: respuesta.content,
                           herramientas: recolector.usadas(),
                           lienzo: recolector.lienzo(),
                           pensamiento: "", segundosPensando: nil,
                           motor: "Apple Intelligence", hoja: "", citas: [])
    }

    /// Las herramientas que vienen a cuento, ya como `Tool` de Apple. Una
    /// que no se pueda traducir se salta (el modelo no la vera).
    static func herramientasPara(_ pedido: String, recolector: Recolector) -> [any Tool] {
        guard let datos = iaCatalogoPara(pedido: pedido, conImagen: false).data(using: .utf8),
              let lista = (try? JSONSerialization.jsonObject(with: datos)) as? [[String: Any]]
        else { return [] }
        var salida: [any Tool] = []
        for t in lista {
            guard let f = t["function"] as? [String: Any], let nombre = f["name"] as? String else { continue }
            let descripcion = f["description"] as? String ?? nombre
            let parametros = f["parameters"] as? [String: Any] ?? ["type": "object"]
            let raiz = esquema(parametros, nombre: nombre)
            guard let schema = try? GenerationSchema(root: raiz, dependencies: []) else { continue }
            salida.append(HerramientaPixPin(name: nombre, description: descripcion,
                                            parameters: schema, recolector: recolector))
        }
        return salida
    }

    /// Un JSON Schema (el subconjunto que usa el catalogo) como
    /// `DynamicGenerationSchema`.
    static func esquema(_ j: [String: Any], nombre: String) -> DynamicGenerationSchema {
        let descripcion = j["description"] as? String
        if let opciones = j["enum"] as? [String], !opciones.isEmpty {
            return DynamicGenerationSchema(name: nombre, description: descripcion, anyOf: opciones)
        }
        var tipo = j["type"] as? String
        if tipo == nil, let varios = j["type"] as? [String] {
            tipo = varios.first { $0 != "null" }
        }
        switch tipo {
        case "string":
            return DynamicGenerationSchema(type: String.self)
        case "integer":
            return DynamicGenerationSchema(type: Int.self)
        case "number":
            return DynamicGenerationSchema(type: Double.self)
        case "boolean":
            return DynamicGenerationSchema(type: Bool.self)
        case "array":
            let items = j["items"] as? [String: Any] ?? ["type": "string"]
            return DynamicGenerationSchema(arrayOf: esquema(items, nombre: nombre + "_elemento"))
        default:
            let props = j["properties"] as? [String: Any] ?? [:]
            let requeridos = Set(j["required"] as? [String] ?? [])
            let propiedades = props.keys.sorted().map { clave -> DynamicGenerationSchema.Property in
                let sub = props[clave] as? [String: Any] ?? [:]
                return DynamicGenerationSchema.Property(
                    name: clave,
                    description: sub["description"] as? String,
                    schema: esquema(sub, nombre: nombre + "_" + clave),
                    isOptional: !requeridos.contains(clave)
                )
            }
            return DynamicGenerationSchema(name: nombre, description: descripcion, properties: propiedades)
        }
    }
}

/// Una herramienta de PixPin para Foundation Models: los argumentos llegan
/// como `GeneratedContent` y se pasan tal cual (su JSON) al puente.
@available(macOS 26, *)
struct HerramientaPixPin: Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name: String
    let description: String
    let parameters: GenerationSchema
    let recolector: Recolector

    func call(arguments: GeneratedContent) async throws -> String {
        let r = iaLlamar(nombre: name, argumentos: arguments.jsonString,
                         origenX: 0, origenY: recolector.siguienteY(), semilla: recolector.semilla())
        return recolector.apuntar(nombre: name, respuesta: r)
    }
}
#endif

/// Junta lo que hacen las herramientas en un turno (de varios hilos: el
/// modelo puede llamar dos a la vez).
final class Recolector: @unchecked Sendable {
    private let candado = NSLock()
    private var elementos: [Any] = []
    private var hechas: [HerramientaUsada] = []
    private var abajo: Double = 0
    private var cuenta: UInt64 = 0

    func semilla() -> UInt64 {
        candado.lock(); defer { candado.unlock() }
        cuenta += 1
        return cuenta &* 7919
    }

    /// Cada dibujo, debajo del anterior, para que no se pisen.
    func siguienteY() -> Double {
        candado.lock(); defer { candado.unlock() }
        return elementos.isEmpty ? 0 : abajo + 40
    }

    /// Apunta la respuesta de `iaLlamar` y devuelve lo que se le dice al
    /// modelo.
    func apuntar(nombre: String, respuesta: String) -> String {
        let obj = respuesta.data(using: .utf8)
            .flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any] ?? [:]
        candado.lock(); defer { candado.unlock() }
        guard obj["ok"] as? Bool == true else {
            let motivo = obj["para_el_modelo"] as? String ?? "ERROR: la herramienta no contesto"
            hechas.append(HerramientaUsada(nombre: nombre, ok: false, resumen: motivo))
            return motivo
        }
        let nuevos = obj["elementos"] as? [[String: Any]] ?? []
        for e in nuevos {
            let y = (e["y"] as? Double ?? 0) + (e["height"] as? Double ?? 0)
            abajo = max(abajo, y)
        }
        elementos.append(contentsOf: nuevos as [Any])
        let resumen = nuevos.isEmpty
            ? "\(nombre): esta herramienta aún no se puede hacer en PixPin para Mac"
            : "dibujadas \(nuevos.count) figuras"
        hechas.append(HerramientaUsada(nombre: nombre, ok: !nuevos.isEmpty, resumen: resumen))
        return resumen
    }

    func usadas() -> [HerramientaUsada] {
        candado.lock(); defer { candado.unlock() }
        return hechas
    }

    /// Lo dibujado como un `.excalidraw` entero, o `nil`.
    func lienzo() -> String? {
        candado.lock(); defer { candado.unlock() }
        guard !elementos.isEmpty,
              let datos = try? JSONSerialization.data(withJSONObject: elementos),
              let texto = String(data: datos, encoding: .utf8)
        else { return nil }
        return iaLienzoDe(elementos: texto)
    }
}

/// Lo dicho antes, en pocas lineas, delante del pedido (cada turno de
/// Foundation Models es una sesion nueva con las herramientas que vienen a
/// cuento, y su ventana es pequena).
func textoConHistorial(_ pedido: String, historial: [MensajeIa]) -> String {
    let previos = historial.suffix(6).map { m -> String in
        let t = m.texto.count > 600 ? String(m.texto.prefix(600)) + "…" : m.texto
        return (m.delUsuario ? "Usuario: " : "Asistente: ") + t
    }
    guard !previos.isEmpty else { return pedido }
    return "Conversación hasta ahora:\n" + previos.joined(separator: "\n") + "\n\nPedido nuevo: " + pedido
}

/// Si Foundation Models esta disponible en este Mac ahora mismo.
func motorAppleDisponible() -> Bool {
    #if canImport(FoundationModels)
    if #available(macOS 26, *) { return MotorApple.disponible }
    #endif
    return false
}

/// Un turno con Foundation Models; `nil` si no existe en este sistema.
func preguntarAApple(_ pedido: String, historial: [MensajeIa]) async throws -> RespuestaIa? {
    #if canImport(FoundationModels)
    if #available(macOS 26, *) { return try await MotorApple.preguntar(pedido, historial: historial) }
    #endif
    return nil
}
