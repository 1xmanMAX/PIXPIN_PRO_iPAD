// **Lo comun de las ventanitas de la voz** (Mac `VozVentanas/Ventanita.swift`,
// Windows `ventanita.rs`): el teleprónter, Pronunciar, la conversacion por
// turnos y grabar una clase. Los colores de «papel oscuro», un boton plano,
// un aviso corto que se va solo y el microfono que graba un `.m4a` suelto.
//
// En el iPad no hay ventanas sueltas: cada ventanita sale **a pantalla
// completa encima de todo lo de la app** (`Ventanita.abrir`), siempre
// oscura, y se maneja con el dedo y, si hay teclado, con las mismas teclas
// que en el Mac (`.onKeyPress`).
//
// El microfono (`MicrofonoSuelto`) es lo de `GrabadoraVoz` sin el chat:
// abre la sesion de sonido de grabar (iOS), graba a un fichero con el
// nombre que le den, cuenta lo llevado contando muestras y al parar da la
// `NotaGrabada`. Se puede pausar (grabar una clase) y, si una llamada o
// Siri lo cortan, vuelve a grabar solo al acabar la interrupcion.

import AVFoundation
import PixPinCore
import SwiftUI
import UIKit

/// Los colores de las ventanitas (`ventanita.rs`): papel oscuro, no
/// interfaz, como el visor limpio.
enum ColoresVoz {
    static let fondo = Color(hex: 0x121316)
    static let texto = Color(hex: 0xE4E2E6)
    static let apagado = Color(hex: 0x9A9AA2)
    static let cristal = Color(hex: 0x26262C)
    static let rojo = Color(hex: 0xE05A4F)
    static let verde = Color(hex: 0x2FB344)
    static let azul = Color(hex: 0x1E88E5)
    static let dorado = Color(hex: 0xE8C06A)
    static let boton = Color(hex: 0x2C2C34)
    /// La franja de lectura del teleprónter.
    static let franja = Color(hex: 0x1D2733)

    static let fondoUIKit = UIColor(red: 0x12 / 255, green: 0x13 / 255, blue: 0x16 / 255, alpha: 1)
}

/// Un boton de las ventanitas: el rotulo centrado en una caja de radio 8;
/// `color` es el fondo si esta encendido (rojo de grabar, azul de empezar).
struct BotonVentanita: View {
    let rotulo: String
    var color: Color?
    var ancho: CGFloat?
    var alto: CGFloat = 44
    let accion: () -> Void

    init(_ rotulo: String, color: Color? = nil, ancho: CGFloat? = nil, alto: CGFloat = 44,
         accion: @escaping () -> Void) {
        self.rotulo = rotulo
        self.color = color
        self.ancho = ancho
        self.alto = alto
        self.accion = accion
    }

    var body: some View {
        Button(action: accion) {
            Text(rotulo)
                .font(.system(size: 16))
                .foregroundStyle(ColoresVoz.texto)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
                .padding(.horizontal, 12)
                .frame(minWidth: ancho ?? 44, maxWidth: ancho, minHeight: alto, maxHeight: alto)
                .background(color ?? ColoresVoz.boton, in: RoundedRectangle(cornerRadius: 8))
                .contentShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
    }
}

/// Un aviso corto abajo (la nota salio demasiado corta, no hay micro), que
/// se va solo.
@MainActor
final class AvisoCorto: ObservableObject {
    @Published private(set) var texto: String?
    private var quitar: DispatchWorkItem?

    func decir(_ t: String, segundos: TimeInterval = 4) {
        texto = t
        quitar?.cancel()
        let q = DispatchWorkItem { [weak self] in MainActor.assumeIsolated { self?.texto = nil } }
        quitar = q
        DispatchQueue.main.asyncAfter(deadline: .now() + segundos, execute: q)
    }
}

// MARK: - Abrir y lo de los ficheros

@MainActor
enum Ventanita {
    /// Las que estan abiertas ahora (una de cada a la vez, como en Windows).
    private static var abiertas: Set<String> = []

    static func abierta(_ nombre: String) -> Bool { abiertas.contains(nombre) }

