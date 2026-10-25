// **La llamada** (Windows `llamada.rs`, Mac `VozVentanas/LlamadaSecreta.swift`):
// una nota de voz con hora y cada clase del horario suenan como si
// llamaran.
//
// - **Sonando**: la tarjeta «Llamada entrante» en el centro, encima de todo
//   lo de la app (tambien de los visores y los pines: una `UIWindow` propia
//   a nivel de alerta), con quien llama, Contestar y Colgar, y un tono de
//   llamada en bucle. Si nadie contesta en 45 s se corta y queda dicho como
//   «Llamada perdida · <quien>» en la ventanita de avisos, con «+10 min».
// - **Contestada**: la nota suena con el reloj de la llamada y cuelga sola
//   al acabar; en una clase, despues la voz de la app lee lo apuntado.
//   **Lo que cambia en el iPad**: no tiene auricular de llamadas (el
//   `eCommunications` de Windows, el del movil); la nota sale **bajita**
//   (o por los cascos, si estan puestos) y «Altavoz» la sube.
// - **Volver a llamar en** 5 min, 15, 30, 1 h o 2 h (sonando o ya
//   contestada): la misma alarma a otra hora y cuelga.
//
// **Por que no CallKit**: iOS no deja simular una llamada entrante sin un
// servicio de VoIP de verdad (PushKit con su servidor), y lo revisan en la
// tienda. La alternativa: con la app abierta, esta pantalla de llamada; con
// la app cerrada, la notificacion del sistema **con sonido de llamada**
// (`TonoLlamada`, un timbre hecho aqui y dejado en `Library/Sounds`), que
// al tocarla abre la llamada ya contestada.
//
// Teclado: Intro contesta, Esc cuelga.

import AVFoundation
import PixPinCore
import SwiftUI
import UIKit

// MARK: - Lo puro

enum Llamada {
    /// Cuanto suena antes de darla por perdida (`SEGUNDOS_SONANDO`).
    static let segundosSonando: TimeInterval = 45
    /// Lo que tarda en colgar sola tras acabar la nota.
    static let colgarTras: TimeInterval = 0.9
    /// Los minutos de «Volver a llamar en» (`VOLVER_EN` del movil).
    static let volverEn = [5, 15, 30, 60, 120]
    /// El volumen «al oido» y el del altavoz.
    static let volumenAlOido: Float = 0.3
    static let volumenAltavoz: Float = 1
    /// Lo que viaja en los datos del aviso: quien llama (para «+10 min» de
    /// una clase, que se pospone con su nombre).
    static let claveNombre = "nombre"

    enum Fase: Equatable { case sonando, contestada }

    /// «5 min» o «1 h», como el movil.
    static func rotulo(deVolver minutos: Int) -> String {
        minutos < 60 ? "\(minutos) min" : "\(minutos / 60) h"
    }

    /// La hora a la que vuelve a llamar si se pide en `minutos`.
    static func horaDeVolver(_ ahora: Date, minutos: Int) -> Date {
        ahora.addingTimeInterval(TimeInterval(minutos) * 60)
    }

    /// Si toca colgar sola: sonando demasiado, o la nota ya acabo hace un
    /// rato.
    static func tocaColgar(_ fase: Fase, desde: Date, ahora: Date, acaboEn: Date?) -> Bool {
        // En milisegundos enteros: restar fechas deja decimales sueltos.
        func ms(_ a: Date, _ b: Date) -> Int { Int((a.timeIntervalSince(b) * 1000).rounded()) }
        switch fase {
        case .sonando: return ms(ahora, desde) >= Int(segundosSonando * 1000)
        case .contestada: return acaboEn.map { ms(ahora, $0) >= Int(colgarTras * 1000) } ?? false
        }
    }

    /// Lo que se lee de quien llama (vacio: «Llamada»).
    static func titulo(_ nombre: String) -> String {
        let n = nombre.trimmingCharacters(in: .whitespacesAndNewlines)
        return n.isEmpty ? "Llamada" : n
    }

    /// Si un recordatorio que vencio a las `cuando` aun suena como llamada
    /// al verlo `ahora`: lo que vencio con la app cerrada hace rato ya es
    /// una llamada perdida.
    static func aunSuena(_ cuando: Date, ahora: Date) -> Bool {
        ahora.timeIntervalSince(cuando) < 10 * 60
    }

