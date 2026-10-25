// **Estudiar una clase grabada** (Windows ce6c058, `estudiar_clase.rs`, el
// usuario: «una ventana especial … donde se vea la grabacion del video con
// su transcripcion al lado; al otro lado las capturas, los marcadores … y
// que las capturas y el texto se puedan desplazar de forma que uno siga al
// otro»). Del Mac (`Grabar/EstudiarClase.swift`), en el iPad a pantalla
// completa en vez de una ventana. Sustituye a la pantalla de la letra con
// las capturas (`VisorClase`).
//
// Una sola a la vez (otra clase pedida la cambia):
//
// - **A la izquierda, lo que se ve**: el video de la clase (con su sonido)
//   o, si no se grabo video, la captura del minuto que suena en grande y la
//   nota de voz sonando. Debajo, la barra del tiempo con una raya por
//   captura y una banderita por marcador (tocar cerca se pega a ellas), los
//   mandos (atras, reproducir, adelante, la velocidad, «Marcador» y
//   «Seguir») y la lista de marcadores.
// - **En el centro, el texto**, con su minuto delante de cada trozo y los
//   marcadores metidos en su sitio. Un toque lleva ahi; lo que suena se
//   resalta.
// - **A la derecha, las capturas** en una tira, con su minuto. Un toque
//   lleva ahi y la ensena en grande.
//
// En un iPad estrecho (vertical, Split View) lo que se ve va arriba y el
// texto y la tira debajo, uno al lado del otro.
//
// **Que una siga a la otra** (las cuentas de `pixpin_proyecto::clase`, por
// el nucleo: `claseScrollPara`, `claseSeguir`, `claseAcercar`): con
// «Seguir», mientras suena el texto y la tira van solos a lo que suena; al
// mover una con el dedo, la otra se pone en el mismo minuto, y se deja de
// seguir al sonido cinco segundos, para poder leer.
//
// **Los marcadores** se guardan en la nota de voz (`resto["marcadores"]`,
// `Nucleo.ponerMarcadorDeClase`), que viaja al movil, a Windows y al Mac
// como lo demas de `resto`; las banderitas del movil (`marcas`) se ensenan
// con ellos y cada una se quita de su campo.
//
// Con teclado: espacio o K reproduce y para, ← → cinco segundos, J/L diez,
// ↑ ↓ el trozo anterior y el siguiente, M pone un marcador (se escribe su
// nota e ↩ lo guarda; ⎋ lo deja), S enciende y apaga «Seguir», ⎋ cierra.
//
// Los textos son los de Windows (`estudiar-*` de `main.ftl`).

import AVFoundation
import ImageIO
import PixPinCore
import SwiftUI
import UIKit

/// La pantalla de estudiar una clase: una sola.
@MainActor
enum EstudiarClase {
    private static var modelo: ModeloEstudiar?

    /// **Abre la clase de `mensaje`** (la nota de voz, o una captura o el
    /// video de la clase: se busca su nota) en `ms`, o la cambia si ya
    /// estaba abierta.
    static func abrir(chat: String, mensaje: String, ms: Int64, almacen: Almacen) {
        let nucleo = almacen.nucleo
        _ = Task<Void, Never> {
            let leida = await Task.detached(priority: .userInitiated) { () -> Result<ClaseParaEstudiar, Error> in
                Result { try nucleo.claseParaEstudiar(chat: chat, mensaje: mensaje) }
            }.value
            guard case let .success(clase) = leida else { return }
            mostrar(clase, ms: ms, almacen: almacen)
        }
    }

    private static func mostrar(_ clase: ClaseParaEstudiar, ms: Int64, almacen: Almacen) {
        // Dos sonidos a la vez no se entienden.
        if ReproductorVoz.compartido.sonando { ReproductorVoz.compartido.pausar() }
        if let modelo {
            modelo.cargar(clase, ms: ms)
            return
        }
        let m = ModeloEstudiar(almacen: almacen, clase: clase, ms: ms)
        modelo = m
        ArribaUIKit.presentar(estilo: .fullScreen) { cerrar in
            NavigationStack {
                VistaEstudiarClase(m: m, d: m.desplazar, cerrar: cerrar)
                    .navigationTitle("Estudiar la clase")
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Cerrar", action: cerrar)
                        }
                        ToolbarItem(placement: .principal) { TituloDeLaClase(m: m) }
                    }
            }
            .onDisappear {
                m.cerrar()
                if modelo === m { modelo = nil }
            }
        }
    }
}

/// El titulo de arriba y, debajo, el nombre de la clase (el `subtitle` de
/// la ventana del Mac).
private struct TituloDeLaClase: View {
    @ObservedObject var m: ModeloEstudiar

    var body: some View {
        VStack(spacing: 0) {
            Text("Estudiar la clase").font(.headline)
            Text(m.clase.nombre).font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
    }
}

/// Donde esta cada lista: aparte del modelo, para que deslizarlas solo
/// repinte las listas.
@MainActor
final class DesplazamientoDeClase: ObservableObject {
    @Published var texto = ScrollPosition(idType: Int.self)
    @Published var fotos = ScrollPosition(idType: Int.self)
}

/// **Todo lo de la pantalla abierta** (la `Vista` de Windows).
@MainActor
final class ModeloEstudiar: ObservableObject {
    /// Lo que mide la tira de capturas (`ANCHO_FOTOS`).
    static let anchoFotos: CGFloat = 236
    /// Lo que mide una fila de la tira: la imagen a 16:9, su minuto y aire.
    static let filaFoto: CGFloat = 8 + (anchoFotos - 16) * 9 / 16 + 24
    /// Lo que saltan ← → y J/L (y los botones de los lados).
    static let saltoCorto: Int64 = 5_000
    static let saltoLargo: Int64 = 10_000
    /// Lo que se deja de seguir al sonido tras mover una lista a mano.
    static let aMano: TimeInterval = 5
    /// Un toque en la barra a menos de esto de una senal se queda en ella
    /// (algo mas que con raton: es un dedo).
    static let pegar: Float = 10

