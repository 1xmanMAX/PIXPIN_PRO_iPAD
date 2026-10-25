// Los pines de los tres relojes: cronometro, temporizador y alarma (lo que
// en Windows pinta `pines/herramienta.rs` con la vista de `mini_panel.rs`).
// Copiado del Mac tal cual, sin AppKit.
//
// Como las demas mini-apps, el documento es el del movil y Windows y cada
// toque lo reescribe el nucleo (`miniCronometroCambiar`...). Lo que se
// guarda es **el instante** (cuando arranco, cuando acaba), no el numero: el
// numero se calcula al pintar con la hora de la pared, asi que es correcto
// aunque el iPad se haya dormido o el pin se haya cerrado.
//
// Rendimiento: solo se repinta mientras corre (cada 100 ms el cronometro y
// cada 200 ms el temporizador, como el movil) y sin tocar disco; parado, un
// pin quieto no despierta a nadie.

import PixPinCore
import SwiftUI

/// Cada cuanto se repinta: lo pedido en marcha, y casi nunca parado.
private func cadencia(_ corriendo: Bool, _ segundos: TimeInterval) -> PeriodicTimelineSchedule {
    .periodic(from: .now, by: corriendo ? segundos : 3600)
}

/// El numero grande de los relojes.
private struct Tablero: View {
    let texto: String
    var alerta = false
    var apagado = false

    var body: some View {
        Text(texto)
            .font(.system(size: 44, weight: .semibold, design: .rounded))
            .monospacedDigit()
            .minimumScaleFactor(0.3)
            .lineLimit(1)
            .foregroundStyle(alerta ? Color.red : apagado ? Color.secondary : Color.primary)
            .frame(maxWidth: .infinity)
    }
}

struct VistaCronometroPin: View {
    @ObservedObject var doc: DocumentoMini

    var body: some View {
        let corriendo = miniCronometroLeer(documento: doc.documento, ahoraMs: milisegundos()).corriendo
        TimelineView(cadencia(corriendo, 0.1)) { contexto in
            let c = miniCronometroLeer(documento: doc.documento, ahoraMs: milisegundos(contexto.date))
            VStack(spacing: 8) {
                CabeceraMini(doc: doc)
                Tablero(texto: c.tablero)
                HStack(spacing: 10) {
                    Button(c.corriendo ? "Parar" : "Arrancar") { cambiar(.arrancarOParar) }
                        .keyboardShortcut(.space, modifiers: [])
                        .buttonStyle(.borderedProminent)
                    // Solo en marcha, como el movil; parado se ve para que se
                    // sepa que existe.
                    Button("Vuelta") { cambiar(.vuelta) }
                        .keyboardShortcut("v", modifiers: [])
                        .disabled(!c.corriendo)
                    Button("Reiniciar") { cambiar(.reiniciar) }
                        .disabled(!c.seReinicia)
                }
                // Del reves: la ultima vuelta arriba, que es la que se mira.
                ScrollView {
                    VStack(spacing: 2) {
                        ForEach(Array(c.vueltas.enumerated().reversed()), id: \.offset) { n, v in
                            HStack {
                                Text("\(n + 1)").foregroundStyle(.secondary)
                                Spacer()
                                Text(v).monospacedDigit()
                            }
                            .font(.callout)
                        }
                    }
                }
            }
            .padding(10)
        }
    }

    private func cambiar(_ accion: AccionCronometro) {
        let ahora = milisegundos()
        doc.cambiar { miniCronometroCambiar(documento: $0, accion: accion, ahoraMs: ahora) }
    }
}

struct VistaTemporizadorPin: View {
    @ObservedObject var doc: DocumentoMini
    @State private var tecleado = ""

    /// Las «pegatinas de tiempo» de Windows: en esos minutos y en marcha.
    private static let pegatinas: [Int64] = [1, 3, 5, 10, 15, 25, 30, 45, 60]