    /// Si el id de un aviso es de una clase del horario (y no de un chat).
    static func esDelHorario(_ id: String) -> Bool { id.hasPrefix("horario:") }
}

// MARK: - El tono

/// **El timbre de la llamada**, hecho aqui: iOS no deja usar sus tonos de
/// llamada desde una app. Dos tonos juntos (400 + 450 Hz, el «ring-ring»
/// del telefono de siempre: 0,4 s, 0,2 s de silencio, 0,4 s y 2 s de
/// silencio), en WAV de 16 bits. Lo usan la pantalla de la llamada (en
/// bucle) y la notificacion del sistema (`Library/Sounds`, de menos de 30 s
/// como pide iOS).
enum TonoLlamada {
    /// El nombre del fichero en `Library/Sounds` (el que se da a
    /// `UNNotificationSound(named:)`).
    static let nombre = "pixpin-llamada.wav"
    static let muestrasPorSegundo = 22_050
    /// Lo que dura el fichero (menos de los 30 s del sistema).
    static let segundos: Double = 27

    /// Si en el instante `t` (segundos desde el principio) suena, y su
    /// volumen (con 8 ms de subida y bajada para que no chasque).
    static func envolvente(_ t: Double) -> Double {
        let ciclo = 3.0
        let x = t.truncatingRemainder(dividingBy: ciclo)
        let tramos: [(Double, Double)] = [(0, 0.4), (0.6, 1.0)]
        for (a, b) in tramos where x >= a && x < b {
            let rampa = 0.008
            return min(1, (x - a) / rampa, (b - x) / rampa)
        }
        return 0
    }

    /// El WAV entero (PCM de 16 bits, mono).
    static func wav(segundos: Double = TonoLlamada.segundos, muestrasPorSegundo: Int = TonoLlamada.muestrasPorSegundo) -> Data {
        let n = Int(segundos * Double(muestrasPorSegundo))
        var pcm = Data(capacity: n * 2)
        for i in 0 ..< n {
            let t = Double(i) / Double(muestrasPorSegundo)
            let e = envolvente(t)
            let v = e == 0 ? 0 : e * 0.45 * (sin(2 * .pi * 400 * t) + sin(2 * .pi * 450 * t)) / 2
            var s = Int16(max(-1, min(1, v)) * Double(Int16.max)).littleEndian
            withUnsafeBytes(of: &s) { pcm.append(contentsOf: $0) }
        }
        var d = Data()
        func u32(_ v: UInt32) { var x = v.littleEndian; withUnsafeBytes(of: &x) { d.append(contentsOf: $0) } }
        func u16(_ v: UInt16) { var x = v.littleEndian; withUnsafeBytes(of: &x) { d.append(contentsOf: $0) } }
        d.append(contentsOf: Array("RIFF".utf8))
        u32(UInt32(36 + pcm.count))
        d.append(contentsOf: Array("WAVE".utf8))
        d.append(contentsOf: Array("fmt ".utf8))
        u32(16)
        u16(1) // PCM
        u16(1) // mono
        u32(UInt32(muestrasPorSegundo))
        u32(UInt32(muestrasPorSegundo * 2))
        u16(2)
        u16(16)
        d.append(contentsOf: Array("data".utf8))
        u32(UInt32(pcm.count))
        d.append(pcm)
        return d
    }

    /// Donde lo busca el sistema para las notificaciones.
    static var url: URL? {
        FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask).first?
            .appendingPathComponent("Sounds", isDirectory: true)
            .appendingPathComponent(nombre)
    }

    /// Lo deja en `Library/Sounds` si aun no esta (fuera del hilo principal:
    /// son 1,2 MB). Devuelve donde esta.
    @discardableResult
    static func preparar() -> URL? {
        guard let url else { return nil }
        if FileManager.default.fileExists(atPath: url.path) { return url }
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        return (try? wav().write(to: url, options: .atomic)) != nil ? url : nil
    }
}

// MARK: - El modelo