    /// Si en esta fase del scroll es el usuario quien mueve la lista (no
    /// una animacion ni el scroll que sigue al sonido).
    nonisolated static func esAMano(_ fase: ScrollPhase) -> Bool {
        switch fase {
        case .tracking, .interacting, .decelerating: true
        default: false
        }
    }

    let almacen: Almacen
    let desplazar = DesplazamientoDeClase()

    @Published private(set) var clase: ClaseParaEstudiar
    @Published private(set) var jugador: AVPlayer?
    @Published private(set) var esVideo = false
    @Published private(set) var sonando = false
    @Published private(set) var velocidad: Float = 1
    @Published private(set) var posicionMs: Int64 = 0
    @Published private(set) var seguir = true
    /// Ensenar la captura del minuto aunque haya video.
    @Published var verCapturas = false
    /// El marcador que se esta escribiendo: su minuto (su nota, en `nota`).
    @Published private(set) var escribiendo: Int64?
    @Published var nota = ""
    @Published private(set) var aviso: String?
    @Published private(set) var miniaturas: [String: UIImage] = [:]
    @Published private(set) var grande: (ruta: String, imagen: UIImage)?

    /// Lo de las listas que no se pinta: por donde van, lo que se ve de
    /// ellas y lo alto de cada fila del texto.
    var scrollTexto: CGFloat = 0
    var scrollFotos: CGFloat = 0
    var vistaTexto: CGFloat = 0
    var vistaFotos: CGFloat = 0
    var altosTexto: [Int: CGFloat] = [:]
    /// Se esta moviendo una lista con el dedo.
    var tocandoTexto = false
    var tocandoFotos = false

    /// Hasta cuando no se sigue al sonido: se movio una lista a mano.
    private var aManoHasta: Date?
    /// Ir a lo que suena en la siguiente vuelta, aunque este parado (tras
    /// un salto).
    private var irYa = true
    /// A donde ir en cuanto el medio este listo (la carga es asincrona).
    private var irPendiente: Int64?
    private var reloj: Timer?
    private var pedida: String?
    private var leido: Int64 = 0
    private var mirado = Date()
    private var turnoAviso = 0

    init(almacen: Almacen, clase: ClaseParaEstudiar, ms: Int64) {
        self.almacen = almacen
        self.clase = clase
        cargar(clase, ms: ms)
        reloj = Timer.scheduledTimer(withTimeInterval: 1.0 / 30, repeats: true) { [weak self] t in
            MainActor.assumeIsolated {
                guard let self else { return t.invalidate() }
                self.latido()
            }
        }
    }

    // MARK: La clase

    /// Pone `c` en la pantalla, desde `ms` (parado).
    func cargar(_ c: ClaseParaEstudiar, ms: Int64) {
        jugador?.pause()
        clase = c
        escribiendo = nil
        verCapturas = false
        altosTexto = [:]
        grande = nil
        pedida = nil
        leido = almacen.nucleo.cambioDelCuadernoMs(chat: c.chat)
        abrirElMedio(video: true)
        cargarMiniaturas()
        irA(ms)
    }

    /// El video (con su sonido) o, si no hay o no se puede ver, la nota.
    private func abrirElMedio(video: Bool) {
        sonando = false
        if video, let v = clase.video, FileManager.default.fileExists(atPath: v) {
            jugador = AVPlayer(url: URL(fileURLWithPath: v))
            esVideo = true
        } else if let a = clase.audio {
            jugador = AVPlayer(url: URL(fileURLWithPath: a))
            esVideo = false
        } else {
            jugador = nil
            esVideo = false
        }
    }

    /// Las miniaturas de la tira, fuera del hilo principal.
    private func cargarMiniaturas() {
        let rutas = clase.capturas.map(\.ruta).filter { miniaturas[$0] == nil }
        guard !rutas.isEmpty else { return }
        _ = Task<Void, Never> {
            let leidas = await Task.detached(priority: .userInitiated) { () -> [(String, UIImage)] in
                rutas.compactMap { r in ModeloEstudiar.miniatura(r, lado: 480).map { (r, $0) } }
            }.value
            for (r, i) in leidas { miniaturas[r] = i }
        }
    }

