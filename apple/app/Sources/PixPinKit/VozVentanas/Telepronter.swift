// **El teleprónter: el texto baja solo y uno lo lee en voz alta, grabandose**
// (Windows `teleprompter.rs`, Mac `VozVentanas/Telepronter.swift`, movil
// `TelepronterActivity.kt`). No hay voz sintetica: lee el usuario y el iPad
// graba.
//
// La nota que sale **ya trae su texto con minutos** sin pasar por ningun
// reconocedor: como el texto baja a una velocidad conocida, se sabe en que
// segundo cada parrafo cruzo la franja de lectura (`Desfile`, la cuenta de
// `pixpin_voz::telepronter`, copiada del Mac con sus pruebas). Entra en el
// chat como `VOZ` con `transcripcion` y `estadoDelTexto = bien`.
//
// En el iPad sale a pantalla completa encima de la app. Arriba ← y el reloj
// de la grabacion; en medio el texto con su franja (arrastrarlo lo mueve a
// mano, como la rueda); abajo A−/A+, −/+ (la velocidad dicha con palabras),
// la raya de lo leido, «Ensayar», «Grabar» y, grabando, «Pausa», «Tirar» y
// «Terminar». Tres, dos, uno antes de grabar. La pantalla no se apaga
// mientras esta abierto.
//
// Teclado (si hay): Esc cierra (tira lo grabado), Espacio pausa o ensaya,
// Intro graba o termina, ← → la velocidad, ↑ ↓ un renglon, + − la letra,
// RePag/AvPag media pantalla.
//
// Rendimiento: el texto se mide una vez por letra y ancho, y solo se pintan
// los parrafos que se ven. El reloj (40 por segundo) solo corre mientras el
// texto se mueve o se cuenta; quieto, no hay temporizador.

import PixPinCore
import SwiftUI
import UIKit

// MARK: - La cuenta (pura)

/// Un parrafo ya medido: donde empieza en la columna sin desplazar y cuanto
/// ocupa.
struct MedidaParrafo: Equatable {
    var cima: CGFloat
    var alto: CGFloat
}

/// **El desfile** (`pixpin_voz::telepronter::Desfile`): por donde va el
/// texto y en que milisegundo cruzo cada parrafo la linea de lectura. No
/// sabe nada de letras: quien pinta mide y se lo da.
struct Desfile: Equatable {
    private(set) var medidas: [MedidaParrafo] = []
    /// En que milisegundo cruzo cada parrafo; -1 es «todavia no».
    private var tiempos: [Int64]
    /// Cuanto se ha desplazado el texto hacia arriba.
    var y: CGFloat = 0
    private var altoTotal: CGFloat = 0
    private var altoVisible: CGFloat = 0

    init(parrafos: Int) {
        tiempos = Array(repeating: -1, count: parrafos)
    }

    /// Lo que mide cada parrafo y cuanto se ve. Si cambio el numero de
    /// parrafos, los minutos de antes ya no son de nadie.
    mutating func medir(_ m: [MedidaParrafo], altoVisible: CGFloat) {
        altoTotal = max(m.last.map { $0.cima + $0.alto } ?? 0, 0)
        self.altoVisible = max(altoVisible, 0)
        if m.count != tiempos.count { tiempos = Array(repeating: -1, count: m.count) }
        medidas = m
        y = acotar(y)
    }

    /// Solo cambia cuanto se ve, sin volver a medir.
    mutating func ajustarVentana(_ alto: CGFloat) {
        altoVisible = max(alto, 0)
        y = acotar(y)
    }

    func medida(_ i: Int) -> MedidaParrafo? { medidas.indices.contains(i) ? medidas[i] : nil }

    /// Hasta donde se puede bajar: el final del texto se queda a la vista.
    var tope: CGFloat { max(altoTotal - altoVisible, 0) }

    private func acotar(_ y: CGFloat) -> CGFloat { min(max(y, 0), tope) }

    /// Donde cae la linea de lectura, desde arriba de lo que se ve.
    var lineaDeLectura: CGFloat { altoVisible * Telepronter.franja }

    /// Baja lo que toque en `segundos` a esa velocidad. `true` cuando ya no
    /// hay mas que bajar (lo que para el ensayo solo).
    @discardableResult
    mutating func avanzar(_ segundos: Double, velocidad: CGFloat) -> Bool {
        y = acotar(y + Telepronter.acotarVelocidad(velocidad) * CGFloat(max(segundos, 0)))
        return y >= tope
    }