    var body: some View {
        let corriendo = miniTemporizadorLeer(documento: doc.documento, ahoraMs: milisegundos()).corriendo
        TimelineView(cadencia(corriendo, 0.2)) { contexto in
            let t = miniTemporizadorLeer(documento: doc.documento, ahoraMs: milisegundos(contexto.date))
            VStack(spacing: 8) {
                HStack {
                    CabeceraMini(doc: doc)
                    Menu {
                        ForEach(Self.pegatinas, id: \.self) { m in
                            Button("\(m) min") { cambiar(.enMarchaCon(minutos: m)) }
                        }
                    } label: {
                        Image(systemName: "timer")
                    }
                    .fixedSize()
                    .help("Poner en marcha con…")
                }
                // En rojo al llegar a cero (`MiniActivity.kt:446-453`).
                Tablero(texto: t.tablero, alerta: t.vencido)
                // Con el temporizador en marcha no se cambia lo que dura.
                HStack(spacing: 6) {
                    ForEach([Int64(-5), -1, 1, 5], id: \.self) { m in
                        Button(m < 0 ? "−\(-m)" : "+\(m)") { cambiar(.minutos(minutos: m)) }
                            .disabled(t.corriendo)
                    }
                }
                Button(t.corriendo ? "Parar" : "Arrancar") { cambiar(.arrancarOParar) }
                    .buttonStyle(.borderedProminent)
                    .tint(t.vencido ? .red : nil)
                    .disabled(t.duracion <= 0)
                Spacer(minLength: 0)
                TextField("Lo que dura: 25, 1:30…", text: $tecleado)
                    .textFieldStyle(.roundedBorder)
                    .disabled(t.corriendo)
                    .onSubmit { teclear() }
            }
            .padding(10)
        }
    }

    private func cambiar(_ accion: AccionTemporizador) {
        let ahora = milisegundos()
        doc.cambiar { miniTemporizadorCambiar(documento: $0, accion: accion, ahoraMs: ahora) }
    }

    /// «25» son minutos, «1:30» m:ss. Lo que no es un tiempo se queda en la
    /// caja para corregirlo.
    private func teclear() {
        guard let ms = miniDuracionTecleada(texto: tecleado) else { return avisarFalloPin() }
        tecleado = ""
        cambiar(.duracion(ms: ms))
    }
}

struct VistaAlarmaPin: View {
    @ObservedObject var doc: DocumentoMini
    @State private var tecleado = ""

    var body: some View {
        let a = miniAlarmaLeer(documento: doc.documento)
        VStack(spacing: 8) {
            CabeceraMini(doc: doc)
            Tablero(texto: a.tablero, apagado: !a.activa)
            Text(cuando(a))
                .font(.caption)
                .foregroundStyle(.secondary)
            HStack(spacing: 6) {
                Button("−1 h") { doc.cambiar { miniAlarmaCambiar(documento: $0, accion: .horas(horas: -1)) } }
                Button("+1 h") { doc.cambiar { miniAlarmaCambiar(documento: $0, accion: .horas(horas: 1)) } }
                Button("−5 min") { doc.cambiar { miniAlarmaCambiar(documento: $0, accion: .minutos(minutos: -5)) } }
                Button("+5 min") { doc.cambiar { miniAlarmaCambiar(documento: $0, accion: .minutos(minutos: 5)) } }
            }
            .font(.caption)
            if a.activa {
                Button("Apagar") { doc.cambiar { miniAlarmaCambiar(documento: $0, accion: .activar) } }
                    .buttonStyle(.borderedProminent)
            } else {
                Button("Encender") { doc.cambiar { miniAlarmaCambiar(documento: $0, accion: .activar) } }
            }
            Spacer(minLength: 0)
            TextField("Hora: 7:30, 1945…", text: $tecleado)
                .textFieldStyle(.roundedBorder)
                .onSubmit { teclear() }
        }
        .padding(10)
    }

    /// «Sonará hoy a las 7:30» o «mañana»: lo mismo que calcula el aviso.
    private func cuando(_ a: AlarmaMini) -> String {
        guard a.activa else { return "Apagada" }
        let ahora = Date()
        guard let ms = miniCuandoAvisa(miniapp: doc.miniapp, documento: doc.documento,
                                       ahoraMs: milisegundos(ahora), desfaseLocalMs: desfaseLocalMs(ahora))
        else { return "" }
        let fecha = fechaDe(ms: ms)
        return Calendar.current.isDateInToday(fecha) ? "Sonará hoy" : "Sonará mañana"
    }

    /// Teclear una hora la pone y la enciende (con los botones no).
    private func teclear() {
        guard let h = miniHoraTecleada(texto: tecleado) else { return avisarFalloPin() }
        tecleado = ""
        doc.cambiar { miniAlarmaCambiar(documento: $0, accion: .hora(hora: h.hora, minuto: h.minuto)) }
    }
}
