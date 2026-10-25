// **El reproductor de la app: uno para todo** (tanda D3). Es `audio.rs` de
// Windows (el `object Reproductor` del movil): antes cada nota de voz era un
// boton de play y poco mas; ahora el estado —que suena, por donde va, a que
// velocidad— vive en un solo sitio y lo leen la barra de lo que suena
// (encima de la caja de escribir del chat y en la biblioteca), las burbujas,
// la biblioteca de audio, el reproductor flotante y «Está sonando» del Mac.
//
// - **Uno solo a la vez**: tocar otra nota suelta la de antes. Si la voz
//   leia un documento, se pausa (dos voces a la vez no se entienden).
// - **La velocidad sobrevive** al cambio de pista y a parar: quien escucha
//   todo a 1,5× no quiere volver a ponerlo. Son las cinco del movil, en su
//   orden (1, 1,25, 1,5, 2, 0,75), y no cambian el tono.
// - Atras y adelante son **10 s**; ir a un punto de la barra no hace nada
//   mientras no se sepa cuanto dura.
// - Al acabar se queda cargada al principio: otro toque la repite. Si hay
//   fila (varios audios al flotante), empieza la siguiente.
//
// Solo publica lo que cambia poco (que suena, si suena, la velocidad): el
// avance lo lee quien pinta con un `TimelineView` mientras suena, asi las
// demas burbujas no se repintan diez veces por segundo.

import AVFoundation
import Foundation

/// Un audio que se puede poner: lo que suena y lo que se lee en la barra.
public struct Pista: Equatable, Sendable {
    /// De quien es: el id del mensaje, o la ruta si viene de fuera.
    public var id: String
    public var url: URL
    public var titulo: String

    public init(id: String, url: URL, titulo: String) {
        self.id = id
        self.url = url
        self.titulo = titulo
    }
}

@MainActor
public final class ReproductorVoz: NSObject, ObservableObject {
    public static let compartido = ReproductorVoz.deLaApp()

    /// Las cinco velocidades del movil, en su orden (`VELOCIDADES`).
    public nonisolated static let velocidades: [Float] = [1.0, 1.25, 1.5, 2.0, 0.75]
    /// Lo que saltan los botones de atras y adelante (`SALTO_MS`).
    public nonisolated static let saltoMs: Int64 = 10_000

    /// El id de lo cargado (sonando o en pausa).
    @Published public private(set) var actual: String?
    @Published public private(set) var sonando = false
    @Published public private(set) var titulo = ""
    @Published public private(set) var velocidad: Float = 1
    /// Cuantas pistas se acabaron (el flotante se va al acabar la ultima).
    @Published public private(set) var acabadas = 0
    /// Lo que espera en la fila, detras de lo que suena.
    @Published public private(set) var cola: [Pista] = []
    public private(set) var url: URL?

    private var reproductor: AVAudioPlayer?
    /// Sin tarjeta de sonido (las pruebas): todo igual, pero no suena.
    var mudo = false

    override public init() { super.init() }

    /// Lo que se recuerda la velocidad (solo el de la app: las pruebas no
    /// tocan las preferencias).
    private var esDeLaApp = false

    /// El de la app: con la velocidad de la ultima vez.
    private static func deLaApp() -> ReproductorVoz {
        let r = ReproductorVoz()
        r.velocidad = VelocidadGuardada.leer(VelocidadGuardada.claveEscuchar, validas: velocidades)
        r.esDeLaApp = true
        return r
    }

    /// iOS: la sesion de sonido para escuchar (suena aunque el iPad este en
    /// silencio, como un reproductor, y sigue con la app de fondo si se
    /// pide en el Info.plist).
    static func sesionDeEscuchar() {
        let s = AVAudioSession.sharedInstance()
        if s.category != .playback {
            try? s.setCategory(.playback, mode: .spokenAudio)
        }
        try? s.setActive(true)
    }

    // MARK: Lo que se lee

    /// Hay algo cargado: la barra solo se pinta entonces.
    public var hayAlgo: Bool { actual != nil }

    /// Si suena esto (el boton de la burbuja: pausa o play).
    public func suena(_ id: String) -> Bool { sonando && actual == id }

    /// Lo que dura lo cargado, en milisegundos (0 si no hay nada).
    public var duracionMs: Int64 {
        guard let r = reproductor, r.duration.isFinite else { return 0 }
        return Int64(r.duration * 1000)
    }

    /// Por donde va, en milisegundos.
    public var posicionMs: Int64 {
        guard let r = reproductor else { return 0 }
        return Int64(r.currentTime * 1000)
    }

