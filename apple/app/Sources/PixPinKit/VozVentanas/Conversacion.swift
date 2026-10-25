// **Una conversacion de varias personas, grabada por turnos y pasada a
// texto con nombres** (Windows `conversacion.rs` y `voz_en_chat.rs`, Mac
// `VozVentanas/Conversacion.swift`, movil `ConversacionActivity.kt`).
//
// Se dice cuantos hablan (2 a 9) y como se llaman, y cada uno dice cuando
// le toca: asi se sabe quien dijo cada cosa sin adivinarlo por la voz. Se
// graba **sin parar** (un solo `.m4a`) y se toca la ficha de quien habla
// desde ese momento (o, con teclado, **1..9**; **0** es «no habla nadie», que
// no sale en el texto).
//
// Al terminar entra en el chat como `VOZ` con `turnos` (los campos del
// movil, `turnosDeLaConversacion` del nucleo) y se pasa a texto turno a
// turno: `[0:12] **Pepe:** …`.

import PixPinCore
import SwiftUI
import UIKit

enum ConversacionPorTurnos {
    /// El nombre de la ventanita (una a la vez).
    static let nombreVentanita = "conversacion"
    /// Cuantos pueden hablar: del 2 al 9, una tecla por persona.
    static let maximo = 9

    /// Los colores del movil (`COLORES`), y tres mas para llegar a nueve.
    static let colores: [UInt32] = [0x1E88E5, 0xE53935, 0x43A047, 0xFB8C00, 0x8E24AA, 0x00897B, 0x6D4C41,
                                    0xD81B60, 0x3949AB]

    /// Como se llama la persona `i`: lo escrito, o «Persona 2».
    static func nombre(_ nombres: [String], _ i: Int) -> String {
        let n = nombres.indices.contains(i) ? nombres[i].trimmingCharacters(in: .whitespaces) : ""
        return n.isEmpty ? "Persona \(i + 1)" : n
    }

    /// Lo que hace una tecla grabando: 1..9 una persona y 0 nadie. `nil`: no
    /// es de las suyas.
    enum Tecla: Equatable { case persona(Int), nadie }

    static func tecla(_ letra: Character?) -> Tecla? {
        guard let letra, letra.isASCII, let n = letra.wholeNumberValue else { return nil }
        return n == 0 ? .nadie : .persona(n - 1)
    }
}

@MainActor
final class ModeloConversacion: ObservableObject {
    enum Paso { case preparar, grabar }

    @Published var paso = Paso.preparar
    @Published var cuantos = 2
    @Published var nombres = Array(repeating: "", count: ConversacionPorTurnos.maximo)
    @Published private(set) var hablando: Int?
    @Published private(set) var abriendo = false
    /// `(milisegundo, quien)` de cada cambio; `quien` vacio = nadie.
    @Published private(set) var cambios: [CambioDeTurno] = []
    let aviso = AvisoCorto()
    private let mic = MicrofonoSuelto()
    var alTerminar: ((NotaGrabada, [TurnoVoz]) -> Void)?
    var cerrar: (() -> Void)?

    var llevadoMs: Int64 { mic.llevadoMs }
    var desde: Date? { mic.desde }

    func menos() { cuantos = max(cuantos - 1, 2) }
    func mas() { cuantos = min(cuantos + 1, ConversacionPorTurnos.maximo) }

    /// Empieza a grabar.
    func empezar() {
        guard paso == .preparar, !abriendo else { return }
        abriendo = true
        let fichero = Ventanita.carpeta("conversacion").appendingPathComponent("conversacion-\(Ventanita.ahoraMs).m4a")
        _ = Task<Void, Never> {
            do {
                try await mic.empezar(en: fichero)
                cambios = []
                hablando = nil
                paso = .grabar
                UIApplication.shared.isIdleTimerDisabled = true
            } catch {
                aviso.decir(MicrofonoSuelto.explicar(error))
            }
            abriendo = false
        }
    }

    /// Desde ahora habla `quien` (o nadie). Tocar otra vez al que habla es
    /// «nadie».
    func cambiar(_ quien: Int?) {
        guard paso == .grabar, mic.grabando else { return }
        let nuevo = quien == hablando ? nil : quien
        if let nuevo, nuevo >= cuantos { return }
        guard nuevo != hablando else { return }
        let nombre = nuevo.map { ConversacionPorTurnos.nombre(nombres, $0) } ?? ""
        cambios.append(CambioDeTurno(ms: mic.llevadoMs, quien: nombre))
        hablando = nuevo
    }