    /// Una imagen reducida a `lado` pixeles como mucho (sin leerla entera).
    nonisolated static func miniatura(_ ruta: String, lado: Int) -> UIImage? {
        guard let fuente = CGImageSourceCreateWithURL(URL(fileURLWithPath: ruta) as CFURL, nil) else { return nil }
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: lado,
        ]
        guard let cg = CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary) else { return nil }
        return UIImage(cgImage: cg)
    }

    /// Lo que dura: lo que dice el medio, o lo que dice la nota.
    var duracionMs: Int64 {
        if let d = jugador?.currentItem?.duration.seconds, d.isFinite, d > 0 { return Int64(d * 1000) }
        return clase.duracionMs
    }

    var tiemposCapturas: [Int64] { clase.capturas.map(\.ms) }
    var tiemposFilas: [Int64] { clase.filas.map(\.ms) }
    var altosFotos: [Float] { Array(repeating: Float(ModeloEstudiar.filaFoto), count: clase.capturas.count) }

    func altosDelTexto() -> [Float] {
        (0..<clase.filas.count).map { Float(altosTexto[$0] ?? 30) }
    }

    /// La captura que se ensena en grande, si se ensena alguna.
    var capturaALaVista: Int? {
        if esVideo, !verCapturas { return nil }
        return claseUltimoAntes(tiempos: tiemposCapturas, ms: posicionMs).map(Int.init)
    }

    /// La linea del texto que suena.
    var lineaActual: Int? {
        claseLineaEn(lineas: clase.lineas.map(\.ms), ms: posicionMs).map(Int.init)
    }

    // MARK: Lo que suena

    private func posicionDelJugador() -> Int64 {
        guard let t = jugador?.currentTime().seconds, t.isFinite else { return 0 }
        return max(Int64(t * 1000), 0)
    }

    /// Por donde va de verdad: a donde se va si aun carga, o lo que suena.
    private var ahoraMs: Int64 { irPendiente ?? posicionDelJugador() }

    /// Reproduce o para. Al final, otra vez desde el principio.
    func alternar() {
        guard let j = jugador else { return }
        if sonando {
            j.pause()
            sonando = false
            return
        }
        // Que suene aunque el iPad este en silencio, como una nota de voz.
        let sesion = AVAudioSession.sharedInstance()
        try? sesion.setCategory(.playback, mode: esVideo ? .moviePlayback : .spokenAudio)
        try? sesion.setActive(true)
        if duracionMs > 0, ahoraMs >= duracionMs - 100 { irA(0) }
        j.playImmediately(atRate: velocidad)
        sonando = true
    }

    /// Va a `ms`, y el texto y la tira van con el. Mientras el medio carga,
    /// se va en cuanto se pueda.
    func irA(_ ms: Int64) {
        let ms = claseSaltar(ms: ms, delta: 0, duracionMs: duracionMs)
        if jugador?.currentItem?.status == .readyToPlay {
            jugador?.seek(to: CMTime(value: ms, timescale: 1000), toleranceBefore: .zero, toleranceAfter: .zero)
            irPendiente = nil
        } else {
            irPendiente = ms
        }
        posicionMs = ms
        aManoHasta = nil
        irYa = true
    }

    func saltar(_ delta: Int64) {
        irA(claseSaltar(ms: ahoraMs, delta: delta, duracionMs: duracionMs))
    }

    /// El trozo anterior o el siguiente al que suena.
    func otroTrozo(adelante: Bool) {
        let tiempos = clase.lineas.compactMap(\.ms)
        if let t = claseOtroTrozo(tiempos: tiempos, ms: ahoraMs, adelante: adelante) { irA(t) }
    }

    /// La siguiente de las cinco velocidades del movil.
    func otraVelocidad() {
        velocidad = ReproductorVoz.siguienteVelocidad(velocidad)
        if sonando { jugador?.rate = velocidad }
    }

    func alternarSeguir() {
        seguir.toggle()
        if seguir {
            aManoHasta = nil
            irYa = true
        }
    }

    /// Un toque en una captura de la tira: a su minuto, y en grande.
    func verCaptura(_ i: Int) {
        guard clase.capturas.indices.contains(i) else { return }
        verCapturas = true
        irA(clase.capturas[i].ms)
    }

    // MARK: Cada vuelta

    private func latido() {
        releerSiCambio()
        guard let j = jugador else { return }
        // El video no se deja ver aqui: suena la nota.
        if esVideo, j.currentItem?.status == .failed {
            abrirElMedio(video: false)
            avisar("Este vídeo no se puede ver aquí; suena la nota de voz")
            irA(posicionMs)
            return
        }
        if let ms = irPendiente, j.currentItem?.status == .readyToPlay {
            irPendiente = nil
            j.seek(to: CMTime(value: ms, timescale: 1000), toleranceBefore: .zero, toleranceAfter: .zero)
        }
        let suena = j.rate != 0
        if suena != sonando { sonando = suena }
        // Mientras carga, se ensena a donde se va.
        let p = ahoraMs
        // Lo que se pinta va a saltos de 0,1 s: de sobra para la barra y el
        // resaltado, y la pantalla no se repinta entera en cada vuelta.
        if abs(p - posicionMs) >= 100 || (!sonando && p != posicionMs) { posicionMs = p }
        pedirLaGrande()
        seguirAlSonido()
    }

    /// La imagen grande de la captura del minuto, cuando cambia.
    private func pedirLaGrande() {
        guard let i = capturaALaVista, clase.capturas.indices.contains(i) else { return }
        let ruta = clase.capturas[i].ruta
        guard grande?.ruta != ruta, pedida != ruta else { return }
        pedida = ruta
        _ = Task<Void, Never> {
            // Reducida al doble de la pantalla: una captura de un monitor
            // grande no hace falta entera.
            let img = await Task.detached(priority: .userInitiated) {
                ModeloEstudiar.miniatura(ruta, lado: 2400)
            }.value
            guard pedida == ruta else { return }
            pedida = nil
            if let img { grande = (ruta: ruta, imagen: img) }
        }
    }

    /// Con «Seguir», el texto y la tira se deslizan hasta lo que suena
    /// (mientras suena, o justo despues de un salto).
    private func seguirAlSonido() {
        if let h = aManoHasta, Date() < h { return }
        aManoHasta = nil
        guard seguir else {
            irYa = false
            return
        }
        guard irYa || sonando, vistaTexto > 0 || vistaFotos > 0 else { return }
        let ms = ahoraMs
        let destinoTexto = claseScrollPara(tiempos: tiemposFilas, altos: altosDelTexto(), altoVista: Float(vistaTexto),
                                           ms: ms, suave: false)
        let destinoFotos = claseScrollPara(tiempos: tiemposCapturas, altos: altosFotos, altoVista: Float(vistaFotos),
                                           ms: ms, suave: false)
        let t = CGFloat(claseAcercar(actual: Float(scrollTexto), destino: destinoTexto, k: 0.25))
        let f = CGFloat(claseAcercar(actual: Float(scrollFotos), destino: destinoFotos, k: 0.25))
        var cambio = false
        if abs(t - scrollTexto) > 0.5, !clase.filas.isEmpty {
            scrollTexto = t
            desplazar.texto.scrollTo(y: t)
            cambio = true
        }
        if abs(f - scrollFotos) > 0.5, !clase.capturas.isEmpty {
            scrollFotos = f
            desplazar.fotos.scrollTo(y: f)
            cambio = true
        }
        if !cambio { irYa = false }
    }

    /// **Se movio una lista a mano**: la otra la sigue (con «Seguir») y se
    /// deja de seguir al sonido un rato.
    func movida(enTexto: Bool) {
        aManoHasta = Date().addingTimeInterval(ModeloEstudiar.aMano)
        guard seguir else { return }
        if enTexto {
            if let s = claseSeguir(deTiempos: tiemposFilas, deAltos: altosDelTexto(), deVista: Float(vistaTexto),
                                   deScroll: Float(scrollTexto), aTiempos: tiemposCapturas, aAltos: altosFotos,
                                   aVista: Float(vistaFotos)) {
                scrollFotos = CGFloat(s)
                desplazar.fotos.scrollTo(y: CGFloat(s))
            }
        } else if let s = claseSeguir(deTiempos: tiemposCapturas, deAltos: altosFotos, deVista: Float(vistaFotos),
                                      deScroll: Float(scrollFotos), aTiempos: tiemposFilas,
                                      aAltos: altosDelTexto(), aVista: Float(vistaTexto)) {
            scrollTexto = CGFloat(s)
            desplazar.texto.scrollTo(y: CGFloat(s))
        }
    }

    /// Si el cuaderno cambio por fuera (llego el texto, un marcador del
    /// movil), se relee lo que no suena: el texto, las senales y las
    /// capturas nuevas.
    private func releerSiCambio() {
        guard escribiendo == nil, Date().timeIntervalSince(mirado) >= 1.5 else { return }
        mirado = Date()
        let chat = clase.chat
        let ahora = almacen.nucleo.cambioDelCuadernoMs(chat: chat)
        guard ahora != leido else { return }
        leido = ahora
        let id = clase.id
        let nucleo = almacen.nucleo
        _ = Task<Void, Never> {
            let leida = await Task.detached(priority: .utility) {
                try? nucleo.claseParaEstudiar(chat: chat, mensaje: id)
            }.value
            if let leida, leida.id == clase.id { aplicar(leida) }
        }
    }

    /// Lo leido otra vez, sin tocar lo que suena.
    private func aplicar(_ nueva: ClaseParaEstudiar) {
        let otrasCapturas = nueva.capturas != clase.capturas
        if nueva.filas != clase.filas { altosTexto = [:] }
        clase = nueva
        if otrasCapturas {
            grande = nil
            pedida = nil
            cargarMiniaturas()
        }
    }

    // MARK: Los marcadores

    func empezarMarcador() {
        let ms = ahoraMs
        // Al escribir, el sonido se para: se esta tomando una nota.
        if sonando { alternar() }
        nota = clase.senales.first { $0.ms == ms && !$0.banderita }?.texto ?? ""
        escribiendo = ms
    }

    /// «Marcador» con uno a medio escribir lo guarda.
    func pulsarMarcador() {
        if escribiendo != nil { guardarMarcador() } else { empezarMarcador() }
    }

    func cancelarMarcador() {
        escribiendo = nil
    }

    func guardarMarcador() {
        guard let ms = escribiendo else { return }
        escribiendo = nil
        let (chat, voz, texto) = (clase.chat, clase.id, nota)
        let nucleo = almacen.nucleo
        _ = Task<Void, Never> {
            let hecho = await Task.detached(priority: .userInitiated) {
                try? nucleo.ponerMarcadorDeClase(chat: chat, voz: voz, ms: ms, texto: texto)
            }.value
            if let hecho {
                leido = almacen.nucleo.cambioDelCuadernoMs(chat: chat)
                aplicar(hecho)
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                avisar("Marcador guardado")
            } else {
                avisar("No se pudo guardar")
            }
        }
    }

    /// Quita la senal de `ms`: la banderita del movil o el marcador.
    func quitarSenal(_ ms: Int64) {
        let (chat, voz) = (clase.chat, clase.id)
        let nucleo = almacen.nucleo
        _ = Task<Void, Never> {
            let hecho = await Task.detached(priority: .userInitiated) {
                try? nucleo.quitarSenalDeClase(chat: chat, voz: voz, ms: ms)
            }.value
            if let hecho {
                leido = almacen.nucleo.cambioDelCuadernoMs(chat: chat)
                aplicar(hecho)
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            } else {
                avisar("No se pudo guardar")
            }
        }
    }

    // MARK: Avisos, teclas y cerrar

    func avisar(_ texto: String) {
        aviso = texto
        turnoAviso += 1
        let turno = turnoAviso
        _ = Task<Void, Never> {
            try? await Task.sleep(nanoseconds: 2_200_000_000)
            if turno == turnoAviso { aviso = nil }
        }
    }

    /// Las teclas de Windows, con el teclado del iPad. Escribiendo la nota
    /// de un marcador las coge el campo (⎋ lo deja).
    func tecla(_ k: KeyPress, cerrar: () -> Void) -> Bool {
        guard k.phase != .up else { return false }
        if escribiendo != nil {
            if k.key == .escape {
                cancelarMarcador()
                return true
            }
            return false
        }
        guard k.modifiers.subtracting([.capsLock, .numericPad, .function]).isEmpty else { return false }
        switch k.key {
        case .space: alternar()
        case .leftArrow: saltar(-ModeloEstudiar.saltoCorto)
        case .rightArrow: saltar(ModeloEstudiar.saltoCorto)
        case .upArrow: otroTrozo(adelante: false)
        case .downArrow: otroTrozo(adelante: true)
        case .escape: cerrar()
        default:
            // Solo al pulsar (no al repetir) las letras.
            guard k.phase == .down else { return false }
            switch k.characters.lowercased() {
            case "k": alternar()
            case "j": saltar(-ModeloEstudiar.saltoLargo)
            case "l": saltar(ModeloEstudiar.saltoLargo)
            case "m": empezarMarcador()
            case "s": alternarSeguir()
            default: return false
            }
        }
        return true
    }

    func cerrar() {
        reloj?.invalidate()
        reloj = nil
        jugador?.pause()
        sonando = false
    }
}