    /// Por donde va, de 0 a 1 (0 si no hay nada o no se sabe cuanto dura).
    public var fraccion: Double {
        Double(ReproductorVoz.fraccion(posicionMs: posicionMs, duracionMs: duracionMs))
    }

    // MARK: Ordenes

    /// Toca lo que ya suena: pausa o sigue. Toca otra cosa: la carga y
    /// arranca (`alternar`). Es el clic en una burbuja o en una fila de la
    /// biblioteca.
    public func alternar(_ id: String, url: URL, titulo: String = "") {
        if actual == id, reproductor != nil {
            if sonando { pausar() } else { seguir() }
            return
        }
        cola = []
        cargar(Pista(id: id, url: url, titulo: titulo), arrancar: true)
    }

    /// **Pone una fila**: la primera suena ya y las demas esperan; al acabar
    /// una empieza la siguiente (el reproductor flotante). Sustituye lo que
    /// hubiera.
    public func poner(_ pistas: [Pista]) {
        guard let primera = pistas.first else { return }
        cola = Array(pistas.dropFirst())
        cargar(primera, arrancar: true)
    }

    /// **A la cola** (la biblioteca, «Escuchar después»): detras de lo que
    /// espera. Sin nada cargado, suena ya.
    public func encolar(_ p: Pista) {
        guard actual != nil else {
            cargar(p, arrancar: true)
            return
        }
        guard p.id != actual, !cola.contains(where: { $0.id == p.id }) else { return }
        cola.append(p)
    }

    /// Quita de la cola lo que esperaba (no lo que suena).
    public func quitarDeLaCola(_ id: String) {
        cola.removeAll { $0.id == id }
    }

    /// Pasa ya a lo siguiente de la cola (lo de ahora se deja).
    public func siguiente() {
        guard !cola.isEmpty else { return }
        cargar(cola.removeFirst(), arrancar: true)
    }

    /// Carga un audio y, si se pide, arranca. Suelta lo de antes. `false` si
    /// el fichero no se puede abrir (no queda una barra apuntando a nada).
    @discardableResult
    public func cargar(_ p: Pista, arrancar: Bool) -> Bool {
        soltarElReproductor()
        guard let r = try? AVAudioPlayer(contentsOf: p.url) else {
            parar()
            return false
        }
        r.delegate = self
        r.enableRate = true
        r.rate = velocidad
        r.prepareToPlay()
        reproductor = r
        actual = p.id
        url = p.url
        titulo = p.titulo.isEmpty ? p.url.deletingPathExtension().lastPathComponent : p.titulo
        sonando = false
        MandosDelSistema.compartidos.tomar(self, porPasos: false)
        // Fuera del chat, lo que suena sale en la barra flotante.
        if esDeLaApp { ReproductorFlotante.compartido.vigilar() }
        if arrancar { seguir() } else { alDia() }
        return true
    }

    /// Arranca o sigue.
    public func seguir() {
        guard let r = reproductor else { return }
        LeerEnVoz.compartida.pausarSiLee()
        ReproductorVoz.sesionDeEscuchar()
        r.rate = velocidad
        sonando = mudo ? true : r.play()
        alDia()
    }

    public func pausar() {
        reproductor?.pause()
        sonando = false
        alDia()
    }

    /// Descarga lo que hubiera: la barra desaparece. Conserva la velocidad.
    public func parar() {
        soltarElReproductor()
        actual = nil
        url = nil
        titulo = ""
        sonando = false
        cola = []
        MandosDelSistema.compartidos.soltar(self)
    }

    /// Adelanta o atrasa `ms` (negativo para atras), sin salirse.
    public func saltar(ms: Int64) {
        guard let r = reproductor else { return }
        let destino = ReproductorVoz.destinoAlSaltar(posicionMs: posicionMs, ms: ms, duracionMs: duracionMs)
        r.currentTime = ReproductorVoz.segundos(destino, duracionMs: duracionMs)
        alDia()
    }

    public func atras() { saltar(ms: -ReproductorVoz.saltoMs) }
    public func adelante() { saltar(ms: ReproductorVoz.saltoMs) }

    /// Va a un punto de la barra, de 0 a 1 (`ir_a`).
    public func irA(fraccion f: Double) {
        guard let r = reproductor,
              let destino = ReproductorVoz.destinoDeFraccion(Float(f), duracionMs: duracionMs) else { return }
        r.currentTime = ReproductorVoz.segundos(destino, duracionMs: duracionMs)
        alDia()
    }

    /// Salta a una fraccion de la nota `id` si es la cargada (la onda de la
    /// burbuja).
    public func saltar(_ id: String, a fraccion: Double) {
        guard actual == id else { return }
        irA(fraccion: fraccion)
    }

