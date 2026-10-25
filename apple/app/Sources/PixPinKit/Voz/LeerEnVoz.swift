// **Escuchar un documento en voz alta** (tanda D2): es `leer_en_voz.rs` de
// Windows (el `LectorEnVoz` del movil) con la voz del Mac
// (`AVSpeechSynthesizer`). Traido del Mac sin AppKit (sin el panel
// flotante: en el iPad la barra la pinta quien la quiera).
//
// # Una sola voz para la app
//
// Como alli, **la voz no es de la ventana del lector**: vive en la app
// (`LeerEnVoz.compartida`) y **sigue sonando al cerrar el lector**. El lector
// solo se engancha a ella (`LectorEnganchado`) para resaltar lo que suena y
// mover el marcador verde (🟢); al volver a abrir el mismo documento se
// engancha otra vez y va al parrafo que suena. Con el lector cerrado el verde
// lo apunta la propia voz (`Documento.apuntar`).
//
// Lo que en Windows son los mandos del recuadro del volumen aqui es
// «Está sonando» del Mac y las teclas multimedia (`MandosDelSistema`): pausa,
// parrafo anterior y siguiente, y parar.
//
// Por donde va: cada parrafo es una frase o unas pocas (`VozAlta.cortes`) y,
// dentro, la palabra que se dice (`willSpeakRangeOfSpeechString`), que el
// lector resalta encima del ambar del parrafo. La pausa es a media palabra y
// sigue ahi; cambiar la velocidad o la voz sigue desde la palabra que sonaba.

import AVFoundation
import Combine
import SwiftUI

/// Un lector (o la nota) enganchado a la voz: resalta y mueve el verde.
@MainActor
protocol LectorEnganchado: AnyObject {
    /// Empieza a sonar el parrafo `i` (o se situa en el, en pausa).
    func vozSuena(_ i: Int, primero: Bool)
    /// La palabra que se dice, en UTF-16 dentro del parrafo que suena.
    func vozPalabra(_ r: NSRange?)
    /// Se acabo el documento: fuera el verde.
    func vozAcabo()
    /// La voz ya no es de este documento (✕, los mandos del Mac u otro).
    func vozFuera()
    /// Si pinta su barra de escuchar (el lector); si no, flota la de la app.
    var pintaSuBarra: Bool { get }
}

extension LectorEnganchado {
    var pintaSuBarra: Bool { true }
}

/// Lo que hace sonar las frases. El de verdad es `AVSpeechSynthesizer`; las
/// pruebas ponen uno mudo.
@MainActor
protocol Altavoz: AnyObject {
    /// Dice `texto`; avisa con `alDecir` de cada palabra (rango en `texto`) y
    /// con `alAcabar` al terminar (no al callarlo).
    func hablar(_ texto: String, voz: AVSpeechSynthesisVoice?, tasa: Float,
                alDecir: @escaping @MainActor (NSRange) -> Void, alAcabar: @escaping @MainActor () -> Void)
    func pausar()
    func seguir()
    func callar()
}

@MainActor
public final class LeerEnVoz: ObservableObject {
    public static let compartida = LeerEnVoz.deLaApp()

    /// **Lo que hay que leer**, de que documento y como.
    public struct Documento {
        /// De que documento es (su ruta): el lector la compara con la suya.
        public var clave: String
        /// Lo que se ensena en «Está sonando».
        public var titulo: String
        public var parrafos: [String]
        /// El idioma con el que se lee (`VozAlta.idiomaParaLeer`).
        public var idioma: String
        /// **Apunta el verde** con el lector cerrado: `i`, el parrafo que
        /// suena; `nil`, se acabo.
        public var apuntar: (@MainActor (Int?) -> Void)?

        public init(clave: String, titulo: String, parrafos: [String], idioma: String,
                    apuntar: (@MainActor (Int?) -> Void)? = nil) {
            self.clave = clave
            self.titulo = titulo
            self.parrafos = parrafos
            self.idioma = idioma
            self.apuntar = apuntar
        }
    }