    /// Cuantos turnos van (para el pie): los cambios a alguien.
    var cuantosTurnos: Int { cambios.filter { !$0.quien.isEmpty }.count }

    func terminar() {
        guard paso == .grabar else { return }
        let cambios = self.cambios
        _ = Task<Void, Never> {
            guard let nota = await mic.parar() else { return }
            UIApplication.shared.isIdleTimerDisabled = false
            guard nota.duracionMs >= FormatoVoz.minimoMs else {
                try? FileManager.default.removeItem(at: nota.url)
                aviso.decir("La conversación salió demasiado corta", segundos: 3)
                paso = .preparar
                return
            }
            alTerminar?(nota, turnosDeLaConversacion(cambios: cambios, finMs: nota.duracionMs))
            cerrar?()
        }
    }

    /// Cerrar sin terminar tira lo grabado: no se guarda a escondidas.
    func soltar() {
        mic.cancelar()
        UIApplication.shared.isIdleTimerDisabled = false
    }

    func tecla(_ k: KeyPress) -> Bool {
        guard k.phase == .down else { return false }
        if k.key == .escape { cerrar?(); return true }
        switch paso {
        case .preparar:
            // Tab, Intro y las letras los lleva la caja del nombre.
            return false
        case .grabar:
            if TeclasVoz.esIntro(k) { terminar(); return true }
            switch ConversacionPorTurnos.tecla(TeclasVoz.letra(k)) {
            case let .persona(i)?:
                if hablando != i { cambiar(i) }
            case .nadie?:
                if hablando != nil { cambiar(nil) }
            case nil: return false
            }
            return true
        }
    }
}

struct VistaConversacion: View {
    @ObservedObject var m: ModeloConversacion
    @ObservedObject private var aviso: AvisoCorto
    @FocusState private var escribiendo: Int?
    @FocusState private var foco: Bool

    init(m: ModeloConversacion) {
        self.m = m
        aviso = m.aviso
    }