    /// Mueve el texto a mano: cuenta igual que el desfile.
    mutating func empujar(_ puntos: CGFloat) { y = acotar(y + puntos) }

    /// Arriba y sin minutos: lo que hace empezar a grabar.
    mutating func reiniciar() {
        y = 0
        tiempos = Array(repeating: -1, count: tiempos.count)
    }

    /// Apunta, en `ms`, los parrafos que ya cruzaron la linea y no tenian
    /// minuto. Solo la primera vez: volver a subir no se lo quita.
    mutating func apuntar(_ ms: Int64) {
        let linea = lineaDeLectura
        for (i, m) in medidas.enumerated() where tiempos[i] < 0 && m.cima - y <= linea {
            tiempos[i] = max(ms, 0)
        }
    }

    /// El minuto de un parrafo, o `nil` si todavia no cruzo.
    func tiempo(_ i: Int) -> Int64? {
        tiempos.indices.contains(i) && tiempos[i] >= 0 ? tiempos[i] : nil
    }

    /// Los parrafos que se ven ahora (solo esos se pintan).
    var visibles: Range<Int> {
        let arriba = y, abajo = y + altoVisible
        let primero = medidas.firstIndex { $0.cima + $0.alto > arriba } ?? medidas.count
        let ultimo = (medidas.lastIndex { $0.cima < abajo }).map { $0 + 1 } ?? primero
        return primero..<max(ultimo, primero)
    }

    /// El texto que se guarda: cada parrafo con su minuto, separados por un
    /// renglon en blanco. Lo que no llego a cruzar se queda con el final.
    func cuerpo(_ parrafos: [String], duracionMs: Int64) -> String {
        parrafos.enumerated().map { i, p in
            let ms = min(tiempo(i) ?? duracionMs, max(duracionMs, 0))
            return "[\(FormatoVoz.marca(ms: ms))] \(p)"
        }.joined(separator: "\n\n")
    }
}

/// Las constantes y lo puro del teleprónter.
enum Telepronter {
    /// A que altura esta la linea de lectura (`FRANJA`).
    static let franja: CGFloat = 0.3
    static let cuentaAtras = 3
    /// Lo que dura cada numero de la cuenta: «un segundo escaso».
    static let msPorNumero = 900
    static let velocidadMinima: CGFloat = 10
    static let velocidadMaxima: CGFloat = 150
    static let velocidadPorDefecto: CGFloat = 45
    static let tamanoMinimo: CGFloat = 16
    static let tamanoMaximo: CGFloat = 56
    static let tamanoPorDefecto: CGFloat = 30
    /// El nombre de la ventanita (una a la vez).
    static let nombre = "telepronter"

    static func acotarVelocidad(_ v: CGFloat) -> CGFloat { min(max(v, velocidadMinima), velocidadMaxima) }
    static func acotarTamano(_ t: CGFloat) -> CGFloat { min(max(t, tamanoMinimo), tamanoMaximo) }

    /// La velocidad dicha con palabras: un «45» no significa nada.
    static func palabra(deVelocidad v: CGFloat) -> String {
        switch v {
        case ..<30: return "muy lenta"
        case ..<55: return "lenta"
        case ..<85: return "normal"
        case ..<115: return "rápida"
        default: return "muy rápida"
        }
    }

    /// **El texto en parrafos para leer** (`parrafos_de`): fuera las marcas
    /// de Markdown que no se leen (`#`, `**`, `>`, vinetas, imagenes) y
    /// partido por lineas en blanco; las lineas de un parrafo se juntan.
    static func parrafos(de texto: String) -> [String] {
        var bloques: [[Substring]] = []
        var actual: [Substring] = []
        for linea in texto.split(separator: "\n", omittingEmptySubsequences: false) {
            if linea.trimmingCharacters(in: .whitespaces).isEmpty {
                if !actual.isEmpty { bloques.append(actual); actual = [] }
            } else {
                actual.append(linea)
            }
        }
        if !actual.isEmpty { bloques.append(actual) }
        return bloques.compactMap { bloque in
            let limpio = bloque
                .filter { !$0.drop { $0 == " " || $0 == "\t" }.hasPrefix("![") }
                .map { l -> String in
                    let sin = l.drop { "# >-*".contains($0) }
                    return String(sin).replacingOccurrences(of: "**", with: "")
                        .replacingOccurrences(of: "__", with: "")
                        .replacingOccurrences(of: "`", with: "")
                        .trimmingCharacters(in: .whitespaces)
                }
                .joined(separator: " ")
                .trimmingCharacters(in: .whitespaces)
            return limpio.isEmpty ? nil : limpio
        }
    }
}