// MARK: - Las vistas

/// El ambar de los marcadores (`AMBAR`).
private let ambar = Color(red: 0xF2 / 255, green: 0xA3 / 255, blue: 0x3A / 255)
/// El fondo de lo que se ve y de cada captura.
private let fondoOscuro = Color(red: 0x0B / 255, green: 0x0F / 255, blue: 0x14 / 255)
/// El fondo de las columnas.
private let fondoLista = Color.primary.opacity(0.05)

struct VistaEstudiarClase: View {
    @ObservedObject var m: ModeloEstudiar
    let d: DesplazamientoDeClase
    let cerrar: () -> Void
    @FocusState private var foco: Bool

    var body: some View {
        GeometryReader { geo in
            if geo.size.width >= 900 {
                // El texto, entre 320 y 460 (un 30 % de la pantalla).
                let anchoTexto = min(max(geo.size.width * 0.30, 320), 460)
                HStack(alignment: .top, spacing: 16) {
                    ColumnaDelMedio(m: m)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    ColumnaDelTexto(m: m, d: d)
                        .frame(width: anchoTexto)
                    ColumnaDeFotos(m: m, d: d)
                        .frame(width: ModeloEstudiar.anchoFotos)
                }
                .padding(16)
            } else {
                // Estrecho: lo que se ve arriba; el texto y la tira debajo.
                VStack(spacing: 12) {
                    ColumnaDelMedio(m: m)
                        .frame(height: min(geo.size.height * 0.55, geo.size.width * 9 / 16 + 300))
                    HStack(alignment: .top, spacing: 12) {
                        ColumnaDelTexto(m: m, d: d)
                            .frame(maxWidth: .infinity)
                        ColumnaDeFotos(m: m, d: d)
                            .frame(width: min(ModeloEstudiar.anchoFotos, geo.size.width * 0.38))
                    }
                }
                .padding(12)
            }
        }
        .background(Color(uiColor: .systemBackground))
        .overlay(alignment: .bottom) {
            if let a = m.aviso { AvisoDelChat(texto: a).padding(.bottom, 12) }
        }
        .animation(.easeOut(duration: 0.15), value: m.aviso)
        .tecladoDeVentanita($foco) { k in m.tecla(k, cerrar: cerrar) }
        // Guardado o dejado el marcador, las teclas vuelven aqui.
        .onChange(of: m.escribiendo) { _, ahora in
            if ahora == nil { foco = true }
        }
    }
}