@MainActor
final class ModeloLlamada: ObservableObject {
    let llamada: LlamadaSecreta
    let nucleo: Nucleo
    @Published private(set) var fase = Llamada.Fase.sonando
    @Published private(set) var desde = Date()
    @Published private(set) var altavoz = false
    @Published private(set) var dicho: String?
    private var timbre: AVAudioPlayer?
    private var voz: AVAudioPlayer?
    private var acaboEn: Date?
    private var reloj: Timer?
    private var colgada = false
    var cerrar: (() -> Void)?
    /// La hora y el aula de una clase (el horario).
    let detalle: String
    /// Lo que lee la voz al contestar, tras la nota (el horario).
    let textoALeer: String?
    private let habla = AVSpeechSynthesizer()
    private var leido = false

    init(_ llamada: LlamadaSecreta, nucleo: Nucleo, detalle: String = "", texto: String? = nil) {
        self.llamada = llamada
        self.nucleo = nucleo
        self.detalle = detalle
        let t = texto?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        textoALeer = t.isEmpty ? nil : t
    }

    /// Si es un aviso del horario (y no la nota de un chat).
    var esDelHorario: Bool { Llamada.esDelHorario(llamada.mensaje) }

    var nombre: String { Llamada.titulo(llamada.nombre) }

    /// La sesion de audio de reproducir (suena aunque el iPad este en
    /// silencio: es una llamada).
    private func sesion(activa: Bool) {
        let s = AVAudioSession.sharedInstance()
        if activa {
            try? s.setCategory(.playback, mode: .spokenAudio)
            try? s.setActive(true)
        } else {
            try? s.setActive(false, options: .notifyOthersOnDeactivation)
        }
    }

    /// Empieza a sonar: el tono por el altavoz, en bucle (una llamada se
    /// oye en el cuarto; lo secreto es lo que se dice despues).
    func sonar() {
        sesion(activa: true)
        if let t = TonoLlamada.preparar(), let p = try? AVAudioPlayer(contentsOf: t) {
            p.numberOfLoops = -1
            p.play()
            timbre = p
        }
        desde = Date()
        let r = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.latir() }
        }
        RunLoop.main.add(r, forMode: .common)
        reloj = r
    }

    func contestar() {
        guard fase == .sonando, dicho == nil else { return }
        timbre?.stop()
        timbre = nil
        let p = llamada.audio.isEmpty ? nil : try? AVAudioPlayer(contentsOf: URL(fileURLWithPath: llamada.audio))
        guard p != nil || textoALeer != nil else {
            colgar()
            return
        }
        if let p {
            p.volume = altavoz ? Llamada.volumenAltavoz : Llamada.volumenAlOido
            p.play()
            voz = p
        } else {
            leer()
        }
        fase = .contestada
        desde = Date()
    }

    /// La voz de la app lee lo apuntado (una vez, tras la nota).
    private func leer() {
        guard !leido, let t = textoALeer else { return }
        leido = true
        let u = AVSpeechUtterance(string: t)
        u.voice = AVSpeechSynthesisVoice(language: "es-ES")
        u.volume = altavoz ? Llamada.volumenAltavoz : Llamada.volumenAlOido
        habla.speak(u)
    }

    func cambiarAltavoz() {
        guard fase == .contestada else { return }
        altavoz.toggle()
        voz?.volume = altavoz ? Llamada.volumenAltavoz : Llamada.volumenAlOido
    }

    func colgar() {
        guard !colgada else { return }
        colgada = true
        reloj?.invalidate()
        reloj = nil
        timbre?.stop()
        voz?.stop()
        habla.stopSpeaking(at: .immediate)
        timbre = nil
        voz = nil
        sesion(activa: false)
        cerrar?()
    }

    /// «Volver a llamar en»: la misma alarma mas tarde, y cuelga.
    func volver(en minutos: Int) {
        guard dicho == nil else { return }
        let cuando = Llamada.horaDeVolver(Date(), minutos: minutos)
        let (nucleo, chat, mensaje) = (self.nucleo, llamada.chat, llamada.mensaje)
        timbre?.stop()
        voz?.stop()
        habla.stopSpeaking(at: .immediate)
        if esDelHorario {
            Recordatorios.compartido.posponerHorario(id: mensaje, nombre: llamada.nombre, a: cuando)
            dicho = "Te vuelve a llamar a las \(AtajosDeHora.legible(cuando, ahora: Date()))"
            Task {
                try? await Task.sleep(for: .seconds(1.6))
                colgar()
            }
            return
        }
        Task {
            let hecho = await Task.detached(priority: .userInitiated) {
                (try? nucleo.volverARecordar(chat: chat, mensaje: mensaje, cuandoUtcMs: milisegundos(cuando))) == true
            }.value
            if hecho {
                Recordatorios.avisarCambio(chat: chat, tarea: false)
                Recordatorios.compartido.releer()
                dicho = "Te vuelve a llamar a las \(AtajosDeHora.legible(cuando, ahora: Date()))"
            } else {
                dicho = "La nota ya no está"
            }
            try? await Task.sleep(for: .seconds(1.6))
            colgar()
        }
    }

    private func latir() {
        let ahora = Date()
        if fase == .contestada, acaboEn == nil, voz?.isPlaying != true {
            // Tras la nota, la voz lee lo apuntado; acaba cuando calla.
            if textoALeer != nil, !leido { leer() }
            if !habla.isSpeaking { acaboEn = ahora }
        }
        if dicho == nil, Llamada.tocaColgar(fase, desde: desde, ahora: ahora, acaboEn: acaboEn) {
            if fase == .sonando { perdida() }
            colgar()
        }
    }

    /// Nadie contesto: queda dicho en la ventanita de avisos, con
    /// «+10 min» (vuelve a llamar).
    private func perdida() {
        let datos = [AvisosRecordatorio.claveChat: llamada.chat, AvisosRecordatorio.claveId: llamada.mensaje,
                     Llamada.claveNombre: llamada.nombre]
        CentroAvisos.compartido.mostrarEnLaApp(
            Aviso(id: AvisosRecordatorio.identificador(chat: llamada.chat, id: llamada.mensaje) + ":perdida",
                  categoria: .llamada, titulo: "Llamada perdida", texto: "Llamada perdida · \(nombre)",
                  datos: datos),
            sonar: false)
    }
}

