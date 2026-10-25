// El asistente del iPad: un chat con el motor que dicen los ajustes
// `[asistente]` (los mismos que Windows y el Mac, Ajustes → Asistente):
//
// - `motor = claude | deepseek`: la nube (Claude Haiku 5.5 o DeepSeek V4.1
//   Flash, con la clave del Llavero, `LlaveroDelSistema`) por el HTTPS de
//   URLSession (`TransporteHttps`); si falla y `respaldo_local`, Apple
//   Intelligence si la hay.
// - local (vacio): en el iPad no hay `llama-server` ni modelos bajados:
//   **Foundation Models** (iPadOS 26 con Apple Intelligence,
//   `MotorApple.swift`); si no esta, se dice y la ventana ofrece poner la
//   clave de la nube (`HojaClaves`).
//
// Lo que no es Foundation Models va entero por el nucleo de Rust
// (`iaTurno`): el bucle del agente de Windows con pensar, el plan, la hoja
// de trabajo, el camino rapido y todas las herramientas y acciones (las de
// la app son pedidos del protocolo, `ManosDelAsistente`). En vivo: el texto
// y lo pensado llegan por tandas a un `OyenteIa` y se puede parar
// (`CorteIa`).
//
// Igual que el Mac (es su `ModeloAsistente` sin AppKit): cada turno va en un
// hilo de GCD (puede esperar un «Sí»/«No» del usuario sin quitarle hilos al
// resto de la app).

import AVFoundation
import Foundation
import ImageIO
import PixPinCore
import UIKit
import UniformTypeIdentifiers

@MainActor
public final class ModeloAsistente: ObservableObject {
    /// Que motor contesta.
    public enum Motor: Equatable, Sendable {
        case apple
        case llamaLocal(UInt16)
        /// Un modelo bajado con su `llama-server` (no en el iPad; por si el
        /// nucleo lo dice).
        case descargado(String)
        /// Claude o DeepSeek (su nombre); `conClave` si hay clave guardada.
        case nube(String, conClave: Bool)
        case ninguno

        public var nombre: String {
            switch self {
            case .apple: return "Apple Intelligence (en este iPad)"
            case let .llamaLocal(p): return "llama-server (127.0.0.1:\(p))"
            case let .descargado(n): return n
            case let .nube(n, conClave): return conClave ? n : n + " · falta la clave"
            case .ninguno: return "Sin motor"
            }
        }

        var esNube: Bool {
            if case .nube = self { return true }
            return false
        }

        /// Si hace falta poner una clave para que conteste.
        public var faltaClave: Bool {
            switch self {
            case .ninguno: return true
            case let .nube(_, conClave): return !conClave
            default: return false
            }
        }
    }

    /// Una herramienta que se uso (guardable).
    public struct Hecha: Codable, Equatable, Sendable {
        public var nombre: String
        public var ok: Bool
        public var resumen: String
    }

    /// Lo citado de los documentos del usuario (abre el documento).
    public struct Cita: Codable, Equatable, Sendable {
        public var numero: Int
        public var titulo: String
        /// El pedido `abrir` del protocolo (JSON).
        public var pedido: String
    }

    /// Un fichero adjunto a la conversacion.
    public struct Adjunto: Codable, Hashable, Identifiable, Sendable {
        public var id = UUID()
        public var nombre: String
        public var ruta: String

        public var esImagen: Bool {
            ["png", "jpg", "jpeg", "gif", "webp", "heic", "tif", "tiff", "bmp"]
                .contains(URL(fileURLWithPath: ruta).pathExtension.lowercased())
        }
    }

    /// Una burbuja del chat.
    public struct Burbuja: Identifiable, Equatable, Codable, Sendable {
        public var id = UUID()
        public var delUsuario: Bool
        public var texto: String
        public var herramientas: [Hecha] = []
        /// Lo dibujado, como `.excalidraw` (se puede guardar y abrir).
        public var lienzo: String?
        /// Un aviso de la app (no lo dijo el modelo): no vuelve al modelo.
        public var esAviso = false
        /// Lo que penso (plegable) y cuanto tardo.
        public var pensamiento = ""
        public var segundosPensando: Double?
        /// Quien contesto.
        public var motor = ""
        /// Lo que adjunto el usuario con este mensaje.
        public var adjuntos: [Adjunto] = []
        /// Lo citado de sus documentos (`buscar_en_mis_cosas`).
        public var citas: [Cita] = []
        /// «Dibujando…» mientras el modelo pide una herramienta (no se guarda).
        public var llamando: String?

