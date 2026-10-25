// «¿Qué sincronizar?», la lista de elegir chats antes de una vuelta con un
// aparato: `sincronizar/elegir.rs` de Windows (copia de `Eligiendo` del
// movil). Dos pestanas, lo de este aparato y lo del otro; el punto verde es
// lo que esta en los dos y el rojo lo que solo esta en uno y se copiara al
// otro; debajo, cual de las dos versiones es la mas reciente. Abajo, «Todo»,
// «Nada» y «Sincronizar N».
//
// **Antes de borrar, se pregunta aqui**: lo que el otro borro sale con su
// casilla (sin marcar) y el aviso «SE BORRARÁ AQUÍ», y solo se borra si se
// deja marcado; lo que se borro aqui y se borrara alli se dice en rojo. Y el
// interruptor «Juntar / Lo mío manda». La cuenta de que se borra es del
// nucleo (`pixpin_sincro::vuelta`); aqui solo se ensena.

import PixPinCore
import SwiftUI

/// Lo que se va marcando en la lista: puro, para probarlo sin pantalla.
public struct Eligiendo: Equatable {
    public let pregunta: PreguntaSincro
    public var marcados: [Bool]
    public var loMioManda: Bool

    public init(_ pregunta: PreguntaSincro) {
        self.pregunta = pregunta
        marcados = pregunta.filas.map(\.marcado)
        loMioManda = pregunta.loMioManda
    }

    public var cuantos: Int { marcados.filter { $0 }.count }

    public mutating func todo() { marcados = marcados.map { _ in true } }
    public mutating func nada() { marcados = marcados.map { _ in false } }

    /// Lo que se contesta al pulsar «Sincronizar N» (nada si no hay ninguno).
    public var eleccion: EleccionSincro? {
        guard cuantos > 0 else { return nil }
        let elegidos = zip(pregunta.filas, marcados).filter { $0.1 }.map { $0.0.id }
        return EleccionSincro(elegidos: elegidos, loMioManda: loMioManda)
    }

    /// Lo que se lee bajo el nombre de una fila y si va en rojo, verde o
    /// gris (`estado_de_fila`): una fila que el otro borro lo dice siempre,
    /// en las dos pestanas.
    public func estado(_ i: Int, esMio: Bool) -> (texto: String, color: ColorDeEstado) {
        let f = pregunta.filas[i]
        let otro = pregunta.otro
        if f.seBorraAqui {
            if loMioManda { return ("Borrado en \(otro) · se le vuelve a mandar", .verde) }
            if marcados[i] { return ("Borrado en \(otro) · SE BORRARÁ AQUÍ (queda copia)", .rojo) }
            return ("Borrado en \(otro) · sin marcar: se queda aquí", .gris)
        }
        return esMio ? (f.estadoAqui, f.recienteAqui ? .verde : .gris)
            : (f.estadoAlli, f.recienteAlli ? .verde : .gris)
    }

    /// El aviso en rojo de lo que se va a borrar, si hay algo. Con «Lo mío
    /// manda» no se borra nada aqui (`aviso_de_borrados`).
    public var avisoDeBorrados: String? {
        guard !loMioManda else { return nil }
        let lista = { (v: [String]) in v.map { "«\($0)»" }.joined(separator: ", ") }
        var partes: [String] = []
        let aqui = zip(pregunta.filas, marcados).filter { $0.0.seBorraAqui && $0.1 }.map { $0.0.nombre }
        if !aqui.isEmpty {
            partes.append("Se borrará AQUÍ, porque se borró en \(pregunta.otro): \(lista(aqui)). "
                + "Desmárcalo para dejarlo, o elige «Lo mío manda» para volver a mandárselo.")
        }
        if !pregunta.seBorranAlli.isEmpty {
            partes.append("Se borrará en \(pregunta.otro), porque lo borraste aquí: "
                + "\(lista(pregunta.seBorranAlli)).")
        }
        return partes.isEmpty ? nil : partes.joined(separator: "\n")
    }

    /// Como se junta (`sinc-elegir-como` y `sincro-lo-mio-manda-como`).
    public var explicacion: String {
        loMioManda
            ? "De una sola dirección: lo de \(pregunta.yo) pisa a \(pregunta.otro). Aquí no se borra ni "
            + "se cambia nada; allí, lo que sea distinto queda como aquí y lo que borraron vuelve. Lo "
            + "que solo tenga \(pregunta.otro) se conserva. Queda copia de lo pisado."
            : "Lo que cambió en los dos se junta: figura a figura, celda a celda, párrafo a párrafo. Si "
            + "los dos tocaron lo mismo, gana el último cambio, y lo de antes queda en «Copias de "
            + "seguridad»."
    }