    /// El documento que suena o esta en pausa.
    @Published public private(set) var clave: String?
    @Published public private(set) var titulo = ""
    @Published public private(set) var cuantos = 0
    /// El parrafo que suena (o donde se paro).
    @Published public private(set) var actual = 0
    @Published public private(set) var leyendo = false
    /// La velocidad sobrevive al cambio de documento, como en el movil.
    @Published public private(set) var velocidad: Float = 1
    /// «Mónica · mejorada»: la voz que lee.
    @Published public private(set) var nombreVoz = ""
    @Published public private(set) var idioma = ""
    /// Cuantas veces se acabo un documento.
    @Published public private(set) var acabados = 0

    private let altavoz: Altavoz
    private var parrafos: [String] = []
    private var apuntar: (@MainActor (Int?) -> Void)?
    private weak var enganchado: LectorEnganchado? {
        didSet { conLector = enganchado?.pintaSuBarra ?? false }
    }
    /// Hay una ventana enganchada que pinta su barra. Sin ninguna (el lector
    /// se cerro, una nota) la barra flota sola (`PanelEscuchar`).
    @Published public private(set) var conLector = false
    private var voz: AVSpeechSynthesisVoice?
    /// Cada frase que se le da al altavoz es otra vuelta: lo que avise una
    /// vieja (callada a medias) no cuenta.
    private var vuelta = 0
    /// Donde empieza lo que se esta diciendo, dentro del parrafo (al seguir
    /// desde una palabra tras cambiar de velocidad).
    private var desde = 0
    /// La palabra que sono la ultima, dentro del parrafo.
    private(set) var palabra: NSRange?
    /// En pausa a media frase: seguir es seguir ahi.
    private var aMedias = false
    /// En pausa sin frase a medias: seguir empieza el parrafo aqui.
    private var reanudarEn = 0

    init(altavoz: Altavoz) {
        self.altavoz = altavoz
    }

    /// La de la app. En el iPad no hay panel flotante: la barra de
    /// escuchar (`BarraEscuchar`) la pinta quien la quiera (el lector, la
    /// capa de pines) cuando `conLector` es falso.
    private static func deLaApp() -> LeerEnVoz {
        let l = LeerEnVoz(altavoz: AltavozMicrosoft())
        l.velocidad = VelocidadGuardada.leer(VelocidadGuardada.claveLeer, validas: VozAlta.velocidades)
        l.recordarVelocidad = true
        return l
    }

    /// Se recuerda la velocidad (solo la de la app: las pruebas no tocan
    /// las preferencias).
    private var recordarVelocidad = false

    /// Si lo que suena (o esta en pausa) es este documento.
    public func suena(_ clave: String) -> Bool { self.clave == clave }

    /// Hay algo que leer (sonando o en pausa).
    public var vivo: Bool { clave != nil }

    // MARK: Empezar y engancharse

    /// **Empieza a escuchar `doc`** (se calla lo que sonara, sea voz o
    /// audio). No suena hasta `leer(desde:)`. `false` si el Mac no tiene voz
    /// para su idioma ni para el de la app.
    @discardableResult
    func arrancar(_ doc: Documento, lector: LectorEnganchado?) -> Bool {
        let v = VozAlta.voz(para: doc.idioma, preferida: VozAlta.preferida(lengua: doc.idioma))
            ?? VozAlta.voz(para: VozAlta.idiomaDeLaApp, preferida: VozAlta.preferida(lengua: VozAlta.idiomaDeLaApp))
        guard v != nil || altavoz is AltavozMudo else { return false }
        let antes = enganchado
        callar()
        ReproductorVoz.compartido.parar()
        if let antes, antes !== lector { antes.vozFuera() }
        voz = v
        clave = doc.clave
        titulo = doc.titulo
        parrafos = doc.parrafos
        cuantos = doc.parrafos.count
        idioma = doc.idioma
        apuntar = doc.apuntar
        enganchado = lector
        actual = 0
        leyendo = false
        reanudarEn = 0
        nombreVoz = altavoz is AltavozMicrosoft && EdgeVoz.activas
            ? EdgeVoz.nombre(EdgeVoz.voz(para: v?.language ?? doc.idioma))
            : v.map(VozAlta.nombre(de:)) ?? ""
        MandosDelSistema.compartidos.tomar(self, porPasos: true)
        alDia()
        return true
    }

