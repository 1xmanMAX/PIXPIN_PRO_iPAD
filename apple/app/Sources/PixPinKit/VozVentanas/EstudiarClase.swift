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
// **Como los visores** (Windows fe0c9c4, 25-oct-2026, el usuario: «el
// visualizador de las grabaciones deberia ser como el previsualizador, con
// la misma interfaz»; del Mac, `BarraVisor`): arriba la barra de los
// visores del iPad (`BarraVisorIPad`: el nombre, «Anotar», «Compartir», la
// luna/sol, que es la de todos los visores, y cerrar), y **claro u oscuro**
// como ellos (`ModoNoche`).
//
// **Acercar lo que se ve como en un lienzo** (`encuadre*` del nucleo, que
// es `pixpin_ui::encuadre` de Windows): el pellizco encima del video o de la
// captura acerca hacia los dedos, arrastrar la lleva, el doble toque la
// vuelve a encajar; abajo a la derecha, −, el tanto por ciento, + y encajar.
// Al cambiar de captura o de video a captura, vuelve a encajada.
//
// **Anotar encima** con el lienzo de siempre: «Anotar» abre la captura que
// se ve en SU lienzo (`foto-<id>`, el de las fotos del chat), encima de
// esta pantalla, y lo anotado se ve aqui encima de la captura. Con el
// video, «Anotar» lo para y guarda el fotograma que se ve como una captura
// mas de la clase, en su minuto (`Nucleo.guardarFotogramaDeClase`), y la
// abre para anotar.
//
// **Compartir** la grabacion con la hoja de toda la app
// (`CompartirGrabacion.swift`: su pagina web, PDF, Word, Markdown, Texto,
// Anki y los originales).
//
// Al arrastrar la barra del tiempo el tirador va con el dedo y el fotograma
// exacto se busca cada 70 ms (el aproximado caia en fotogramas clave
// lejanos).
//
// Con teclado: espacio o K reproduce y para, ← → cinco segundos, J/L diez,
// ↑ ↓ el trozo anterior y el siguiente, M pone un marcador (se escribe su
// nota e ↩ lo guarda; ⎋ lo deja), S enciende y apaga «Seguir», A anota,
// ⌘0 encaja, ⌘+ / ⌘− acercan y alejan; ⎋ primero encaja lo acercado, luego
// cierra.
//
// Los textos son los de Windows (`estudiar-*` de `main.ftl`).

import AVFoundation
import ImageIO
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

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
            // Como los visores: su barra arriba, sin la de navegacion. La
            // navegacion es suya (la barra la pide; «Abrir en otra ventana»
            // no va aqui).
            VStack(spacing: 0) {
                BarraEstudiar(m: m, cerrar: cerrar)
                VistaEstudiarClase(m: m, d: m.desplazar, cerrar: cerrar)
            }
            .environmentObject(Navegacion())
            .onDisappear {
                m.cerrar()
                if modelo === m { modelo = nil }
            }
        }
    }
}

/// **La barra de los visores** (`lector_pin` de Windows, `BarraVisor` del
/// Mac): el nombre de la clase, «Anotar», «Compartir», la luna/sol y cerrar.
private struct BarraEstudiar: View {
    @ObservedObject var m: ModeloEstudiar
    let cerrar: () -> Void
    @ObservedObject private var noche = ModoNoche.compartido