        public init(delUsuario: Bool, texto: String, herramientas: [Hecha] = [], lienzo: String? = nil,
                    esAviso: Bool = false) {
            self.delUsuario = delUsuario
            self.texto = texto
            self.herramientas = herramientas
            self.lienzo = lienzo
            self.esAviso = esAviso
        }

        enum CodingKeys: String, CodingKey {
            case id, delUsuario, texto, herramientas, lienzo, esAviso, pensamiento, segundosPensando, motor, adjuntos, citas
        }

        public init(from d: Decoder) throws {
            let c = try d.container(keyedBy: CodingKeys.self)
            id = try c.decode(UUID.self, forKey: .id)
            delUsuario = try c.decode(Bool.self, forKey: .delUsuario)
            texto = try c.decode(String.self, forKey: .texto)
            herramientas = try c.decodeIfPresent([Hecha].self, forKey: .herramientas) ?? []
            lienzo = try c.decodeIfPresent(String.self, forKey: .lienzo)
            esAviso = try c.decodeIfPresent(Bool.self, forKey: .esAviso) ?? false
            pensamiento = try c.decodeIfPresent(String.self, forKey: .pensamiento) ?? ""
            segundosPensando = try c.decodeIfPresent(Double.self, forKey: .segundosPensando)
            motor = try c.decodeIfPresent(String.self, forKey: .motor) ?? ""
            adjuntos = try c.decodeIfPresent([Adjunto].self, forKey: .adjuntos) ?? []
            citas = try c.decodeIfPresent([Cita].self, forKey: .citas) ?? []
        }
    }

    /// Una pregunta de «¿seguro?» que espera en la burbuja.
    public struct Pregunta: Identifiable {
        public let id = UUID()
        public let texto: String
        let responder: @MainActor (Bool) -> Void
    }

    public static let textoSinMotor = """
    No hay motor para el asistente en este iPad. Opciones:
    • Claude o DeepSeek con tu clave (se guarda cifrada en el Llavero): pulsa «Poner la clave».
    • iPadOS 26 con Apple Intelligence activado (Ajustes → Apple Intelligence y Siri).
    """

    @Published public private(set) var burbujas: [Burbuja] = []
    @Published public private(set) var ocupado = false
    @Published public private(set) var motor: Motor?
    /// La conversacion abierta y las guardadas (la lista de la izquierda).
    @Published public private(set) var conversacion = Conversacion()
    @Published public private(set) var guardadas: [Conversacion] = []
    /// Lo adjuntado para el mensaje que se esta escribiendo.
    @Published public var pendientes: [Adjunto] = []
    @Published public private(set) var pregunta: Pregunta?

    public let puertoLlama: UInt16
    private let usarApple: Bool
    private weak var almacen: Almacen?
    private let historial: HistorialAsistente?
    /// Lo que se manda a Foundation Models como historial, como mucho.
    static let historialMaximo = 8

    public init(almacen: Almacen?, puertoLlama: UInt16 = 8080, usarApple: Bool = true,
                historial: HistorialAsistente? = nil) {
        self.almacen = almacen
        self.puertoLlama = puertoLlama
        self.usarApple = usarApple
        self.historial = historial
        ModeloAsistente.instalarLaNube()
        if let historial {
            guardadas = historial.todas()
        }
    }

    /// El Llavero y el HTTPS en el nucleo (una vez por proceso; las demas
    /// no hacen nada): sin ellos no se guarda ninguna clave ni se llama a la
    /// nube.
    public static func instalarLaNube() {
        LlaveroDelSistema.instalar()
        TransporteHttps.instalar()
    }

    // MARK: Ajustes y motor

    /// Los ajustes `[asistente]` de la carpeta de datos (sin ella, los de
    /// fabrica). En el iPad no hay modelos bajados: la carpeta va vacia.
    func ajustes() -> AjustesIa {
        let a = almacen.flatMap { try? $0.nucleo.ajustes() }
        return AjustesIa(motor: a?.asistenteMotor ?? "",
                         modelo: a?.asistenteModelo ?? "",
                         pensar: a?.asistentePensar,
                         respaldoLocal: a?.asistenteRespaldoLocal ?? true,
                         mejorVista: a?.asistenteMejorVista ?? false,
                         documentosALaNube: a?.asistenteDocumentosALaNube ?? "",
                         carpetaModelos: "",
                         puertoUsuario: puertoLlama)
    }