    /// La siguiente de las cinco velocidades, en ciclo. Sonando, ya.
    @discardableResult
    public func otraVelocidad() -> Float {
        velocidad = ReproductorVoz.siguienteVelocidad(velocidad)
        if esDeLaApp { VelocidadGuardada.guardar(velocidad, clave: VelocidadGuardada.claveEscuchar) }
        if let r = reproductor, sonando { r.rate = velocidad }
        alDia()
        return velocidad
    }

    // MARK: Por dentro

    private func soltarElReproductor() {
        reproductor?.stop()
        reproductor?.delegate = nil
        reproductor = nil
    }

    func acabo() {
        acabadas += 1
        if !cola.isEmpty {
            cargar(cola.removeFirst(), arrancar: true)
            return
        }
        // Se queda cargada al principio, como el movil: otro toque la repite.
        reproductor?.currentTime = 0
        sonando = false
        alDia()
    }

    /// «Está sonando» del Mac, al dia.
    private func alDia() {
        guard actual != nil else { return }
        MandosDelSistema.compartidos.poner(LoQueSuena(titulo: titulo, duracion: Double(duracionMs) / 1000,
                                                      posicion: Double(posicionMs) / 1000,
                                                      velocidad: velocidad, sonando: sonando), de: self)
    }

    // MARK: Las cuentas (las de `pixpin_audio::reloj`)

    /// La siguiente velocidad del ciclo; una que no este en la lista vuelve
    /// a la primera.
    public nonisolated static func siguienteVelocidad(_ actual: Float) -> Float {
        guard let i = velocidades.firstIndex(where: { abs($0 - actual) < 1e-4 }) else { return velocidades[0] }
        return velocidades[(i + 1) % velocidades.count]
    }

    /// A donde lleva saltar `ms` (negativo para atras): entre 0 y lo que dura.
    public nonisolated static func destinoAlSaltar(posicionMs: Int64, ms: Int64, duracionMs: Int64) -> Int64 {
        let tope = max(duracionMs, 0)
        let (suma, desborda) = posicionMs.addingReportingOverflow(ms)
        return min(max(desborda ? (ms > 0 ? Int64.max : 0) : suma, 0), tope)
    }

    /// A donde lleva pinchar en la fraccion `f` de la barra; `nil` si aun no
    /// se sabe cuanto dura.
    public nonisolated static func destinoDeFraccion(_ f: Float, duracionMs: Int64) -> Int64? {
        guard duracionMs > 0 else { return nil }
        let f = f.isNaN ? 0 : min(max(f, 0), 1)
        return Int64(Double(duracionMs) * Double(f))
    }

    /// El punto en segundos para `AVAudioPlayer`: el final justo lo devuelve
    /// al principio, asi que se queda un pelo antes y acaba solo.
    nonisolated static func segundos(_ ms: Int64, duracionMs: Int64) -> TimeInterval {
        TimeInterval(duracionMs > 0 && ms >= duracionMs ? max(duracionMs - 30, 0) : ms) / 1000
    }

    /// Por donde va, de 0 a 1 (0 sin duracion).
    public nonisolated static func fraccion(posicionMs: Int64, duracionMs: Int64) -> Float {
        guard duracionMs > 0 else { return 0 }
        return min(max(Float(posicionMs) / Float(duracionMs), 0), 1)
    }
}

extension ReproductorVoz: AVAudioPlayerDelegate {
    nonisolated public func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        nonisolated(unsafe) let p = player
        Task { @MainActor [weak self] in
            if let self, self.reproductor === p { self.acabo() }
        }
    }

    nonisolated public func audioPlayerDecodeErrorDidOccur(_ player: AVAudioPlayer, error: Error?) {
        nonisolated(unsafe) let p = player
        Task { @MainActor [weak self] in
            if let self, self.reproductor === p { self.parar() }
        }
    }
}

extension ReproductorVoz: DuenoDeLosMandos {
    func mandoTocar() { seguir() }
    func mandoPausar() { pausar() }
    func mandoAlternar() { if sonando { pausar() } else { seguir() } }
    func mandoParar() { parar() }
    func mandoPaso(_ sentido: Int) { saltar(ms: Int64(sentido) * ReproductorVoz.saltoMs) }
    func mandoSaltar(segundos: Double) { saltar(ms: Int64(segundos * 1000)) }
    func mandoIrA(segundos: Double) {
        guard duracionMs > 0 else { return }
        irA(fraccion: segundos * 1000 / Double(duracionMs))
    }
}