/// Lo que se ve, la barra del tiempo, los mandos y los marcadores.
private struct ColumnaDelMedio: View {
    @ObservedObject var m: ModeloEstudiar

    var body: some View {
        GeometryReader { geo in
            // A 16:9, dejando sitio debajo para la barra, los mandos y algo
            // de la lista de marcadores.
            let alto = max(min(geo.size.width * 9 / 16, geo.size.height - 230), 120)
            VStack(spacing: 8) {
                medio.frame(height: alto)
                BarraDelTiempo(m: m).frame(height: 32)
                mandos.frame(height: 44)
                ListaDeSenales(m: m)
                    .frame(maxHeight: .infinity)
                    .padding(.top, 4)
            }
        }
    }

    // MARK: Lo que se ve

    private var medio: some View {
        ZStack(alignment: .topTrailing) {
            RoundedRectangle(cornerRadius: 10).fill(fondoOscuro)
            contenido
                .clipShape(RoundedRectangle(cornerRadius: 10))
                .contentShape(Rectangle())
                .onTapGesture { m.alternar() }
            // Con video y capturas, una ficha cambia entre los dos.
            if m.esVideo, !m.clase.capturas.isEmpty {
                Button { m.verCapturas.toggle() } label: {
                    Label(m.verCapturas ? "Ver el vídeo" : "Ver la captura",
                          systemImage: m.verCapturas ? "video" : "photo")
                        .font(.system(size: 13))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 12)
                        .frame(height: 32)
                        .background(Color.black.opacity(0.65), in: Capsule())
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .padding(10)
            }
        }
    }