/// Lo que sale de una lectura: lo que va al mensaje de voz.
struct LecturaTelepronter {
    var nota: NotaGrabada
    /// `[m:ss] parrafo`, para `transcripcion`.
    var cuerpo: String
}

// MARK: - El modelo

@MainActor
final class ModeloTelepronter: ObservableObject {
    enum Fase: Equatable { case parado, ensayando, contando, grabando }

    let parrafos: [String]
    /// El desfile no se publica: cambia 40 veces por segundo y lo pinta su
    /// vista; se publica el fotograma.
    var desfile: Desfile
    @Published var velocidad = Telepronter.velocidadPorDefecto
    @Published var tamano = Telepronter.tamanoPorDefecto
    @Published private(set) var fase = Fase.parado
    @Published private(set) var pausado = false
    @Published private(set) var cuenta = 0
    @Published private(set) var fotograma = 0
    let aviso = AvisoCorto()

    private let mic = MicrofonoSuelto()
    private var abriendo: Task<Void, Never>?
    private var falloAlAbrir: String?
    private var cuentaHasta = Date()
    private var reloj: Timer?
    private var ultimo = Date()
    private(set) var inicio = Date()
    /// Con que letra y que ancho se midio lo que hay en el desfile.
    private var medidoCon: (CGFloat, CGFloat) = (0, 0)
    /// Lo que llevaba el dedo en el arrastre anterior.
    private var arrastreAntes: CGFloat = 0
    var alTerminar: ((LecturaTelepronter) -> Void)?
    var cerrar: (() -> Void)?

    init(parrafos: [String]) {
        self.parrafos = parrafos
        desfile = Desfile(parrafos: parrafos.count)
    }

    // MARK: Medir