    var body: some View {
        BarraVisorIPad(tipo: "Estudiar la clase", icono: m.esVideo ? "video" : "mic", titulo: m.clase.nombre,
                       alCerrar: cerrar) {
            Button { m.anotar() } label: { Label("Anotar", systemImage: "pencil") }
                .help("Anotar (A)")
            Button { m.compartir() } label: { Image(systemName: "square.and.arrow.up") }
                .help("Compartir")
        }
        .environment(\.colorScheme, noche.activo ? .dark : .light)
        .background(Color(uiColor: .systemBackground))
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
    /// Como se ve lo de la izquierda: acercado y movido como en un lienzo.
    @Published private(set) var encuadre = encuadreEncajado()
    /// Lo que mide el video (pixeles), cuando ya se sabe.
    @Published private(set) var tamanoVideo = CGSize.zero
    /// Se esta arrastrando la barra del tiempo: el minuto bajo el dedo.
    @Published private(set) var arrastrando: Int64?
    /// La captura que se ve con lo anotado en su lienzo propio, ya pintada.
    @Published private(set) var anotada: (lienzo: String, imagen: CGImage)?

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

    /// Lo que mide la caja de lo que se ve (puntos).
    var cajaMedio = CGSize.zero
    /// Lo que se ve en grande ahora: al cambiar, se vuelve a encajar.
    private var visto: String?
    /// Cuando se busco el ultimo fotograma arrastrando la barra.
    private var buscadoAl = Date.distantPast
    /// El lienzo cuya anotacion se pidio (o ya se tiene).
    private var anotadaPedida: String?
    private var vigias: [NSObjectProtocol] = []

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
        // Se anoto la captura en su lienzo: se vuelve a leer lo anotado.
        vigias.append(NotificationCenter.default.addObserver(forName: .pixpinLienzoGuardado, object: nil,
                                                             queue: .main) { [weak self] n in
            let r = n.object as? String
            MainActor.assumeIsolated {
                if let self, let r, r == self.anotadaPedida { self.anotadaPedida = nil }
            }
        })
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
        anotada = nil
        anotadaPedida = nil
        visto = nil
        arrastrando = nil
        tamanoVideo = .zero
        encuadre = encuadreEncajado()
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

    // MARK: Acercar lo que se ve

    /// Lo que mide la imagen que se ve (pixeles): la captura grande, su
    /// miniatura mientras llega, o el video. `nil` si aun no se sabe.
    var medidaVista: CGSize? {
        if let i = capturaALaVista, clase.capturas.indices.contains(i) {
            let ruta = clase.capturas[i].ruta
            let img = grande?.ruta == ruta ? grande?.imagen : miniaturas[ruta]
            return img.flatMap(ModeloEstudiar.pixeles)
        }
        if esVideo, tamanoVideo.width > 0, tamanoVideo.height > 0 { return tamanoVideo }
        return nil
    }

    /// Los pixeles de una imagen (no sus puntos).
    nonisolated static func pixeles(_ i: UIImage) -> CGSize? {
        let t = CGSize(width: i.size.width * i.scale, height: i.size.height * i.scale)
        return t.width > 0 && t.height > 0 ? t : nil
    }

    private func caja(_ t: CGSize) -> CajaDeClase {
        CajaDeClase(x: 0, y: 0, ancho: Float(t.width), alto: Float(t.height))
    }

    /// **Donde se pinta lo que se ve** en una caja de `tam`; `nil` si aun
    /// no se sabe lo que mide.
    func destino(en tam: CGSize) -> CGRect? {
        guard let t = medidaVista, tam.width > 0, tam.height > 0 else { return nil }
        let d = encuadreDestino(e: encuadre, cajaVista: caja(tam), w: Float(t.width), h: Float(t.height))
        return CGRect(x: CGFloat(d.x), y: CGFloat(d.y), width: CGFloat(d.ancho), height: CGFloat(d.alto))
    }

    /// El tanto por ciento de la pastilla (pixeles de la imagen por punto).
    var porCiento: UInt32 {
        guard let t = medidaVista, cajaMedio.width > 0 else { return 100 }
        return encuadrePorCiento(e: encuadre, cajaVista: caja(cajaMedio), w: Float(t.width), h: Float(t.height))
    }

    var encajado: Bool { encuadreEstaEncajado(e: encuadre) }

    /// Si lo que se ve es el video (no una captura).
    var veVideo: Bool {
        guard esVideo, jugador != nil else { return false }
        return !(capturaALaVista.map { clase.capturas.indices.contains($0) } ?? false)
    }

    /// Acerca (`factor` > 1) o aleja hacia `punto` (el centro, sin el).
    func acercar(_ factor: Float, en punto: CGPoint? = nil) {
        guard let t = medidaVista, cajaMedio.width > 0, cajaMedio.height > 0 else { return }
        let p = punto ?? CGPoint(x: cajaMedio.width / 2, y: cajaMedio.height / 2)
        encuadre = encuadreAcercar(e: encuadre, cajaVista: caja(cajaMedio), w: Float(t.width), h: Float(t.height),
                                   factor: factor, px: Float(p.x), py: Float(p.y))
    }

    /// La lleva con el dedo, sin perderla de vista.
    func mover(_ dx: CGFloat, _ dy: CGFloat) {
        guard let t = medidaVista, cajaMedio.width > 0, cajaMedio.height > 0 else { return }
        encuadre = encuadreMover(e: encuadre, cajaVista: caja(cajaMedio), w: Float(t.width), h: Float(t.height),
                                 dx: Float(dx), dy: Float(dy))
    }

    func encajar() {
        if !encajado { encuadre = encuadreEncajado() }
    }

    /// Si cambio lo que se ve (otra captura, del video a la captura), vuelve
    /// a encajada: acercar una no es acercar la otra.
    private func alDiaLoVisto() {
        let ahora: String? = capturaALaVista.map { "captura-\($0)" } ?? (esVideo ? "video" : nil)
        guard let ahora, ahora != visto else { return }
        visto = ahora
        encuadre = encuadreEncajado()
    }

    /// ⎋: acercado, primero lo encaja; si no, cierra, como los visores.
    func escapar(_ cerrar: () -> Void) {
        if !encajado { return encajar() }
        cerrar()
    }

    // MARK: Anotar

    /// **«Anotar»** (o A): la captura que se ve, en su lienzo; con el video,
    /// se para, se guarda el fotograma como una captura de la clase en su
    /// minuto y se abre esa.
    func anotar() {
        if let i = capturaALaVista, clase.capturas.indices.contains(i) {
            let c = clase.capturas[i]
            return anotarCaptura(c.id, ruta: c.ruta, ya: nil)
        }
        guard esVideo, let video = clase.video else { return avisar("Aún no hay nada que anotar aquí") }
        // Se para: se anota lo que se esta viendo, no lo que viene.
        if sonando { alternar() }
        let ms = ahoraMs
        let (nucleo, chat, voz, almacen) = (self.almacen.nucleo, clase.chat, clase.id, self.almacen)
        _ = Task<Void, Never> {
            guard let png = await ModeloEstudiar.fotograma(video, ms: ms) else { return avisar("No se pudo guardar") }
            let hecho = await Task.detached(priority: .userInitiated) { () -> FotogramaDeClase? in
                defer { try? FileManager.default.removeItem(at: png) }
                return try? nucleo.guardarFotogramaDeClase(chat: chat, voz: voz, ms: ms, rutaPng: png.path)
            }.value
            guard let hecho else { return avisar("No se pudo guardar") }
            avisar("Fotograma guardado como captura: anótalo en el lienzo")
            leido = nucleo.cambioDelCuadernoMs(chat: chat)
            // Que se vea la captura nueva y no el video.
            verCapturas = true
            if hecho.clase.id == clase.id { aplicar(hecho.clase) }
            ClaseEnElChat.olvidar(chat: chat, mensaje: voz)
            if almacen.mensajes[chat] != nil { almacen.anadir([hecho.imagen], en: chat) }
            let ruta = hecho.clase.capturas.first { $0.id == hecho.imagen.id }?.ruta
            anotarCaptura(hecho.imagen.id, ruta: ruta, ya: hecho.imagen)
        }
    }

    /// **Abre la captura `id` en SU lienzo**, como al tocarla en el chat:
    /// el `foto-<id>` que viaja con ella, con todas las herramientas, encima
    /// de esta pantalla. Al cerrarlo se vuelve aqui.
    private func anotarCaptura(_ id: String, ruta: String?, ya: MensajeChat?) {
        let (nucleo, chat, almacen) = (self.almacen.nucleo, clase.chat, self.almacen)
        if sonando { alternar() }
        _ = Task<Void, Never> {
            let lienzo = await Task.detached(priority: .userInitiated) {
                try? nucleo.asegurarLienzoDeFoto(chat: chat, id: id)
            }.value
            guard let lienzo else { return avisar("No se pudo guardar") }
            // La foto del chat (cargada, o leida del cuaderno por su lienzo).
            var foto = ya ?? almacen.mensajes[chat]?.first { $0.id == id }
            if foto == nil {
                foto = await Task.detached(priority: .userInitiated) {
                    nucleo.mensajeDelDibujo(chat: chat, dibujo: lienzo)
                }.value
            }
            guard let foto else { return avisar("No se pudo guardar") }
            let url = ruta.map { URL(fileURLWithPath: $0) }
            ArribaUIKit.presentar(estilo: .fullScreen) { cerrar in
                LienzoSobreLaClase(visor: .lienzo(chat: chat, mensaje: foto, foto: url), almacen: almacen,
                                   cerrar: cerrar)
            }
        }
    }

    /// El fotograma de `video` en `ms`, exacto, en un PNG temporal.
    nonisolated static func fotograma(_ video: String, ms: Int64) async -> URL? {
        let generador = AVAssetImageGenerator(asset: AVURLAsset(url: URL(fileURLWithPath: video)))
        generador.appliesPreferredTrackTransform = true
        generador.requestedTimeToleranceBefore = .zero
        generador.requestedTimeToleranceAfter = .zero
        guard let imagen = try? await generador.image(at: CMTime(value: ms, timescale: 1000)).image else { return nil }
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-fotograma-\(UUID().uuidString).png")
        guard let destino = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destino, imagen, nil)
        guard CGImageDestinationFinalize(destino) else { return nil }
        return url
    }