    /// **Pone una ventanita a pantalla completa** encima de lo que haya. Su
    /// vista recibe con que cerrarse; `alCerrar` se llama una vez al irse
    /// (para soltar el microfono). `false` si ya estaba abierta o no hay
    /// donde ponerla.
    @discardableResult
    static func abrir<V: View>(_ nombre: String, alCerrar: @escaping @MainActor () -> Void,
                               @ViewBuilder _ vista: (@escaping () -> Void) -> V) -> Bool {
        guard !abiertas.contains(nombre) else { return false }
        var hecho = false
        let controlador = ArribaUIKit.presentar(estilo: .fullScreen) { cerrarHoja in
            vista {
                guard !hecho else { return }
                hecho = true
                abiertas.remove(nombre)
                alCerrar()
                cerrarHoja()
            }
            .preferredColorScheme(.dark)
            .statusBarHidden()
        }
        guard let controlador else { return false }
        controlador.view.backgroundColor = ColoresVoz.fondoUIKit
        controlador.overrideUserInterfaceStyle = .dark
        abiertas.insert(nombre)
        return true
    }

    /// La carpeta de lo que graban las ventanitas antes de pasar al chat.
    nonisolated static func carpeta(_ nombre: String) -> URL {
        let u = FileManager.default.temporaryDirectory.appendingPathComponent("PixPinVoz", isDirectory: true)
            .appendingPathComponent(nombre, isDirectory: true)
        try? FileManager.default.createDirectory(at: u, withIntermediateDirectories: true)
        return u
    }

    /// Milisegundos de ahora (para los nombres `lectura-<ms>.m4a`).
    nonisolated static var ahoraMs: Int64 { Int64((Date().timeIntervalSince1970 * 1000).rounded()) }
}

// MARK: - El microfono

/// **Un microfono que graba a un fichero** (`abrir_microfono` y
/// `cerrar_microfono` de Windows): para las ventanitas, que no graban al
/// chat sino a un `.m4a` suelto que luego entra con su nombre.
@MainActor
final class MicrofonoSuelto {
    private var motor: AVAudioEngine?
    private var escritor: EscritorVoz?
    private(set) var desde: Date?
    /// En pausa a proposito (no por una llamada).
    private(set) var enPausa = false
    private var vigia: NSObjectProtocol?

    var grabando: Bool { motor != nil }

    /// Lo llevado, contando muestras (lo que de verdad se grabo: la pausa
    /// no cuenta).
    var llevadoMs: Int64 { escritor?.llevadoMs ?? 0 }

    /// Pide el permiso (la primera vez sale el aviso del sistema) y empieza
    /// a grabar en `fichero`. Lanza si no hay permiso o microfono.
    func empezar(en fichero: URL) async throws {
        guard motor == nil else { return }
        guard await GrabadoraVoz.permisoMicrofono() else { throw ErrorVoz.sinPermisoMicrofono }
        guard GrabadoraVoz.compartida.estado == .quieta else { throw ErrorVoz.sinMicrofono }
        ReproductorVoz.compartido.pausar()
        let sesion = AVAudioSession.sharedInstance()
        try sesion.setCategory(.playAndRecord, mode: .default, options: [.defaultToSpeaker, .allowBluetooth])
        try sesion.setActive(true)
        let motor = AVAudioEngine()
        let entrada = motor.inputNode
        let formato = entrada.outputFormat(forBus: 0)
        guard formato.sampleRate > 0, formato.channelCount > 0 else {
            MicrofonoSuelto.soltarSesion()
            throw ErrorVoz.sinMicrofono
        }
        let escritor = try EscritorVoz(url: fichero, muestreoDeEntrada: formato.sampleRate)
        MicrofonoSuelto.instalarGrifo(en: entrada, formato: formato, escritor: escritor)
        motor.prepare()
        do {
            try motor.start()
        } catch {
            entrada.removeTap(onBus: 0)
            try? FileManager.default.removeItem(at: fichero)
            MicrofonoSuelto.soltarSesion()
            throw error
        }
        self.motor = motor
        self.escritor = escritor
        enPausa = false
        desde = Date()
        vigilarInterrupciones()
    }

    /// Para un rato sin cerrar el fichero (lo pausado no se graba).
    func pausar() {
        guard let motor, !enPausa else { return }
        motor.pause()
        enPausa = true
    }