    /// **Se engancha a lo que suena** si es `clave` y tiene `cuantos`
    /// parrafos (el mismo documento contado igual): al volver a abrirlo.
    func enganchar(_ lector: LectorEnganchado, clave: String, cuantos: Int) -> Bool {
        guard self.clave == clave, self.cuantos == cuantos else { return false }
        enganchado = lector
        lector.vozSuena(actual, primero: true)
        lector.vozPalabra(palabra)
        return true
    }

    /// El lector se cierra: la voz sigue y desde ahora el verde lo apunta
    /// ella.
    func desenganchar(_ lector: LectorEnganchado) {
        if enganchado === lector { enganchado = nil }
    }

    // MARK: Ordenes

    /// Empieza a leer en el parrafo `desde`, por su principio.
    public func leer(desde i: Int) {
        guard clave != nil, cuantos > 0 else { return }
        actual = min(max(i, 0), cuantos - 1)
        leyendo = true
        decir(desde: 0)
        avisar(primero: true)
    }

    /// Play/pausa.
    public func alternar() {
        guard clave != nil, cuantos > 0 else { return }
        if leyendo {
            altavoz.pausar()
            aMedias = true
            leyendo = false
        } else {
            leyendo = true
            if aMedias {
                aMedias = false
                altavoz.seguir()
            } else {
                decir(desde: reanudarEn)
            }
        }
        alDia()
    }

    /// Un parrafo adelante (+1) o atras (−1), desde su principio. Devuelve
    /// el parrafo en el que queda.
    @discardableResult
    public func saltar(_ n: Int) -> Int {
        guard clave != nil, cuantos > 0 else { return 0 }
        actual = min(max(actual + n, 0), cuantos - 1)
        if leyendo {
            decir(desde: 0)
        } else {
            parar()
            reanudarEn = 0
            palabra = nil
        }
        avisar(primero: true)
        return actual
    }

    /// La velocidad siguiente; sonando, se sigue con ella desde la palabra
    /// que sonaba.
    @discardableResult
    public func otraVelocidad() -> Float {
        velocidad = VozAlta.siguienteVelocidad(velocidad)
        if recordarVelocidad { VelocidadGuardada.guardar(velocidad, clave: VelocidadGuardada.claveLeer) }
        seguirConOtraVoz()
        return velocidad
    }

    /// **Leer con otra voz** (la del menu de los ajustes): se recuerda para
    /// su lengua y, sonando, sigue con ella.
    public func ponerVoz(_ v: AVSpeechSynthesisVoice) {
        VozAlta.ponerPreferida(v.identifier, lengua: v.language)
        guard clave != nil else { return }
        voz = v
        nombreVoz = VozAlta.nombre(de: v)
        seguirConOtraVoz()
    }

    /// **Deja de escuchar**: calla y olvida el documento (la ✕ de la barra).
    public func soltar() {
        let lector = enganchado
        callar()
        clave = nil
        titulo = ""
        parrafos = []
        cuantos = 0
        actual = 0
        leyendo = false
        apuntar = nil
        enganchado = nil
        MandosDelSistema.compartidos.soltar(self)
        lector?.vozFuera()
    }

    /// Lo pausa si esta leyendo (empieza a sonar un audio).
    func pausarSiLee() {
        if leyendo { alternar() }
    }

    // MARK: Por dentro

    private func seguirConOtraVoz() {
        guard clave != nil else { return }
        let aqui = palabra?.location ?? desde
        if leyendo {
            decir(desde: aqui)
        } else if aMedias {
            parar()
            reanudarEn = aqui
        }
        alDia()
    }

    /// Dice el parrafo `actual` a partir de la letra `desde` (UTF-16).
    private func decir(desde: Int) {
        parar()
        guard actual < parrafos.count else { return }
        let ns = parrafos[actual] as NSString
        let desde = min(max(desde, 0), ns.length)
        self.desde = desde
        reanudarEn = 0
        palabra = nil
        enganchado?.vozPalabra(nil)
        let texto = VozAlta.paraDecir(ns.substring(from: desde))
        vuelta += 1
        let esta = vuelta
        altavoz.hablar(texto, voz: voz, tasa: VozAlta.tasaDeApple(velocidad),
                       alDecir: { [weak self] r in self?.alDecir(r, vuelta: esta) },
                       alAcabar: { [weak self] in self?.alAcabar(vuelta: esta) })
        alDia()
    }