    public var revisarLoDicho: Bool { almacen.flatMap { try? $0.nucleo.ajustes().asistenteRevisarLoDicho } ?? false }

    /// El motor elegido en los ajustes (`""`, `claude`, `deepseek`).
    public var motorElegido: String { almacen.flatMap { try? $0.nucleo.ajustes().asistenteMotor } ?? "" }

    /// Cambia el motor de los ajustes (el mismo que Ajustes → Asistente).
    @discardableResult
    public func elegirMotor(_ id: String) async -> Motor {
        if let nucleo = almacen?.nucleo, var a = try? nucleo.ajustes(), a.asistenteMotor != id {
            a.asistenteMotor = id
            try? nucleo.guardarAjustes(nuevos: a)
        }
        return await comprobarMotor()
    }

    /// Mira que motor hay ahora segun los ajustes. Barato: el disco, una
    /// consulta al sistema y, si hace falta, un `GET /health` local.
    @discardableResult
    public func comprobarMotor() async -> Motor {
        let a = ajustes()
        let visto = await Task.detached(priority: .utility) { iaQueMotor(ajustes: a) }.value
        let apple = usarApple && motorAppleDisponible()
        let elegido: Motor
        switch visto {
        case let .nube(_, nombre, _, conClave): elegido = .nube(nombre, conClave: conClave)
        case let .descargado(_, nombre): elegido = .descargado(nombre)
        case let .usuario(puerto): elegido = apple ? .apple : .llamaLocal(puerto)
        case .ninguno: elegido = apple ? .apple : .ninguno
        }
        motor = elegido
        return elegido
    }

    // MARK: Conversaciones

    /// Empieza otra conversacion (la de antes queda guardada).
    public func nuevaConversacion() {
        guardar()
        conversacion = Conversacion()
        burbujas = []
        pendientes = []
    }

    public func abrir(_ c: Conversacion) {
        guard !ocupado else { return }
        guardar()
        conversacion = c
        burbujas = c.burbujas
        pendientes = []
    }

    public func borrar(_ c: Conversacion) {
        historial?.borrar(c.id)
        guardadas.removeAll { $0.id == c.id }
        if c.id == conversacion.id {
            conversacion = Conversacion()
            burbujas = []
        }
    }

    private func guardar() {
        guard let historial, !burbujas.isEmpty else { return }
        conversacion.burbujas = burbujas
        conversacion.cambiada = Date()
        historial.guardar(conversacion)
        guardadas.removeAll { $0.id == conversacion.id }
        guardadas.insert(conversacion, at: 0)
    }

    // MARK: Adjuntos

    /// Donde se guardan las copias de lo adjuntado (lo que viene de Archivos
    /// o de Fotos solo se puede leer un rato: Rust lo lee mas tarde).
    var carpetaAdjuntos: URL {
        (historial?.carpeta.deletingLastPathComponent() ?? FileManager.default.temporaryDirectory)
            .appendingPathComponent("adjuntos", isDirectory: true)
    }

    /// Adjunta ficheros al mensaje que se escribe. Los de fuera de la caja
    /// de arena (Archivos, iCloud) se copian antes.
    public func adjuntar(_ urls: [URL]) {
        let carpeta = carpetaAdjuntos
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        for u in urls where u.isFileURL {
            let accedido = u.startAccessingSecurityScopedResource()
            defer { if accedido { u.stopAccessingSecurityScopedResource() } }
            if ModeloAsistente.esDeLaApp(u) {
                pendientes.append(Adjunto(nombre: u.lastPathComponent, ruta: u.path))
                continue
            }
            let destino = carpeta.appendingPathComponent("\(Int(Date().timeIntervalSince1970 * 1000))-\(u.lastPathComponent)")
            do {
                try FileManager.default.copyItem(at: u, to: destino)
                pendientes.append(Adjunto(nombre: u.lastPathComponent, ruta: destino.path))
            } catch {
                burbujas.append(Burbuja(delUsuario: false, texto: "No se pudo adjuntar «\(u.lastPathComponent)».",
                                        esAviso: true))
            }
        }
    }