    /// Mide los parrafos si cambio la letra o el ancho (lo llama quien
    /// pinta, con su caja).
    func medir(ancho: CGFloat, alto: CGFloat) {
        let columna = max(ancho - Self.margen(ancho) * 2, 40)
        guard medidoCon != (tamano, columna) else {
            desfile.ajustarVentana(alto)
            return
        }
        let atributos = Self.atributos(tamano, color: .white)
        var y: CGFloat = 0
        let medidas = parrafos.map { p -> MedidaParrafo in
            let alto = (p as NSString).boundingRect(
                with: CGSize(width: columna, height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: atributos, context: nil
            ).height.rounded(.up)
            defer { y += alto + tamano * 0.9 }
            return MedidaParrafo(cima: y, alto: alto)
        }
        desfile.medir(medidas, altoVisible: alto)
        medidoCon = (tamano, columna)
    }

    /// Una columna estrecha se lee de un vistazo.
    nonisolated static func margen(_ ancho: CGFloat) -> CGFloat { max(ancho * 0.12, 24) }

    nonisolated static func atributos(_ tamano: CGFloat, color: UIColor) -> [NSAttributedString.Key: Any] {
        [.font: UIFont.systemFont(ofSize: tamano, weight: .regular), .foregroundColor: color]
    }

    // MARK: Hacer

    func ensayar() {
        switch fase {
        case .ensayando: fase = .parado
        case .parado: fase = .ensayando
        case .grabando, .contando: break
        }
        armar()
    }

    /// Espacio: grabando, para el texto y deja correr la grabacion.
    func pausa() {
        if fase == .grabando { pausado.toggle() } else { ensayar() }
        armar()
    }

    func grabar() {
        pausado = false
        guard fase != .grabando, fase != .contando else { return }
        fase = .contando
        cuenta = Telepronter.cuentaAtras
        cuentaHasta = Date().addingTimeInterval(Double(Telepronter.msPorNumero) / 1000)
        // El microfono se abre ya: tarda y asi esta listo al llegar a cero.
        falloAlAbrir = nil
        let fichero = Ventanita.carpeta("telepronter").appendingPathComponent("lectura-\(Ventanita.ahoraMs).m4a")
        abriendo = Task { [weak self] in
            guard let self else { return }
            do { try await self.mic.empezar(en: fichero) } catch { self.falloAlAbrir = MicrofonoSuelto.explicar(error) }
            self.abriendo = nil
        }
        armar()
    }

    func tirar() {
        mic.cancelar()
        fase = .parado
        pausado = false
        desfile.reiniciar()
        armar()
    }

    func terminar() {
        guard fase == .grabando else { return }
        fase = .parado
        armar()
        _ = Task<Void, Never> {
            guard let nota = await mic.parar() else { return }
            guard nota.duracionMs >= FormatoVoz.minimoMs else {
                try? FileManager.default.removeItem(at: nota.url)
                aviso.decir("La lectura salió demasiado corta")
                return
            }
            alTerminar?(LecturaTelepronter(nota: nota, cuerpo: desfile.cuerpo(parrafos, duracionMs: nota.duracionMs)))
            cerrar?()
        }
    }

    func cambiarVelocidad(_ paso: CGFloat) {
        velocidad = Telepronter.acotarVelocidad(velocidad + paso)
    }

    func cambiarLetra(_ paso: CGFloat) {
        tamano = Telepronter.acotarTamano(tamano + paso)
        fotograma &+= 1
    }

    func empujar(_ puntos: CGFloat) {
        desfile.empujar(puntos)
        if fase == .grabando { desfile.apuntar(mic.llevadoMs) }
        fotograma &+= 1
    }

    /// El dedo arrastrando el texto: hacia arriba baja, como un papel.
    func arrastrar(_ total: CGFloat, acabado: Bool) {
        empujar(-(total - arrastreAntes))
        arrastreAntes = acabado ? 0 : total
    }

    /// Al cerrar: lo grabado se tira y el reloj se para.
    func soltar() {
        reloj?.invalidate()
        reloj = nil
        abriendo?.cancel()
        mic.cancelar()
        UIApplication.shared.isIdleTimerDisabled = false
    }

    // MARK: El reloj

    /// Pone o quita el reloj segun la fase: quieto, nada corre.
    private func armar() {
        let corre = fase != .parado
        if corre, reloj == nil {
            ultimo = Date()
            let t = Timer(timeInterval: 1.0 / 40, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated { self?.latir() }
            }
            RunLoop.main.add(t, forMode: .common)
            reloj = t
        } else if !corre {
            reloj?.invalidate()
            reloj = nil
        }
        fotograma &+= 1
    }

    private func latir() {
        let ahora = Date()
        let segundos = ahora.timeIntervalSince(ultimo)
        ultimo = ahora
        if fase == .contando, ahora >= cuentaHasta {
            if cuenta > 1 {
                cuenta -= 1
                cuentaHasta = ahora.addingTimeInterval(Double(Telepronter.msPorNumero) / 1000)
            } else {
                cuenta = 0
                grabarYa(ahora)
            }
        }
        let baja = fase == .ensayando || (fase == .grabando && !pausado)
        if baja, desfile.avanzar(segundos, velocidad: velocidad), fase == .ensayando {
            fase = .parado
            armar()
        }
        // El minuto es el del audio (lo grabado de verdad), no el del reloj.
        if fase == .grabando { desfile.apuntar(mic.llevadoMs) }
        fotograma &+= 1
    }

    /// Al llegar a cero: graba si el microfono ya esta; si aun se abre, se
    /// vuelve a mirar en el siguiente latido.
    private func grabarYa(_ ahora: Date) {
        if mic.grabando {
            inicio = ahora
            desfile.reiniciar()
            fase = .grabando
        } else if let falloAlAbrir {
            // Que no se pueda grabar es normal (otra app tiene el
            // microfono, el permiso esta quitado): se dice en el acto.
            fase = .parado
            aviso.decir(falloAlAbrir)
            armar()
        }
    }

    // MARK: Teclado

    func tecla(_ k: KeyPress, altoPantalla: CGFloat) -> Bool {
        guard k.phase != .up else { return false }
        let repite = k.phase == .repeat
        switch k.key {
        case .escape: cerrar?()
        case .space: if !repite { pausa() }
        case .return: if !repite { fase == .grabando ? terminar() : grabar() }
        case .rightArrow: cambiarVelocidad(5)
        case .leftArrow: cambiarVelocidad(-5)
        case .downArrow: empujar(tamano * 1.45)
        case .upArrow: empujar(-tamano * 1.45)
        case .pageDown: empujar(altoPantalla * 0.5)
        case .pageUp: empujar(-altoPantalla * 0.5)
        default:
            switch TeclasVoz.letra(k) {
            case "+", "=": cambiarLetra(2)
            case "-": cambiarLetra(-2)
            default: return false
            }
        }
        return true
    }
}

// MARK: - La vista

struct VistaTelepronter: View {
    @ObservedObject var m: ModeloTelepronter
    @ObservedObject private var aviso: AvisoCorto
    @FocusState private var foco: Bool
    @State private var alto: CGFloat = 800