    private func alDecir(_ r: NSRange, vuelta: Int) {
        guard vuelta == self.vuelta else { return }
        let p = NSRange(location: desde + r.location, length: r.length)
        palabra = p
        enganchado?.vozPalabra(p)
    }

    private func alAcabar(vuelta: Int) {
        guard vuelta == self.vuelta, leyendo else { return }
        if actual + 1 < cuantos {
            actual += 1
            decir(desde: 0)
            avisar(primero: false)
        } else {
            acabar()
        }
    }

    /// **Se acabo el documento**: el verde se quita y la proxima vez se
    /// empieza por donde se mire (`alAcabarElDocumento`). La barra se queda.
    private func acabar() {
        parar()
        leyendo = false
        actual = 0
        reanudarEn = 0
        acabados += 1
        palabra = nil
        if let l = enganchado { l.vozAcabo() } else { apuntar?(nil) }
        alDia()
    }

    /// Le cuenta el parrafo nuevo al lector, o apunta el verde si no hay.
    private func avisar(primero: Bool) {
        if let l = enganchado { l.vozSuena(actual, primero: primero) } else { apuntar?(actual) }
        alDia()
    }

    /// Calla la frase (sin olvidar el documento).
    private func parar() {
        vuelta += 1
        aMedias = false
        altavoz.callar()
    }

    private func callar() {
        parar()
        palabra = nil
    }

    /// «Está sonando» del Mac, al dia.
    private func alDia() {
        guard clave != nil else { return }
        let sub = "Párrafo \(actual + 1) de \(max(cuantos, 1))" + (nombreVoz.isEmpty ? "" : " · \(nombreVoz)")
        MandosDelSistema.compartidos.poner(LoQueSuena(titulo: titulo, subtitulo: sub, velocidad: velocidad,
                                                      sonando: leyendo), de: self)
    }
}

extension LeerEnVoz: DuenoDeLosMandos {
    func mandoTocar() { if !leyendo { alternar() } }
    func mandoPausar() { if leyendo { alternar() } }
    func mandoAlternar() { alternar() }
    func mandoParar() { soltar() }
    func mandoPaso(_ sentido: Int) { saltar(sentido) }
    func mandoSaltar(segundos: Double) {}
    func mandoIrA(segundos: Double) {}
}

// MARK: - El altavoz del Mac

/// `AVSpeechSynthesizer` detras del `Altavoz`.
@MainActor
final class AltavozDelMac: NSObject, Altavoz, AVSpeechSynthesizerDelegate {
    private let sintetizador = AVSpeechSynthesizer()
    private var dicho: AVSpeechUtterance?
    private var alDecir: (@MainActor (NSRange) -> Void)?
    private var alAcabar: (@MainActor () -> Void)?

    override init() {
        super.init()
        sintetizador.delegate = self
    }

    func hablar(_ texto: String, voz: AVSpeechSynthesisVoice?, tasa: Float,
                alDecir: @escaping @MainActor (NSRange) -> Void, alAcabar: @escaping @MainActor () -> Void) {
        // iOS: que suene aunque el iPad este en silencio (es leer, no avisar).
        ReproductorVoz.sesionDeEscuchar()
        let u = AVSpeechUtterance(string: texto)
        u.voice = voz
        u.rate = tasa
        dicho = u
        self.alDecir = alDecir
        self.alAcabar = alAcabar
        sintetizador.speak(u)
    }

    func pausar() { sintetizador.pauseSpeaking(at: .word) }
    func seguir() { sintetizador.continueSpeaking() }

    func callar() {
        dicho = nil
        alDecir = nil
        alAcabar = nil
        sintetizador.stopSpeaking(at: .immediate)
    }