    /// Lo mas que mide la captura anotada al pintarla aqui.
    nonisolated static let ladoAnotada = 2048

    /// **Lo anotado encima de la captura que se ve**, al dia: se lee de su
    /// lienzo propio (`Nucleo.fotoAnotada`, recortado a la foto) si cambio
    /// la captura o se guardo el lienzo.
    private func pedirLoAnotado() {
        guard let i = capturaALaVista, clase.capturas.indices.contains(i) else {
            if anotada != nil { anotada = nil }
            anotadaPedida = nil
            return
        }
        let lienzo = clase.capturas[i].lienzo
        guard anotadaPedida != lienzo else { return }
        anotadaPedida = lienzo
        if anotada?.lienzo != lienzo { anotada = nil }
        let (chat, almacen) = (clase.chat, self.almacen)
        _ = Task<Void, Never> {
            let hecha = await ModeloEstudiar.pintarAnotada(chat: chat, lienzo: lienzo, almacen: almacen)
            guard anotadaPedida == lienzo else { return }
            anotada = hecha.map { (lienzo: lienzo, imagen: $0) }
        }
    }

    /// La captura con lo dibujado en su lienzo, recortada a la foto; `nil`
    /// si no tiene nada encima (se ensena la captura de siempre).
    private static func pintarAnotada(chat: String, lienzo id: String, almacen: Almacen) async -> CGImage? {
        guard !id.isEmpty else { return nil }
        let nucleo = almacen.nucleo
        let leida = await Task.detached(priority: .utility) {
            try? nucleo.fotoAnotada(chat: chat, lienzo: id)
        }.value ?? nil
        guard let leida, let dibujo = await almacen.dibujo(leida.lienzo, en: chat) else { return nil }
        var lienzo = dibujo.lienzo
        let foto = leida.foto.map {
            CGRect(x: CGFloat($0.x0), y: CGFloat($0.y0), width: CGFloat($0.x1 - $0.x0), height: CGFloat($0.y1 - $0.y0))
        }
        lienzo.caja = cajaDeFotoAnotada(foto: foto, todo: lienzo.caja, soloLaFoto: true)
        guard let caja = lienzo.caja, caja.width > 0, caja.height > 0 else { return nil }
        let tam = tamanoDeFotoAnotada(caja.size, lado: CGFloat(ladoAnotada))
        let w = max(1, Int(tam.width.rounded())), h = max(1, Int(tam.height.rounded()))
        var fotos: [UInt64: CGImage] = [:]
        for (i, url) in dibujo.fotos {
            if let img = await Miniaturas.compartidas.cargar(url, lado: max(w, h)) { fotos[i] = img }
        }
        let listas = fotos
        let leido = lienzo
        return await Task.detached(priority: .utility) {
            MapaLienzo.imagen(leido, ancho: w, alto: h, margen: 0) { listas[$0] }
        }.value
    }