    init(m: ModeloTelepronter) {
        self.m = m
        aviso = m.aviso
    }

    var body: some View {
        VStack(spacing: 0) {
            barra
            LienzoDesfile(m: m, fotograma: m.fotograma, tamano: m.tamano)
                .background(GeometryReader { g in
                    Color.clear.onAppear { alto = g.size.height }.onChange(of: g.size.height) { _, h in alto = h }
                })
                .gesture(DragGesture(minimumDistance: 2)
                    .onChanged { g in m.arrastrar(g.translation.height, acabado: false) }
                    .onEnded { g in m.arrastrar(g.translation.height, acabado: true) })
                .overlay(alignment: .bottomLeading) {
                    if let t = aviso.texto {
                        Text(t).font(.system(size: 15)).foregroundStyle(ColoresVoz.dorado)
                            .padding(.leading, 40).padding(.bottom, 12)
                    }
                }
            pie
        }
        .background(ColoresVoz.fondo.ignoresSafeArea())
        .overlay { if m.fase == .contando, m.cuenta > 0 { cuenta } }
        .tecladoDeVentanita($foco) { k in m.tecla(k, altoPantalla: alto) }
        .onAppear { UIApplication.shared.isIdleTimerDisabled = true }
    }

    private var barra: some View {
        HStack(spacing: 12) {
            BotonVentanita("←", ancho: 44) { m.cerrar?() }
                .accessibilityLabel("Cerrar")
            if m.fase == .grabando {
                TimelineView(.periodic(from: m.inicio, by: 1)) { c in
                    Text("● \(FormatoVoz.marca(ms: Int64(c.date.timeIntervalSince(m.inicio) * 1000)))")
                        .foregroundStyle(ColoresVoz.rojo)
                }
                .font(.system(size: 17).monospacedDigit())
            } else {
                Text("Leer en voz alta").font(.system(size: 17)).foregroundStyle(ColoresVoz.texto)
            }
            Spacer()
        }
        .padding(.horizontal, 12)
        .frame(height: 56)
        .background(ColoresVoz.cristal)
    }

    private var pie: some View {
        let tope = m.desfile.tope
        let leido = tope > 0 ? min(max(m.desfile.y / tope, 0), 1) : 0
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 6) {
                mandos
                Text("velocidad y letra · \(Telepronter.palabra(deVelocidad: m.velocidad))")
                    .font(.system(size: 14)).foregroundStyle(ColoresVoz.apagado).padding(.leading, 8)
                Spacer(minLength: 8)
                acciones
            }
            VStack(spacing: 8) {
                HStack(spacing: 6) {
                    mandos
                    Text(Telepronter.palabra(deVelocidad: m.velocidad))
                        .font(.system(size: 14)).foregroundStyle(ColoresVoz.apagado)
                    Spacer(minLength: 0)
                }
                HStack(spacing: 6) { Spacer(minLength: 0); acciones }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 14)
        .background(ColoresVoz.cristal.ignoresSafeArea(edges: .bottom))
        // Cuanto texto queda, en una raya fina encima de los mandos.
        .overlay(alignment: .topLeading) {
            GeometryReader { g in
                Rectangle().fill(ColoresVoz.dorado).frame(width: g.size.width * leido, height: 3)
            }
        }
    }

    @ViewBuilder private var mandos: some View {
        BotonVentanita("A-", ancho: 48) { m.cambiarLetra(-2) }.accessibilityLabel("Letra más pequeña")
        BotonVentanita("A+", ancho: 48) { m.cambiarLetra(2) }.accessibilityLabel("Letra más grande")
        BotonVentanita("−", ancho: 48) { m.cambiarVelocidad(-5) }.accessibilityLabel("Más lento")
        BotonVentanita("+", ancho: 48) { m.cambiarVelocidad(5) }.accessibilityLabel("Más rápido")
    }