    nonisolated func speechSynthesizer(_ s: AVSpeechSynthesizer, willSpeakRangeOfSpeechString r: NSRange,
                                       utterance: AVSpeechUtterance) {
        nonisolated(unsafe) let u = utterance
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                if u === self.dicho { self.alDecir?(r) }
            }
        }
    }

    nonisolated func speechSynthesizer(_ s: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        nonisolated(unsafe) let u = utterance
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                guard u === self.dicho else { return }
                self.dicho = nil
                self.alAcabar?()
            }
        }
    }
}

/// Un altavoz que no suena: el de las pruebas, que dicen ellas cuando se
/// acaba cada frase.
@MainActor
final class AltavozMudo: Altavoz {
    var dichos: [String] = []
    var pausado = false
    private(set) var alDecir: (@MainActor (NSRange) -> Void)?
    private(set) var alAcabar: (@MainActor () -> Void)?

    func hablar(_ texto: String, voz: AVSpeechSynthesisVoice?, tasa: Float,
                alDecir: @escaping @MainActor (NSRange) -> Void, alAcabar: @escaping @MainActor () -> Void) {
        dichos.append(texto)
        pausado = false
        self.alDecir = alDecir
        self.alAcabar = alAcabar
    }

    func pausar() { pausado = true }
    func seguir() { pausado = false }

    func callar() {
        alDecir = nil
        alAcabar = nil
    }

    /// Como si la voz acabara la frase.
    func acabar() {
        let a = alAcabar
        alAcabar = nil
        a?()
    }
}

// MARK: - La barra de escuchar

/// **La barra de escuchar**, abajo y de cristal como la de Windows
/// (`pintar_barra`): atras un parrafo, play/pausa, adelante uno, la velocidad
/// (rota al pulsarla) y cerrar. Debajo, que voz lee y por donde va.
struct BarraEscuchar: View {
    @ObservedObject var voz: LeerEnVoz
    /// El documento de esta ventana: la barra solo sale si es el que suena.
    let clave: () -> String
    /// Con el nombre del documento delante (la barra flotante).
    var conTitulo = false

    var body: some View {
        if voz.suena(clave()) {
            VStack(spacing: 2) {
                HStack(spacing: 0) {
                    boton("backward.end.fill", "Párrafo anterior") { voz.saltar(-1) }
                    boton(voz.leyendo ? "pause.fill" : "play.fill", voz.leyendo ? "Pausa (L)" : "Escuchar (L)") {
                        voz.alternar()
                    }
                    boton("forward.end.fill", "Párrafo siguiente") { voz.saltar(1) }
                    Button { voz.otraVelocidad() } label: {
                        Text(VozAlta.rotulo(voz.velocidad))
                            .font(.system(size: 14).monospacedDigit())
                            .padding(.horizontal, 10)
                            .frame(height: 36)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Velocidad")
                    boton("xmark", "Dejar de escuchar (⎋)") { voz.soltar() }
                }
                Text((conTitulo ? voz.titulo + " · " : "")
                    + "\(voz.nombreVoz.isEmpty ? "Voz" : voz.nombreVoz) · \(voz.actual + 1) / \(max(voz.cuantos, 1))")
                    .font(.system(size: 11))
                    .foregroundStyle(Color(white: 0.78))
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .frame(maxWidth: 360)
            }
            .foregroundStyle(Color(red: 0.96, green: 0.96, blue: 0.97))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color(red: 0.118, green: 0.118, blue: 0.125, opacity: 0.95),
                        in: RoundedRectangle(cornerRadius: 20))
            .environment(\.colorScheme, .dark)
        }
    }

    private func boton(_ simbolo: String, _ ayuda: String, _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Image(systemName: simbolo).font(.system(size: 15)).frame(width: 40, height: 36)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(ayuda)
    }
}

/// **La barra de escuchar suelta**, para lo que se lee sin un lector abierto
/// que pinte la suya (un mensaje del chat, «Escuchar»): sale mientras hay
/// algo que leer y nadie la pinta.
struct BarraEscucharSuelta: View {
    @ObservedObject private var voz = LeerEnVoz.compartida

    var body: some View {
        if voz.vivo, !voz.conLector {
            BarraEscuchar(voz: voz, clave: { voz.clave ?? "" }, conTitulo: true)
        }
    }
}