    // MARK: Compartir

    /// **«Compartir»**: la grabacion en la hoja de toda la app, con su
    /// pagina web, sus hojas, Word, Markdown, Texto, Anki y los originales
    /// (`PrepararCompartir.deGrabacion`, lo mismo que desde el chat).
    func compartir() {
        let (nucleo, chat, id, almacen) = (self.almacen.nucleo, clase.chat, clase.id, self.almacen)
        if sonando { alternar() }
        _ = Task<Void, Never> {
            let leida = await Task.detached(priority: .userInitiated) {
                try? nucleo.grabacionParaCompartir(chat: chat, mensaje: id)
            }.value
            guard var g = leida else { return avisar("No se pudo: mira el registro de PixPin") }
            if g.cuando > 0 { g.fecha = PrepararCompartir.fechaDeGrabacion(g.cuando) }
            let p = await PrepararCompartir.deGrabacion(g, chat: chat, almacen: almacen)
            HojaCompartir.abrir(p)
        }
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
        if esVideo, let t = j.currentItem?.presentationSize, t != tamanoVideo, t.width > 0 { tamanoVideo = t }
        let suena = j.rate != 0
        if suena != sonando { sonando = suena }
        // Mientras carga, se ensena a donde se va; arrastrando la barra,
        // donde esta el dedo (`clase::posicion_a_la_vista`).
        let p = clasePosicionALaVista(medioMs: ahoraMs, arrastrando: arrastrando)
        // Lo que se pinta va a saltos de 0,1 s: de sobra para la barra y el
        // resaltado, y la pantalla no se repinta entera en cada vuelta.
        if abs(p - posicionMs) >= 100 || (!sonando && p != posicionMs) { posicionMs = p }
        pedirLaGrande()
        alDiaLoVisto()
        pedirLoAnotado()
        seguirAlSonido()
    }