    /// Si el fichero ya esta dentro de la carpeta de datos de PixPin (lo
    /// del chat): no hace falta copiarlo.
    nonisolated static func esDeLaApp(_ u: URL) -> Bool {
        let datos = carpetaDeDatos().standardizedFileURL.path
        return u.standardizedFileURL.path.hasPrefix(datos + "/")
    }

    /// Adjunta una imagen (Fotos, la camara, pegada): la guarda junto a las
    /// conversaciones.
    public func adjuntar(imagen datos: Data, nombre: String = "foto") {
        let carpeta = carpetaAdjuntos
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        let png = pngDe(datos) ?? datos
        let f = carpeta.appendingPathComponent("\(nombre)-\(Int(Date().timeIntervalSince1970 * 1000)).png")
        if (try? png.write(to: f)) != nil {
            pendientes.append(Adjunto(nombre: f.lastPathComponent, ruta: f.path))
        }
    }

    /// La foto que mira el modelo, reducida a ~768 px (JPEG).
    nonisolated static func foto(_ ruta: String, lado: Int = 768) -> FotoIa? {
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: lado,
        ]
        guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: ruta) as CFURL, nil),
              let img = CGImageSourceCreateThumbnailAtIndex(src, 0, opciones as CFDictionary)
        else { return nil }
        let datos = NSMutableData()
        guard let dst = CGImageDestinationCreateWithData(datos, UTType.jpeg.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dst, img, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
        guard CGImageDestinationFinalize(dst) else { return nil }
        return FotoIa(bytes: datos as Data, mime: "image/jpeg")
    }

    // MARK: Enviar

    /// Manda un pedido y espera la respuesta entera.
    public func enviar(_ texto: String) async {
        let pedido = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !pedido.isEmpty, !ocupado else { return }
        let historialTexto = burbujas.filter { !$0.esAviso && !$0.texto.isEmpty }
            .map { MensajeIa(delUsuario: $0.delUsuario, texto: $0.texto) }
        var mia = Burbuja(delUsuario: true, texto: pedido)
        mia.adjuntos = pendientes
        conversacion.adjuntos += pendientes
        let nuevos = pendientes
        pendientes = []
        burbujas.append(mia)
        ocupado = true
        defer {
            ocupado = false
            guardar()
        }
        let motor = await comprobarMotor()
        switch motor {
        case .apple:
            await conApple(pedido, historial: Array(historialTexto.suffix(ModeloAsistente.historialMaximo)), aviso: nil)
        case .ninguno:
            burbujas.append(Burbuja(delUsuario: false, texto: ModeloAsistente.textoSinMotor, esAviso: true))
        case .llamaLocal, .descargado, .nube:
            let foto = nuevos.first(where: \.esImagen).map(\.ruta)
            await turno(pedido, historial: historialTexto, foto: foto, motor: motor)
        }
    }

    /// Un turno con Foundation Models. `aviso`: lo que va delante (la nube
    /// fallo y sigue Apple).
    private func conApple(_ pedido: String, historial: [MensajeIa], aviso: String?) async {
        do {
            guard let r = try await preguntarAApple(pedido, historial: historial) else {
                burbujas.append(Burbuja(delUsuario: false, texto: ModeloAsistente.textoSinMotor, esAviso: true))
                return
            }
            var b = Burbuja(delUsuario: false, texto: (aviso ?? "") + r.texto,
                            herramientas: r.herramientas.map(ModeloAsistente.hecha), lienzo: r.lienzo)
            b.motor = Motor.apple.nombre
            burbujas.append(b)
        } catch {
            burbujas.append(Burbuja(delUsuario: false, texto: ModeloAsistente.texto(de: error), esAviso: true))
        }
    }

    static func hecha(_ u: HerramientaUsada) -> Hecha {
        Hecha(nombre: u.nombre, ok: u.ok, resumen: u.resumen)
    }

    /// Un error del nucleo en palabras (sin el nombre del caso), como
    /// `ModeloAjustes.texto(de:)` del Mac.
    nonisolated static func texto(de error: Error) -> String {
        if let e = error as? ErrorNucleo {
            switch e {
            case let .Disco(d), let .Lienzo(d), let .Sincro(d), let .Argumento(d), let .Ajustes(d), let .Asistente(d):
                return d
            case let .NoExiste(que):
                return "no existe: \(que)"
            }
        }
        return error.localizedDescription
    }

    // MARK: En vivo (el nucleo)

    /// El corte del turno en vivo que esta en marcha.
    private var corte: CorteIa?
    /// Las burbujas en vivo que ya tienen su texto final: lo que llegue
    /// tarde por el oyente no se pega dos veces.
    private var cerradas: Set<UUID> = []

    /// Si hay un turno en vivo que se puede parar.
    public var sePuedeParar: Bool { corte != nil }

    /// «Parar»: el modelo deja de escribir y queda lo que llevaba (y una
    /// pregunta pendiente cuenta como «no»).
    public func parar() {
        corte?.cancelar()
        responder(false)
    }

    /// Contesta la pregunta de la burbuja.
    public func responder(_ si: Bool) {
        guard let p = pregunta else { return }
        pregunta = nil
        p.responder(si)
    }

    /// Un turno por el nucleo: la burbuja del asistente aparece vacia y se
    /// va llenando con las tandas (cada ~60 ms como mucho, nunca una llamada
    /// por palabra); al acabar se pone la respuesta entera, con lo pensado,
    /// sus herramientas y lo dibujado.
    func turno(_ pedido: String, historial: [MensajeIa], foto: String?, motor: Motor) async {
        let corte = CorteIa.nuevo()
        self.corte = corte
        defer {
            self.corte = nil
            pregunta = nil
        }
        let burbuja = Burbuja(delUsuario: false, texto: "")
        let id = burbuja.id
        burbujas.append(burbuja)
        let principal: @Sendable (@escaping @MainActor (ModeloAsistente) -> Void) -> Void = { [weak self] hacer in
            DispatchQueue.main.async { MainActor.assumeIsolated { if let self { hacer(self) } } }
        }
        let oyente = OyenteDelAsistente(
            alTexto: { t in principal { $0.cambiar(id) { b in b.texto += t; b.llamando = nil } } },
            alHerramienta: { u in principal { $0.cambiar(id) { b in b.herramientas.append(ModeloAsistente.hecha(u)); b.llamando = nil } } },
            alPensar: { t in principal { $0.cambiar(id) { b in b.pensamiento += t } } },
            alLlamar: { n in principal { $0.cambiar(id) { b in b.llamando = ModeloAsistente.rotuloDe(n) } } })
        let manos = ManosDelAsistente(nucleo: almacen?.nucleo, corte: corte) { [weak self] texto, responder in
            guard let self else { return responder(false) }
            self.pregunta = Pregunta(texto: texto, responder: responder)
        }
        let peticion = PeticionIa(historial: historial, pedido: pedido,
                                  foto: foto.flatMap { ModeloAsistente.foto($0) },
                                  adjuntos: conversacion.adjuntos.map { AdjuntoIa(nombre: $0.nombre, ruta: $0.ruta) },
                                  hoja: conversacion.hoja,
                                  desfaseMs: Int64(TimeZone.current.secondsFromGMT()) * 1000)
        let a = ajustes()
        let nucleo = almacen?.nucleo
        let r: Result<RespuestaIa, Error> = await withCheckedContinuation { c in
            DispatchQueue.global(qos: .utility).async {
                c.resume(returning: Result {
                    try iaTurno(ajustes: a, peticion: peticion, nucleo: nucleo, app: manos, oyente: oyente, corte: corte)
                })
            }
        }
        switch r {
        case let .success(r):
            conversacion.hoja = r.hoja.isEmpty ? conversacion.hoja : r.hoja
            cerrar(id) { b in
                b.texto = r.texto.isEmpty && corte.cortado() ? "(parado)" : r.texto
                b.herramientas = r.herramientas.map(ModeloAsistente.hecha)
                b.lienzo = r.lienzo
                b.pensamiento = r.pensamiento
                b.segundosPensando = r.segundosPensando.map(Double.init)
                b.motor = r.motor
                b.citas = r.citas.map { Cita(numero: Int($0.numero), titulo: $0.titulo, pedido: $0.pedido) }
                b.llamando = nil
            }
        case let .failure(e):
            let texto = ModeloAsistente.texto(de: e)
            // La nube fallo antes de decir nada: si el usuario quiere
            // respaldo, sigue Apple Intelligence.
            let vacia = burbujas.first { $0.id == id }?.texto.isEmpty ?? true
            if motor.esNube, vacia, a.respaldoLocal, usarApple, motorAppleDisponible(), !corte.cortado() {
                cerradas.insert(id)
                burbujas.removeAll { $0.id == id }
                await conApple(pedido, historial: Array(historial.suffix(ModeloAsistente.historialMaximo)),
                               aviso: "⚠️ \(texto)\nSigo con Apple Intelligence.\n\n")
                return
            }
            cerrar(id) { b in
                b.texto = texto
                b.esAviso = true
                b.llamando = nil
            }
        }
    }

    /// Lo que se ve mientras el modelo pide una herramienta.
    static func rotuloDe(_ herramienta: String) -> String {
        switch herramienta {
        case "dibujar_en_lienzo", "grafica", "funciones", "ecuacion", "svg", "cronograma", "fisica",
             "problema_en_limpio": return "Dibujando…"
        case "calcar_imagen": return "Calcando la imagen…"
        case "ocr_pdf", "leer_documento": return "Leyendo…"
        default: return "Haciendo «\(herramienta)»…"
        }
    }

    private func cambiar(_ id: UUID, _ cambio: (inout Burbuja) -> Void) {
        guard !cerradas.contains(id), let i = burbujas.firstIndex(where: { $0.id == id }) else { return }
        cambio(&burbujas[i])
    }

    private func cerrar(_ id: UUID, _ cambio: (inout Burbuja) -> Void) {
        cerradas.insert(id)
        guard let i = burbujas.firstIndex(where: { $0.id == id }) else { return }
        cambio(&burbujas[i])
    }

    // MARK: Documentos

    /// Abre lo citado (el lector en su pagina, o el chat en su mensaje).
    public func abrir(_ c: Cita) {
        if let error = ManosDelAsistente.aLaApp(c.pedido) {
            burbujas.append(Burbuja(delUsuario: false, texto: error, esAviso: true))
        }
    }

    private static var indexando = false

    /// Pone al dia el indice de los documentos en segundo plano (al abrir la
    /// ventana), para que la primera busqueda no espere. Solo si hay quien
    /// conteste: quien no usa el asistente no paga nada.
    public func prepararDocumentos() {
        guard let nucleo = almacen?.nucleo, !ModeloAsistente.indexando, let motor, !motor.faltaClave else { return }
        ModeloAsistente.indexando = true
        let manos = ManosDelAsistente(nucleo: nucleo, corte: CorteIa.nuevo()) { _, responder in responder(false) }
        DispatchQueue.global(qos: .background).async {
            _ = iaDocumentosAlDia(nucleo: nucleo, app: manos)
            DispatchQueue.main.async { MainActor.assumeIsolated { ModeloAsistente.indexando = false } }
        }
    }

    // MARK: Voz

    private static let voz = AVSpeechSynthesizer()

    /// Lee la burbuja en voz alta (otra pulsacion la para).
    public func leerEnVoz(_ b: Burbuja) {
        let voz = ModeloAsistente.voz
        if voz.isSpeaking {
            voz.stopSpeaking(at: .immediate)
            return
        }
        let texto = b.texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !texto.isEmpty else { return }
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
        let frase = AVSpeechUtterance(string: texto)
        frase.voice = AVSpeechSynthesisVoice(language: VozAlta.idiomaDeLaApp)
        voz.speak(frase)
    }

    // MARK: Lo dibujado

    /// Guarda lo dibujado en «Mensajes guardados» como un `.excalidraw`
    /// (el formato del lienzo de Windows y Android) y, con `abrir`, lo abre
    /// en el lienzo.
    @discardableResult
    public func guardarDibujo(_ b: Burbuja, abrir: Bool = false) async -> Bool {
        guard let lienzo = b.lienzo, let almacen else { return false }
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-asistente-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: carpeta) }
        do {
            try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
            let ms = Int64(Date().timeIntervalSince1970 * 1000)
            let fichero = carpeta.appendingPathComponent("asistente-\(ms).excalidraw")
            try Data(lienzo.utf8).write(to: fichero)
            let nucleo = almacen.nucleo
            let guardados = try await Task.detached(priority: .utility) { try nucleo.chatGuardados() }.value
            let hechos = await almacen.adjuntar(urls: [fichero], en: guardados.id)
            if abrir, let m = hechos.first, m.referencia != nil {
                Navegacion.compartida.abrir(.lienzo(chat: guardados.id, mensaje: m, foto: nil))
            }
            return !hechos.isEmpty
        } catch {
            return false
        }
    }
}