    @ViewBuilder private var contenido: some View {
        if let i = m.capturaALaVista, m.clase.capturas.indices.contains(i) {
            let ruta = m.clase.capturas[i].ruta
            if let g = m.grande, g.ruta == ruta {
                Image(uiImage: g.imagen).resizable().scaledToFit()
            } else if let mini = m.miniaturas[ruta] {
                Image(uiImage: mini).resizable().scaledToFit()
            } else {
                vacio("photo", "Cargando…")
            }
        } else if m.esVideo, let j = m.jugador {
            VideoDeLaClase(jugador: j)
        } else if m.jugador == nil {
            vacio("mic", "No se encuentra el audio de esta clase")
        } else {
            vacio("mic", "Solo audio")
        }
    }

    private func vacio(_ simbolo: String, _ texto: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: simbolo).font(.system(size: 34)).foregroundStyle(Color.white.opacity(0.6))
            Text(texto).font(.system(size: 15)).foregroundStyle(Color.white.opacity(0.7))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: Los mandos

    private var mandos: some View {
        HStack(spacing: 6) {
            boton("gobackward.10", "", encendido: false) { m.saltar(-ModeloEstudiar.saltoLargo) }
                .frame(width: 44)
                .accessibilityLabel("Atrás 10 s")
            boton(m.sonando ? "pause.fill" : "play.fill", "", encendido: true) { m.alternar() }
                .frame(width: 52)
                .accessibilityLabel(m.sonando ? "Pausa" : "Reproducir")
            boton("goforward.10", "", encendido: false) { m.saltar(ModeloEstudiar.saltoLargo) }
                .frame(width: 44)
                .accessibilityLabel("Adelante 10 s")
            // 12:34 / 45:00
            Text("\(FormatoVoz.marca(ms: m.posicionMs)) / \(FormatoVoz.marca(ms: m.duracionMs))")
                .font(.system(size: 14).monospacedDigit())
                .padding(.leading, 8)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
            Spacer(minLength: 8)
            Button { m.otraVelocidad() } label: {
                Text(VozAlta.rotulo(m.velocidad))
                    .font(.system(size: 14).monospacedDigit())
                    .padding(.horizontal, 12)
                    .frame(height: 40)
                    .background(fondoLista, in: RoundedRectangle(cornerRadius: 10))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Velocidad")
            boton("bookmark", "Marcador", encendido: m.escribiendo != nil) { m.pulsarMarcador() }
                .fixedSize()
            boton("link", "Seguir", encendido: m.seguir) { m.alternarSeguir() }
                .fixedSize()
                .accessibilityHint("El texto y las capturas siguen a lo que suena")
        }
    }

    /// Un boton redondeado de los mandos: icono y, si lo hay, rotulo.
    private func boton(_ simbolo: String, _ rotulo: String, encendido: Bool,
                       _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            HStack(spacing: 6) {
                Image(systemName: simbolo).font(.system(size: 15, weight: .semibold))
                if !rotulo.isEmpty { Text(rotulo).font(.system(size: 14)).lineLimit(1) }
            }
            .foregroundStyle(encendido ? Color.white : Color.primary)
            .padding(.horizontal, rotulo.isEmpty ? 0 : 12)
            .frame(maxWidth: rotulo.isEmpty ? .infinity : nil)
            .frame(height: 40)
            .background(encendido ? Color.accentColor : fondoLista, in: RoundedRectangle(cornerRadius: 10))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
    }
}

/// El video de la clase, sin mandos propios (los de abajo mandan; un toque
/// reproduce y para).
private struct VideoDeLaClase: UIViewRepresentable {
    let jugador: AVPlayer

    final class Lienzo: UIView {
        override static var layerClass: AnyClass { AVPlayerLayer.self }
        var capa: AVPlayerLayer { layer as! AVPlayerLayer }
    }

    func makeUIView(context: Context) -> Lienzo {
        let v = Lienzo()
        v.backgroundColor = .clear
        v.capa.videoGravity = .resizeAspect
        v.capa.player = jugador
        return v
    }

    func updateUIView(_ v: Lienzo, context: Context) {
        if v.capa.player !== jugador { v.capa.player = jugador }
    }
}

/// **La barra del tiempo**: lo oido, una raya por captura, una banderita por
/// senal y el tirador. Tocar (o arrastrar) lleva ahi; cerca de una senal o
/// una captura, se queda en ella.
private struct BarraDelTiempo: View {
    @ObservedObject var m: ModeloEstudiar
    @State private var arrastrando = false