    /// **Arrastrando la barra del tiempo**: el tirador va con el dedo y el
    /// fotograma exacto se busca cada 70 ms (Windows fe0c9c4).
    func arrastrarBarra(a ms: Int64) {
        arrastrando = ms
        posicionMs = ms
        guard Date().timeIntervalSince(buscadoAl) >= 0.07 else { return }
        buscadoAl = Date()
        irA(ms)
    }

    /// Se solto la barra: alli se queda.
    func soltarBarra() {
        if let ms = arrastrando { irA(ms) }
        arrastrando = nil
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
            // El lienzo de una captura pudo estrenarse (su `referencia`).
            anotadaPedida = nil
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
        case .escape: escapar(cerrar)
        default:
            // Solo al pulsar (no al repetir) las letras.
            guard k.phase == .down else { return false }
            switch k.characters.lowercased() {
            case "k": alternar()
            case "j": saltar(-ModeloEstudiar.saltoLargo)
            case "l": saltar(ModeloEstudiar.saltoLargo)
            case "m": empezarMarcador()
            case "s": alternarSeguir()
            case "a": anotar()
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
        for v in vigias { NotificationCenter.default.removeObserver(v) }
        vigias = []
    }
}

/// **El lienzo de una captura, encima de la clase**: el editor de siempre
/// con su propia navegacion; al cerrarlo (su ✕ cierra el visor) se vuelve a
/// la clase, que sigue abierta debajo.
private struct LienzoSobreLaClase: View {
    @StateObject private var nav: Navegacion
    let almacen: Almacen
    let cerrar: () -> Void