    /// «Sincronizar N», o «Mandar N» con «Lo mío manda».
    public var boton: String { loMioManda ? "Mandar \(cuantos)" : "Sincronizar \(cuantos)" }
}

public enum ColorDeEstado: Sendable {
    case verde, rojo, gris
}

/// La hoja de «¿Qué sincronizar?». Cerrarla sin contestar es cancelar.
public struct VistaElegirSincro: View {
    let pendiente: PreguntaPendiente
    @State private var el: Eligiendo
    @State private var pestana = 0
    @Environment(\.dismiss) private var cerrar

    public init(_ pendiente: PreguntaPendiente) {
        self.pendiente = pendiente
        _el = State(initialValue: Eligiendo(pendiente.pregunta))
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("¿Qué sincronizar?").font(.title2.weight(.semibold))
            Text("Con \(el.pregunta.otro). Lo marcado queda igual en los dos.")
                .foregroundStyle(.secondary)
            HStack(spacing: 14) {
                Label { Text("En los dos") } icon: { Circle().fill(.green).frame(width: 8, height: 8) }
                Label { Text("Solo en uno") } icon: { Circle().fill(.red).frame(width: 8, height: 8) }
            }
            .font(.caption)

            Picker("", selection: $pestana) {
                Text("\(el.pregunta.yo) · \(el.pregunta.filas.filter(\.aqui).count)").tag(0)
                Text("\(el.pregunta.otro) · \(el.pregunta.filas.filter(\.alli).count)").tag(1)
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            List {
                ForEach(Array(el.pregunta.filas.enumerated()), id: \.element.id) { i, f in
                    if pestana == 0 ? f.aqui : f.alli {
                        fila(i, f)
                    }
                }
            }
            .frame(minHeight: 200)

            Picker("", selection: $el.loMioManda) {
                Text("Juntar").tag(false)
                Text("Lo mío manda").tag(true)
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            if let aviso = el.avisoDeBorrados {
                Text(aviso).font(.callout).foregroundStyle(.red).fixedSize(horizontal: false, vertical: true)
            }
            Text(el.explicacion).font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            HStack {
                Button("Todo") { el.todo() }
                Button("Nada") { el.nada() }
                Spacer()
                Button("Cancelar", role: .cancel) { contestar(nil) }
                    .keyboardShortcut(.cancelAction)
                Button(el.boton) { contestar(el.eleccion) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(el.cuantos == 0)
            }
        }
        .padding(18)
        .frame(minWidth: 460, idealWidth: 520, minHeight: 520)
        // Cerrar de cualquier otro modo es cancelar: el hilo de la vuelta
        // no puede quedarse esperando.
        .onDisappear { pendiente.contestar(nil) }
    }

    private func fila(_ i: Int, _ f: FilaElegir) -> some View {
        let (texto, color) = el.estado(i, esMio: pestana == 0)
        return Toggle(isOn: $el.marcados[i]) {
            HStack(spacing: 8) {
                Circle().fill(f.aqui && f.alli ? Color.green : Color.red).frame(width: 8, height: 8)
                VStack(alignment: .leading, spacing: 2) {
                    Text(f.nombre).lineLimit(1)
                    Text(texto).font(.caption).lineLimit(2)
                        .foregroundStyle(color == .verde ? Color.green : color == .rojo ? Color.red : Color.secondary)
                }
            }
        }
        .toggleStyle(EstiloCasilla())
    }

    private func contestar(_ e: EleccionSincro?) {
        pendiente.contestar(e)
        cerrar()
    }
}

/// La casilla de Windows (`.checkbox` es solo del Mac): el cuadro a la
/// izquierda y toda la fila se toca.
struct EstiloCasilla: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        Button {
            configuration.isOn.toggle()
        } label: {
            HStack(spacing: 10) {
                Image(systemName: configuration.isOn ? "checkmark.square.fill" : "square")
                    .font(.title3)
                    .foregroundStyle(configuration.isOn ? Color.accentColor : Color.secondary)
                configuration.label
                Spacer(minLength: 0)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