// MARK: - La vista

struct VistaLlamada: View {
    @ObservedObject var m: ModeloLlamada
    private let texto = Color(hex: 0xF5F5F7)

    var body: some View {
        ZStack {
            Color.black.opacity(0.45).ignoresSafeArea()
            tarjeta
        }
    }

    private var tarjeta: some View {
        VStack(spacing: 0) {
            Group {
                if m.fase == .contestada {
                    TimelineView(.periodic(from: m.desde, by: 1)) { c in
                        Text(FormatoVoz.marca(ms: Int64(max(0, c.date.timeIntervalSince(m.desde)) * 1000)))
                    }
                    .monospacedDigit()
                } else {
                    Text("Llamada entrante")
                }
            }
            .font(.system(size: 15)).foregroundStyle(texto.opacity(0.7)).padding(.top, 30)
            ZStack {
                Circle().fill(Color.white.opacity(0.12)).frame(width: 112, height: 112)
                Image(systemName: m.esDelHorario ? "graduationcap.fill" : "person.fill")
                    .font(.system(size: 50)).foregroundStyle(texto)
            }
            .padding(.top, 26)
            Text(m.nombre).font(.system(size: 26)).foregroundStyle(texto).lineLimit(2)
                .multilineTextAlignment(.center).padding(.horizontal, 16).padding(.top, 18)
            if !m.detalle.isEmpty {
                Text(m.detalle).font(.system(size: 15)).foregroundStyle(texto.opacity(0.75))
                    .multilineTextAlignment(.center).padding(.horizontal, 16).padding(.top, 6)
            }
            Spacer()
            if let dicho = m.dicho {
                Text(dicho).font(.system(size: 15)).foregroundStyle(texto).padding(.bottom, 24)
            } else {
                volver
            }
            redondos.padding(.bottom, 30)
        }
        .frame(width: 360, height: 560)
        .background(
            VStack(spacing: 0) {
                Color(hex: 0x101A33).frame(height: 296)
                Color(hex: 0x05070F)
            }
        )
        .clipShape(RoundedRectangle(cornerRadius: 22))
        .shadow(color: .black.opacity(0.5), radius: 24, y: 8)
        .environment(\.colorScheme, .dark)
        .accessibilityElement(children: .contain)
    }