    var body: some View {
        GeometryReader { geo in
            let ancho = Float(geo.size.width)
            let duracion = m.duracionMs
            let x = { (ms: Int64) -> CGFloat in CGFloat(claseXEnLaBarra(ms: ms, ancho: ancho, duracionMs: duracion)) }
            let alto = geo.size.height
            let xp = x(m.posicionMs)
            ZStack(alignment: .topLeading) {
                Capsule().fill(Color.secondary.opacity(0.35))
                    .frame(height: 6)
                    .offset(y: alto / 2 - 3)
                Capsule().fill(Color.accentColor)
                    .frame(width: max(xp, 0), height: 6)
                    .offset(y: alto / 2 - 3)
                ForEach(Array(m.clase.capturas.enumerated()), id: \.offset) { _, c in
                    Rectangle().fill(Color.primary.opacity(0.7))
                        .frame(width: 2, height: alto - 8)
                        .offset(x: x(c.ms) - 1, y: 4)
                }
                ForEach(Array(m.clase.senales.enumerated()), id: \.offset) { _, s in
                    Rectangle().fill(ambar)
                        .frame(width: 2, height: alto / 2)
                        .offset(x: x(s.ms) - 1)
                    Circle().fill(ambar)
                        .frame(width: 8, height: 8)
                        .offset(x: x(s.ms) - 4, y: -1)
                }
                let lado: CGFloat = arrastrando ? 18 : 14
                Circle().fill(Color.accentColor)
                    .frame(width: lado, height: lado)
                    .offset(x: xp - lado / 2, y: alto / 2 - lado / 2)
            }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { g in
                    arrastrando = true
                    let senales = m.clase.capturas.map(\.ms) + m.clase.senales.map(\.ms)
                    m.irA(clasePegarASenal(x: Float(g.location.x), ancho: ancho, duracionMs: duracion,
                                           senales: senales, tolerancia: ModeloEstudiar.pegar))
                }
                .onEnded { _ in arrastrando = false })
            .accessibilityElement()
            .accessibilityLabel("Posición")
            .accessibilityValue(FormatoVoz.marca(ms: m.posicionMs))
            .accessibilityAdjustableAction { dir in
                switch dir {
                case .increment: m.saltar(ModeloEstudiar.saltoCorto)
                case .decrement: m.saltar(-ModeloEstudiar.saltoCorto)
                @unknown default: break
                }
            }
        }
    }
}

/// **Los marcadores**: el que se esta escribiendo arriba y, debajo, los que
/// hay, cada uno con su minuto.
private struct ListaDeSenales: View {
    @ObservedObject var m: ModeloEstudiar
    @FocusState private var enfocado: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Marcadores · \(m.clase.senales.count)").font(.system(size: 15, weight: .medium))
                Spacer()
            }
            if let ms = m.escribiendo {
                HStack(spacing: 10) {
                    Text(FormatoVoz.marca(ms: ms)).font(.system(size: 14).monospacedDigit()).foregroundStyle(ambar)
                    TextField("", text: $m.nota, prompt: Text("Escribe una nota y pulsa Intro (Esc cancela)"))
                        .textFieldStyle(.plain)
                        .font(.system(size: 15))
                        .focused($enfocado)
                        .submitLabel(.done)
                        .onSubmit { m.guardarMarcador() }
                        .onAppear { enfocado = true }
                    Button { m.cancelarMarcador() } label: {
                        Image(systemName: "xmark").font(.system(size: 12, weight: .semibold)).frame(width: 30, height: 30)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .accessibilityLabel("Cancelar")
                }
                .padding(.horizontal, 10)
                .frame(height: 40)
                .background(ambar.opacity(0.18), in: RoundedRectangle(cornerRadius: 8))
            }
            if m.clase.senales.isEmpty, m.escribiendo == nil {
                Text("Pulsa «Marcador» o la M para apuntar algo en este minuto.")
                    .font(.system(size: 14)).foregroundStyle(.secondary)
            }
            ScrollView {
                let actual = m.clase.senales.lastIndex { $0.ms <= m.posicionMs }
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(m.clase.senales.enumerated()), id: \.offset) { k, s in
                        fila(s, actual: actual == k)
                    }
                }
            }
        }
        .padding(12)
        .background(fondoLista, in: RoundedRectangle(cornerRadius: 10))
    }

    private func fila(_ s: SenalDeClase, actual: Bool) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "flag.fill").font(.system(size: 12)).foregroundStyle(ambar)
            Text(FormatoVoz.marca(ms: s.ms)).font(.system(size: 13).monospacedDigit()).foregroundStyle(ambar)
                .frame(width: 50, alignment: .leading)
            Text(s.texto.isEmpty ? "Marcador" : s.texto)
                .font(.system(size: 14))
                .foregroundStyle(s.texto.isEmpty ? Color.secondary : Color.primary)
                .lineLimit(1)
            Spacer(minLength: 4)
            // Sin raton no hay «encima»: la x va siempre, discreta.
            Button { m.quitarSenal(s.ms) } label: {
                Image(systemName: "xmark").font(.system(size: 11, weight: .semibold)).frame(width: 30, height: 30)
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
            .accessibilityLabel("Quitar")
        }
        .padding(.horizontal, 8)
        .frame(height: 36)
        .background(actual ? ambar.opacity(0.12) : Color.clear, in: RoundedRectangle(cornerRadius: 8))
        .contentShape(Rectangle())
        .onTapGesture { m.irA(s.ms) }
        .hoverEffect(.highlight)
        .contextMenu {
            Button(role: .destructive) { m.quitarSenal(s.ms) } label: { Label("Quitar", systemImage: "trash") }
        }
    }
}