    @ViewBuilder private var acciones: some View {
        if m.fase == .grabando {
            BotonVentanita(m.pausado ? "Seguir" : "Pausa", ancho: 130) { m.pausa() }
            BotonVentanita("Tirar", ancho: 130) { m.tirar() }
            BotonVentanita("Terminar", color: ColoresVoz.rojo, ancho: 150) { m.terminar() }
        } else {
            BotonVentanita(m.fase == .ensayando ? "Parar" : "Ensayar", ancho: 130) { m.ensayar() }
            BotonVentanita("Grabar", color: ColoresVoz.rojo, ancho: 150) { m.grabar() }
        }
    }

    private var cuenta: some View {
        ZStack {
            Color(hex: 0x0C0C10).ignoresSafeArea()
            VStack(spacing: 20) {
                Text("\(m.cuenta)").font(.system(size: 96, weight: .regular).monospacedDigit())
                    .foregroundStyle(ColoresVoz.texto)
                Text("Prepárate").font(.system(size: 20)).foregroundStyle(ColoresVoz.apagado)
            }
        }
    }
}

/// El texto que baja, pintado con UIKit: se mide y se pinta con lo mismo
/// (`NSAttributedString`), asi la franja cae donde cae el texto.
struct LienzoDesfile: UIViewRepresentable {
    let m: ModeloTelepronter
    /// Solo para que SwiftUI lo vuelva a pintar.
    let fotograma: Int
    let tamano: CGFloat

    func makeUIView(context: Context) -> VistaDesfile {
        let v = VistaDesfile()
        v.m = m
        v.isOpaque = true
        v.contentMode = .redraw
        return v
    }

    func updateUIView(_ v: VistaDesfile, context: Context) {
        v.setNeedsDisplay()
    }
}

final class VistaDesfile: UIView {
    weak var m: ModeloTelepronter?

    override func draw(_ rect: CGRect) {
        ColoresVoz.fondoUIKit.setFill()
        UIRectFill(bounds)
        guard let m else { return }
        MainActor.assumeIsolated {
            m.medir(ancho: bounds.width, alto: bounds.height)
            let tam = m.tamano
            let d = m.desfile
            // La franja de lectura: donde hay que tener los ojos.
            UIColor(red: 0x1D / 255, green: 0x27 / 255, blue: 0x33 / 255, alpha: 1).setFill()
            UIRectFill(CGRect(x: 0, y: d.lineaDeLectura - tam * 0.35, width: bounds.width, height: tam * 1.5))
            let izquierda = ModeloTelepronter.margen(bounds.width)
            let columna = max(bounds.width - izquierda * 2, 40)
            let claro = UIColor(red: 0xE4 / 255, green: 0xE2 / 255, blue: 0xE6 / 255, alpha: 1)
            let apagado = UIColor(red: 0x9A / 255, green: 0x9A / 255, blue: 0xA2 / 255, alpha: 1)
            for i in d.visibles {
                guard let medida = d.medida(i), m.parrafos.indices.contains(i) else { continue }
                let color = d.tiempo(i) == nil ? claro : apagado
                (m.parrafos[i] as NSString).draw(
                    with: CGRect(x: izquierda, y: medida.cima - d.y, width: columna, height: medida.alto),
                    options: [.usesLineFragmentOrigin, .usesFontLeading],
                    attributes: ModeloTelepronter.atributos(tam, color: color), context: nil)
            }
        }
    }
}

// MARK: - Abrir

extension Telepronter {
    /// Si ya hay uno abierto.
    @MainActor static var abierto: Bool { Ventanita.abierta(nombre) }

    /// **Abre el teleprónter con ese texto** a pantalla completa. `false` si
    /// el texto no tiene nada que leer. La lectura llega por `alTerminar`;
    /// cerrar sin terminar no manda nada.
    @MainActor @discardableResult
    static func abrir(texto: String, alTerminar: @escaping (LecturaTelepronter) -> Void) -> Bool {
        if abierto { return true }
        let parrafos = parrafos(de: texto)
        guard !parrafos.isEmpty else { return false }
        let m = ModeloTelepronter(parrafos: parrafos)
        m.alTerminar = alTerminar
        return Ventanita.abrir(nombre, alCerrar: { m.soltar() }) { cerrar in
            VistaTelepronter(m: m).onAppear { m.cerrar = cerrar }
        }
    }
}