    init(visor: Visor, almacen: Almacen, cerrar: @escaping () -> Void) {
        let n = Navegacion()
        n.abrir(visor)
        _nav = StateObject(wrappedValue: n)
        self.almacen = almacen
        self.cerrar = cerrar
    }

    var body: some View {
        Group {
            if let visor = nav.visor {
                VistaDelVisor(visor: visor, almacen: almacen)
            } else {
                Color.clear
            }
        }
        .environmentObject(nav)
        .onChange(of: nav.visor == nil) { _, cerrado in
            if cerrado { cerrar() }
        }
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
    @ObservedObject private var noche = ModoNoche.compartido

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
        // Claro u oscuro, como los visores (la luna/sol de la barra).
        .environment(\.colorScheme, noche.activo ? .dark : .light)
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
    /// Lo ya llevado de este arrastre y lo ya acercado de este pellizco.
    @State private var llevado = CGSize.zero
    @State private var acercado: CGFloat = 1

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
            GeometryReader { geo in
                encuadrado(geo.size)
                    .frame(width: geo.size.width, height: geo.size.height)
                    .contentShape(Rectangle())
                    .onAppear { m.cajaMedio = geo.size }
                    .onChange(of: geo.size) { _, t in m.cajaMedio = t }
                    // Con los dedos, como en un lienzo: un toque reproduce o
                    // para, el doble toque encaja, el pellizco acerca hacia
                    // los dedos y arrastrar la lleva.
                    .gesture(arrastrar)
                    .simultaneousGesture(pellizco)
                    .onTapGesture(count: 2) { m.encajar() }
                    .onTapGesture { m.alternar() }
            }
            .clipShape(RoundedRectangle(cornerRadius: 10))
            // Arriba a la izquierda, «Anotar»: la captura en su lienzo o el
            // fotograma del video como captura.
            ficha("Anotar", "pencil") { m.anotar() }
                .help("Anotar (A)")
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(10)
            // Abajo a la derecha, −, el tanto por ciento, + y encajar.
            if m.medidaVista != nil {
                pastillaDelZoom
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                    .padding(10)
            }
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

    private var arrastrar: some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { v in
                m.mover(v.translation.width - llevado.width, v.translation.height - llevado.height)
                llevado = v.translation
            }
            .onEnded { _ in llevado = .zero }
    }

    private var pellizco: some Gesture {
        MagnifyGesture()
            .onChanged { v in
                let factor = v.magnification / max(acercado, 0.01)
                acercado = v.magnification
                m.acercar(Float(max(0.2, factor)), en: v.startLocation)
            }
            .onEnded { _ in acercado = 1 }
    }

    /// Lo que se ve, en el sitio que dice el encuadre (encajado, lo de
    /// siempre: entero y centrado).
    @ViewBuilder private func encuadrado(_ tam: CGSize) -> some View {
        let d = m.destino(en: tam)
        if m.veVideo, let j = m.jugador {
            VideoDeLaClase(jugador: j, destino: d)
        } else if let d {
            ZStack(alignment: .topLeading) {
                contenido
                    .frame(width: d.width, height: d.height)
                    .position(x: d.midX, y: d.midY)
            }
            .frame(width: tam.width, height: tam.height, alignment: .topLeading)
        } else {
            contenido
        }
    }