/// **El texto** con su minuto delante de cada trozo y los marcadores en su
/// sitio.
private struct ColumnaDelTexto: View {
    @ObservedObject var m: ModeloEstudiar
    @ObservedObject var d: DesplazamientoDeClase

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Label("Transcripción", systemImage: "text.quote")
                .font(.system(size: 15, weight: .medium))
                .padding(.horizontal, 12)
                .frame(height: 40)
            if m.clase.lineas.isEmpty, m.clase.senales.isEmpty {
                Text("Aún no hay transcripción: pulsa «Pasar a texto» en la nota del chat y aparecerá aquí sola.")
                    .font(.system(size: 14)).foregroundStyle(.secondary)
                    .padding(14)
                Spacer(minLength: 0)
            } else {
                lista
            }
        }
        .background(fondoLista, in: RoundedRectangle(cornerRadius: 10))
    }

    private var lista: some View {
        let actual = m.lineaActual
        return ScrollView {
            // Sin huecos entre filas: lo alto de cada una es lo que mide (lo
            // necesitan las cuentas del scroll que se sigue).
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(m.clase.filas.enumerated()), id: \.offset) { i, f in
                    fila(f, actual: actual)
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { m.altosTexto[i] = $0 }
                }
            }
            .padding(.horizontal, 6)
        }
        .scrollPosition($d.texto)
        .onScrollGeometryChange(for: CGRect.self) { g in
            CGRect(origin: g.contentOffset, size: g.containerSize)
        } action: { _, nuevo in
            m.vistaTexto = nuevo.height
            m.scrollTexto = nuevo.minY
            if m.tocandoTexto { m.movida(enTexto: true) }
        }
        .onScrollPhaseChange { _, fase in
            m.tocandoTexto = ModeloEstudiar.esAMano(fase)
        }
    }

    @ViewBuilder private func fila(_ f: FilaDeClase, actual: Int?) -> some View {
        if let i = f.linea.map(Int.init), m.clase.lineas.indices.contains(i) {
            linea(m.clase.lineas[i], esActual: actual == i)
        } else if let j = f.senal.map(Int.init), m.clase.senales.indices.contains(j) {
            marcador(m.clase.senales[j])
        }
    }

    private func linea(_ l: LineaDeClase, esActual: Bool) -> some View {
        // Lo que aun no se ha oido, un poco apagado.
        let futuro = m.sonando && (l.ms.map { $0 > m.posicionMs } ?? false)
        return HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(l.ms.map { FormatoVoz.marca(ms: $0) } ?? "")
                .font(.system(size: 12.5).monospacedDigit())
                .foregroundStyle(esActual ? Color.accentColor : Color.secondary)
                .frame(width: 46, alignment: .leading)
            Text(l.texto)
                .font(.system(size: 16))
                .foregroundStyle(Color.primary.opacity(futuro ? 0.6 : 1))
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.vertical, 7)
        .padding(.horizontal, 10)
        .background(alignment: .leading) {
            if esActual {
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 8).fill(Color.accentColor.opacity(0.16))
                    RoundedRectangle(cornerRadius: 1.5).fill(Color.accentColor).frame(width: 3)
                }
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { if let ms = l.ms { m.irA(ms) } }
    }

    private func marcador(_ s: SenalDeClase) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "flag.fill").font(.system(size: 12)).foregroundStyle(ambar)
                .frame(width: 46, alignment: .leading)
            Text("\(FormatoVoz.marca(ms: s.ms)) · \(s.texto.isEmpty ? "Marcador" : s.texto)")
                .font(.system(size: 14))
                .lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 10)
        .frame(height: 30)
        .background(ambar.opacity(0.16), in: RoundedRectangle(cornerRadius: 8))
        .padding(.vertical, 3)
        .contentShape(Rectangle())
        .onTapGesture { m.irA(s.ms) }
    }
}

/// **La tira de capturas**, con su minuto.
private struct ColumnaDeFotos: View {
    @ObservedObject var m: ModeloEstudiar
    @ObservedObject var d: DesplazamientoDeClase

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Label("Capturas · \(m.clase.capturas.count)", systemImage: "photo.on.rectangle")
                .font(.system(size: 15, weight: .medium))
                .padding(.horizontal, 12)
                .frame(height: 40)
                .lineLimit(1)
            if m.clase.capturas.isEmpty {
                Text("Esta clase no tiene capturas.")
                    .font(.system(size: 14)).foregroundStyle(.secondary)
                    .padding(14)
                Spacer(minLength: 0)
            } else {
                lista
            }
        }
        .background(fondoLista, in: RoundedRectangle(cornerRadius: 10))
    }

    private var lista: some View {
        let actual = claseUltimoAntes(tiempos: m.tiemposCapturas, ms: m.posicionMs).map(Int.init)
        return ScrollView {
            VStack(spacing: 0) {
                ForEach(Array(m.clase.capturas.enumerated()), id: \.offset) { i, c in
                    foto(c, esActual: actual == i)
                        .frame(height: ModeloEstudiar.filaFoto)
                        .contentShape(Rectangle())
                        .onTapGesture { m.verCaptura(i) }
                }
            }
        }
        .scrollPosition($d.fotos)
        .onScrollGeometryChange(for: CGRect.self) { g in
            CGRect(origin: g.contentOffset, size: g.containerSize)
        } action: { _, nuevo in
            m.vistaFotos = nuevo.height
            m.scrollFotos = nuevo.minY
            if m.tocandoFotos { m.movida(enTexto: false) }
        }
        .onScrollPhaseChange { _, fase in
            m.tocandoFotos = ModeloEstudiar.esAMano(fase)
        }
    }

    private func foto(_ c: CapturaParaEstudiar, esActual: Bool) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            ZStack {
                RoundedRectangle(cornerRadius: 6).fill(fondoOscuro)
                if let i = m.miniaturas[c.ruta] {
                    Image(uiImage: i).resizable().scaledToFit()
                }
            }
            .frame(height: ModeloEstudiar.filaFoto - 32)
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .padding(3)
            .background(esActual ? Color.accentColor : Color.clear, in: RoundedRectangle(cornerRadius: 9))
            Text("⏱ \(FormatoVoz.marca(ms: c.ms))")
                .font(.system(size: 13).monospacedDigit())
                .foregroundStyle(esActual ? Color.accentColor : Color.secondary)
                .padding(.leading, 4)
        }
        .padding(.horizontal, 5)
        .padding(.top, 5)
        .frame(maxHeight: .infinity, alignment: .top)
    }
}