    var body: some View {
        Group {
            switch m.paso {
            case .preparar: preparar
            case .grabar: grabar.tecladoDeVentanita($foco) { k in m.tecla(k) }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(ColoresVoz.fondo.ignoresSafeArea())
    }

    private var preparar: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Conversación").font(.system(size: 26)).foregroundStyle(ColoresVoz.texto).padding(.top, 36)
            Text("Se graba sin parar; toca la ficha de quien empieza a hablar (o pulsa su número, 1 a 9) y otra vez cuando no hable nadie (0). La transcripción sale con los nombres.")
                .font(.system(size: 15)).foregroundStyle(ColoresVoz.apagado).padding(.top, 14)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                Text("¿Cuántas personas hablan?").font(.system(size: 17)).foregroundStyle(ColoresVoz.texto)
                Spacer()
                BotonVentanita("−", ancho: 44) { m.menos() }.accessibilityLabel("Una menos")
                Text("\(m.cuantos)").font(.system(size: 19)).foregroundStyle(ColoresVoz.texto).frame(width: 40)
                BotonVentanita("+", ancho: 44) { m.mas() }.accessibilityLabel("Una más")
            }
            .padding(.top, 24)
            ScrollView {
                let columnas = m.cuantos > 5 ? 2 : 1
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: columnas), spacing: 8) {
                    ForEach(0..<m.cuantos, id: \.self) { i in caja(i) }
                }
                .padding(.top, 14)
            }
            if let t = aviso.texto {
                Text(t).font(.system(size: 14)).foregroundStyle(ColoresVoz.dorado).padding(.bottom, 8)
            }
            HStack {
                BotonVentanita("Cancelar", ancho: 140, alto: 48) { m.cerrar?() }
                Spacer()
                BotonVentanita(m.abriendo ? "Abriendo el micrófono…" : "Empezar a grabar", color: ColoresVoz.azul,
                               ancho: 240, alto: 48) { m.empezar() }
            }
            .padding(.bottom, 26)
        }
        .padding(.horizontal, 40)
        .frame(maxWidth: 820)
        .onAppear { escribiendo = 0 }
        .onKeyPress(.escape) {
            m.cerrar?()
            return .handled
        }
    }

    private func caja(_ i: Int) -> some View {
        HStack(spacing: 10) {
            RoundedRectangle(cornerRadius: 3).fill(Color(hex: ConversacionPorTurnos.colores[i])).frame(width: 6)
            Text("\(i + 1)").font(.system(size: 15)).foregroundStyle(ColoresVoz.apagado)
            TextField("", text: Binding(get: { m.nombres[i] }, set: { m.nombres[i] = String($0.prefix(30)) }),
                      prompt: Text(ConversacionPorTurnos.nombre([], i)).foregroundStyle(ColoresVoz.apagado))
                .textFieldStyle(.plain).font(.system(size: 16)).foregroundStyle(ColoresVoz.texto)
                .textInputAutocapitalization(.words)
                .autocorrectionDisabled()
                .focused($escribiendo, equals: i)
                .submitLabel(i + 1 < m.cuantos ? .next : .go)
                .onSubmit {
                    if i + 1 < m.cuantos { escribiendo = i + 1 } else { m.empezar() }
                }
        }
        .padding(.trailing, 10)
        .frame(height: 44)
        .background(escribiendo == i ? Color(hex: 0x2E3440) : ColoresVoz.cristal, in: RoundedRectangle(cornerRadius: 8))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .onTapGesture { escribiendo = i }
    }

    private var grabar: some View {
        let columnas = m.cuantos <= 2 ? m.cuantos : 3
        return VStack(spacing: 0) {
            ZStack {
                HStack {
                    if let desde = m.desde {
                        TimelineView(.periodic(from: desde, by: 1)) { _ in
                            Text("● \(FormatoVoz.marca(ms: m.llevadoMs))").font(.system(size: 18).monospacedDigit())
                                .foregroundStyle(ColoresVoz.rojo)
                        }
                    }
                    Spacer()
                }
                Text("Toca a quien habla · otra vez: nadie").font(.system(size: 14))
                    .foregroundStyle(ColoresVoz.apagado)
            }
            .padding(.horizontal, 20).frame(height: 56)
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: columnas), spacing: 12) {
                ForEach(0..<m.cuantos, id: \.self) { i in ficha(i) }
            }
            .padding(.horizontal, 18)
            .frame(maxHeight: .infinity)
            HStack {
                Text("\(m.cuantosTurnos) turnos").font(.system(size: 14)).foregroundStyle(ColoresVoz.apagado)
                if let t = aviso.texto {
                    Text(t).font(.system(size: 14)).foregroundStyle(ColoresVoz.dorado)
                }
                Spacer()
                BotonVentanita("Tirar", ancho: 120, alto: 48) { m.cerrar?() }
                BotonVentanita("Terminar y transcribir", color: ColoresVoz.rojo, ancho: 240, alto: 48) { m.terminar() }
            }
            .padding(.horizontal, 20).frame(height: 80)
        }
    }

    private func ficha(_ i: Int) -> some View {
        let activo = m.hablando == i
        let opacidad = activo ? 1 : m.hablando != nil ? 0.25 : 0.55
        return Button { m.cambiar(i) } label: {
            VStack(spacing: 8) {
                Text("\(i + 1)").font(.system(size: 30))
                Text(ConversacionPorTurnos.nombre(m.nombres, i)).font(.system(size: 20)).lineLimit(1)
                Text(activo ? "Hablando…" : " ").font(.system(size: 13))
            }
            .foregroundStyle(ColoresVoz.texto)
            .frame(maxWidth: .infinity, minHeight: m.cuantos > 6 ? 120 : 160)
            .background(Color(hex: ConversacionPorTurnos.colores[i], alfa: opacidad), in: RoundedRectangle(cornerRadius: 18))
            .contentShape(RoundedRectangle(cornerRadius: 18))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(ConversacionPorTurnos.nombre(m.nombres, i))\(activo ? ", hablando" : "")")
    }
}

extension ConversacionPorTurnos {
    @MainActor static var abierto: Bool { Ventanita.abierta(nombreVentanita) }

    /// **Abre la conversacion por turnos.** La grabacion llega por
    /// `alTerminar` con sus turnos; cerrar sin terminar no manda nada.
    @MainActor
    static func abrir(alTerminar: @escaping (NotaGrabada, [TurnoVoz]) -> Void) {
        guard !abierto else { return }
        let m = ModeloConversacion()
        m.alTerminar = alTerminar
        Ventanita.abrir(nombreVentanita, alCerrar: { m.soltar() }) { cerrar in
            VistaConversacion(m: m).onAppear { m.cerrar = cerrar }
        }
    }
}