    /// Una ficha oscura sobre lo que se ve (como «Ver el vídeo»).
    private func ficha(_ rotulo: String, _ simbolo: String, _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Label(rotulo, systemImage: simbolo)
                .font(.system(size: 13))
                .foregroundStyle(.white)
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(Color.black.opacity(0.65), in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .fixedSize()
    }

    /// **La pastilla del zoom**: −, el tanto por ciento, + y encajar.
    private var pastillaDelZoom: some View {
        HStack(spacing: 2) {
            botonDelZoom("minus", "Alejar (⌘−)") { m.acercar(1 / encuadrePaso()) }
                .keyboardShortcut("-", modifiers: .command)
            Text("\(m.porCiento) %")
                .font(.system(size: 13).monospacedDigit())
                .foregroundStyle(.white)
                .frame(minWidth: 50)
            botonDelZoom("plus", "Acercar (⌘+)") { m.acercar(encuadrePaso()) }
                .keyboardShortcut("+", modifiers: .command)
            botonDelZoom("arrow.up.left.and.arrow.down.right", "Encajar (⌘0 o doble toque)") { m.encajar() }
                .keyboardShortcut("0", modifiers: .command)
                .disabled(m.encajado)
                .opacity(m.encajado ? 0.45 : 1)
        }
        .padding(.horizontal, 4)
        .frame(height: 32)
        .background(Color.black.opacity(0.65), in: Capsule())
        .fixedSize()
    }

    private func botonDelZoom(_ simbolo: String, _ ayuda: String, _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Image(systemName: simbolo)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 30, height: 30)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }

    @ViewBuilder private var contenido: some View {
        if let i = m.capturaALaVista, m.clase.capturas.indices.contains(i) {
            let ruta = m.clase.capturas[i].ruta
            if let a = m.anotada, a.lienzo == m.clase.capturas[i].lienzo {
                // Lo anotado en su lienzo, encima de la captura.
                Image(decorative: a.imagen, scale: 1).resizable().interpolation(.high).scaledToFit()
            } else if let g = m.grande, g.ruta == ruta {
                Image(uiImage: g.imagen).resizable().interpolation(.high).scaledToFit()
            } else if let mini = m.miniaturas[ruta] {
                Image(uiImage: mini).resizable().scaledToFit()
            } else {
                vacio("photo", "Cargando…")
            }
        } else if m.esVideo, let j = m.jugador {
            VideoDeLaClase(jugador: j, destino: nil)
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
/// reproduce y para), en su encuadre (`destino`; sin el, la caja entera) y
/// recortado a su caja: acercado no se sale de su sitio.
private struct VideoDeLaClase: UIViewRepresentable {
    let jugador: AVPlayer
    let destino: CGRect?

    final class Lienzo: UIView {
        let capa = AVPlayerLayer()
        var destino: CGRect? {
            didSet { if destino != oldValue { setNeedsLayout() } }
        }

        override init(frame: CGRect) {
            super.init(frame: frame)
            clipsToBounds = true
            layer.addSublayer(capa)
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

        override func layoutSubviews() {
            super.layoutSubviews()
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            capa.frame = destino ?? bounds
            CATransaction.commit()
        }
    }

    func makeUIView(context: Context) -> Lienzo {
        let v = Lienzo()
        v.backgroundColor = .clear
        v.capa.videoGravity = .resizeAspect
        v.capa.player = jugador
        v.destino = destino
        return v
    }

    func updateUIView(_ v: Lienzo, context: Context) {
        if v.capa.player !== jugador { v.capa.player = jugador }
        v.destino = destino
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
                    m.arrastrarBarra(a: clasePegarASenal(x: Float(g.location.x), ancho: ancho, duracionMs: duracion,
                                                         senales: senales, tolerancia: ModeloEstudiar.pegar))
                }
                .onEnded { _ in
                    arrastrando = false
                    m.soltarBarra()
                })
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