    private var volver: some View {
        VStack(spacing: 10) {
            Text("Volver a llamar en").font(.system(size: 13)).foregroundStyle(texto.opacity(0.7))
            HStack(spacing: 0) {
                ForEach(Llamada.volverEn, id: \.self) { min in
                    Spacer(minLength: 0)
                    Button { m.volver(en: min) } label: {
                        Text(Llamada.rotulo(deVolver: min)).font(.system(size: 14)).foregroundStyle(texto)
                            .padding(.horizontal, 11).padding(.vertical, 10)
                            .background(Color.white.opacity(0.14), in: Capsule())
                            .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Volver a llamar en \(Llamada.rotulo(deVolver: min))")
                }
                Spacer(minLength: 0)
            }
        }
        .padding(.bottom, 34)
    }

    private var redondos: some View {
        HStack {
            if m.fase == .sonando {
                redondo("phone.down.fill", fondo: Paleta.v2.rojo, rotulo: "Colgar") { m.colgar() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                redondo("phone.fill", fondo: Paleta.v2.verde, rotulo: "Contestar") { m.contestar() }
                    .keyboardShortcut(.defaultAction)
            } else {
                redondo("speaker.wave.2.fill", fondo: m.altavoz ? texto : Color.white.opacity(0.16),
                        tinta: m.altavoz ? .black : texto, rotulo: "Altavoz") { m.cambiarAltavoz() }
                Spacer()
                redondo("phone.down.fill", fondo: Paleta.v2.rojo, rotulo: "Colgar") { m.colgar() }
                    .keyboardShortcut(.cancelAction)
            }
        }
        .padding(.horizontal, 58)
    }

    private func redondo(_ icono: String, fondo: Color, tinta: Color? = nil, rotulo: String,
                         accion: @escaping () -> Void) -> some View {
        VStack(spacing: 6) {
            Button(action: accion) {
                Image(systemName: icono).font(.system(size: 28)).foregroundStyle(tinta ?? texto)
                    .frame(width: 72, height: 72).background(fondo, in: Circle())
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(rotulo)
            Text(rotulo).font(.system(size: 13)).foregroundStyle(texto.opacity(0.7))
        }
    }
}

// MARK: - Sonar

extension Llamada {
    /// Las que estan sonando ahora, por mensaje (dos a la vez se apilan).
    @MainActor private static var sonando: [String: UIWindow] = [:]
    /// La ventana que tenia el teclado antes de la llamada.
    @MainActor private static weak var anterior: UIWindow?

    /// Si ya suena la de ese mensaje.
    @MainActor static func suena(chat: String, mensaje: String) -> Bool {
        sonando[chat + "\u{0}" + mensaje] != nil
    }

    /// **Hace sonar la llamada**: la tarjeta en el centro, encima de todo
    /// lo de la app. `contestada`: ya contestada (se abrio tocando la
    /// notificacion: el usuario ya dijo que si).
    @MainActor
    static func sonar(_ l: LlamadaSecreta, nucleo: Nucleo, detalle: String = "", texto: String? = nil,
                      contestada: Bool = false) {
        let clave = l.chat + "\u{0}" + l.mensaje
        guard sonando[clave] == nil, let escena = ArribaUIKit.escena() else { return }
        let m = ModeloLlamada(l, nucleo: nucleo, detalle: detalle, texto: texto)
        let v = UIWindow(windowScene: escena)
        v.windowLevel = UIWindow.Level(rawValue: UIWindow.Level.alert.rawValue + 2)
        v.backgroundColor = .clear
        let raiz = UIHostingController(rootView: VistaLlamada(m: m))
        raiz.view.backgroundColor = .clear
        v.rootViewController = raiz
        if sonando.isEmpty { anterior = escena.windows.first { $0.isKeyWindow } }
        m.cerrar = { [weak v] in
            v?.isHidden = true
            sonando[clave] = nil
            if sonando.isEmpty { anterior?.makeKey() }
        }
        sonando[clave] = v
        // La tecla (Intro, Esc) va a la llamada.
        v.makeKeyAndVisible()
        m.sonar()
        if contestada { m.contestar() }
    }
}