    /// Sigue grabando tras `pausar`. `false` si el microfono no volvio.
    @discardableResult
    func seguir() -> Bool {
        guard let motor, enPausa else { return true }
        do {
            try AVAudioSession.sharedInstance().setActive(true)
            try motor.start()
            enPausa = false
            return true
        } catch {
            return false
        }
    }

    /// Para y cierra el fichero (fuera del hilo principal: escribir el
    /// indice del MPEG-4 tarda). `nil` si no grababa.
    func parar() async -> NotaGrabada? {
        guard let motor, let escritor else { return nil }
        motor.inputNode.removeTap(onBus: 0)
        motor.stop()
        self.motor = nil
        self.escritor = nil
        desde = nil
        enPausa = false
        dejarDeVigilar()
        MicrofonoSuelto.soltarSesion()
        return await Task.detached(priority: .userInitiated) { escritor.terminar() }.value
    }

    /// Para y tira lo grabado: no se guarda a escondidas.
    func cancelar() {
        guard let motor, let escritor else { return }
        motor.inputNode.removeTap(onBus: 0)
        motor.stop()
        self.motor = nil
        self.escritor = nil
        desde = nil
        enPausa = false
        dejarDeVigilar()
        MicrofonoSuelto.soltarSesion()
        try? FileManager.default.removeItem(at: escritor.terminar().url)
    }

    /// Una llamada o Siri paran el motor; al acabar se sigue grabando (si
    /// no estaba en pausa a proposito).
    private func vigilarInterrupciones() {
        dejarDeVigilar()
        vigia = NotificationCenter.default.addObserver(
            forName: AVAudioSession.interruptionNotification, object: AVAudioSession.sharedInstance(), queue: .main
        ) { [weak self] aviso in
            let tipo = (aviso.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt)
                .flatMap(AVAudioSession.InterruptionType.init(rawValue:))
            MainActor.assumeIsolated {
                guard let self, let motor = self.motor, tipo == .ended, !self.enPausa else { return }
                try? AVAudioSession.sharedInstance().setActive(true)
                try? motor.start()
            }
        }
    }

    private func dejarDeVigilar() {
        if let vigia { NotificationCenter.default.removeObserver(vigia) }
        vigia = nil
    }

    private nonisolated static func instalarGrifo(en nodo: AVAudioInputNode, formato: AVAudioFormat,
                                                   escritor: EscritorVoz) {
        nodo.installTap(onBus: 0, bufferSize: 4096, format: formato) { bloque, _ in
            try? escritor.escribir(bloque)
        }
    }

    /// La sesion vuelve a la de escuchar (para oir la toma al momento).
    private static func soltarSesion() {
        let sesion = AVAudioSession.sharedInstance()
        try? sesion.setCategory(.playback, mode: .spokenAudio)
        try? sesion.setActive(true)
    }

    /// Lo que se dice cuando no se puede grabar.
    static func explicar(_ error: Error) -> String {
        switch error as? ErrorVoz {
        case .sinPermisoMicrofono?:
            return "PixPin no tiene permiso para el micrófono (Ajustes > Privacidad y seguridad > Micrófono)"
        case .sinMicrofono?:
            return "El micrófono está ocupado o no hay ninguno"
        default:
            return "No hay micrófono disponible"
        }
    }
}

// MARK: - Las teclas

/// Las teclas de las ventanitas, con el teclado del iPad (`KeyPress`).
enum TeclasVoz {
    /// La letra de la tecla, en minuscula («+», «-», «r», «1»).
    static func letra(_ k: KeyPress) -> Character? {
        k.characters.lowercased().first
    }

    static func esIntro(_ k: KeyPress) -> Bool { k.key == .return }

    /// ⌘ o ⌃ (los dos sirven para «guardar»).
    static func conMando(_ k: KeyPress) -> Bool {
        k.modifiers.contains(.command) || k.modifiers.contains(.control)
    }
}

extension View {
    /// El teclado de una ventanita: la vista toma el foco al aparecer y le
    /// llegan las teclas (bajar, repetir y soltar). `teclear` dice si la uso.
    func tecladoDeVentanita(_ foco: FocusState<Bool>.Binding,
                            _ teclear: @escaping (KeyPress) -> Bool) -> some View {
        focusable()
            .focused(foco)
            .focusEffectDisabled()
            .onKeyPress(phases: .all) { k in teclear(k) ? .handled : .ignored }
            .onAppear { foco.wrappedValue = true }
    }
}
