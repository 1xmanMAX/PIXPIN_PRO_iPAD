// La pila de capturas (`pixpin-pila` + `pila_capturas.rs` de Windows,
// `Captura/PilaCapturas.swift` del Mac): las capturas que se copian seguidas
// se apilan en una tanda y un solo pegar las pega todas. Tras cada copia
// sale un recuadrito abajo a la izquierda de la capa de pines con la ultima
// y cuantas van; un toque lo ARMA (borde azul) y desde ahi todas se suman
// sin plazo hasta otro toque; sin armar, la tanda se cierra sola a los 10 s
// de la ultima (`apilar_segundos` de Windows) y el recuadro se va.
// Mantener pulsado: copiar todas, pinear la ultima, quitar.
//
// Como en Windows, cada captura de la pila se escribe como PNG en
// `capturas/` (nombre de la hora, la galeria): el portapapeles lleva los
// FICHEROS (se leen solo si la app que pega los pide), asi que la tanda no
// ocupa memoria. Tope de 24 por tanda: la mas vieja sale por abajo.

import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// La logica de la tanda, sin pantalla ni disco (la prueban las pruebas).
struct TandaPila: Equatable {
    /// Cuantas caben (`TOPE_CAPTURAS` de Windows).
    static let tope = 24
    /// Segundos desde la ultima en los que la siguiente se apila.
    static let segundosParaApilar: Double = 10

    private(set) var ficheros: [URL] = []
    private(set) var ultima: Double?
    /// Armada: no se cierra sola.
    var armada = false

    var cuantas: Int { ficheros.count }

    /// Si la tanda sigue abierta en `ahora` (segundos).
    func abierta(en ahora: Double) -> Bool {
        guard !ficheros.isEmpty, let ultima else { return false }
        return armada || ahora - ultima <= TandaPila.segundosParaApilar
    }

    /// Mete una captura. Devuelve si empezo una tanda nueva.
    @discardableResult
    mutating func meter(_ fichero: URL, en ahora: Double) -> Bool {
        let nueva = !abierta(en: ahora)
        if nueva {
            ficheros = []
            armada = false
        }
        ficheros.append(fichero)
        if ficheros.count > TandaPila.tope { ficheros.removeFirst(ficheros.count - TandaPila.tope) }
        ultima = ahora
        return nueva
    }

    mutating func vaciar() {
        ficheros = []
        ultima = nil
        armada = false
    }

    /// El titulo del recuadro, con los textos de Windows (`pila-titulo-*`).
    static func titulo(_ cuantas: Int) -> String {
        cuantas == 1 ? "1 captura" : "\(cuantas) capturas apiladas"
    }
}

/// La pila de la app: la tanda, la miniatura de la ultima y si se ve.
@MainActor
final class PilaCapturasIPad: ObservableObject {
    static let compartida = PilaCapturasIPad()

    @Published private(set) var tanda = TandaPila()
    /// La ultima, pequena, para el recuadro.
    @Published private(set) var miniatura: CGImage?
    /// Si se ve el recuadro.
    @Published private(set) var visible = false
    private var cerrarSola: Task<Void, Never>?

    private var ahora: Double { Date().timeIntervalSinceReferenceDate }

    /// Copia una captura apilandola: al portapapeles enseguida (ella sola
    /// si la tanda es nueva) y, ya guardada en la galeria, la tanda entera.
    func copiar(_ imagen: CGImage) {
        UIPasteboard.general.image = UIImage(cgImage: imagen)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        miniatura = Miniatura.reducir(imagen, ladoMaximo: 360)
        Task {
            guard let fichero = await CapturadorIPad.compartido.guardarEnGaleria(imagen) else { return }
            tanda.meter(fichero, en: ahora)
            visible = true
            if tanda.cuantas > 1 { ponerEnPortapapeles(tanda.ficheros) }
            programarCierre()
        }
    }

    /// Un toque: la arma (o la desarma).
    func alternarArmada() {
        tanda.armada.toggle()
        programarCierre()
    }

    func copiarTodas() {
        guard !tanda.ficheros.isEmpty else { return }
        ponerEnPortapapeles(tanda.ficheros)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    /// Pinea la ultima de la tanda.
    func pinearUltima() {
        guard let url = tanda.ficheros.last else { return }
        GestorPinesIPad.compartido.pinear(url: url)
    }

    /// Abre la ultima en el editor de captura.
    func editarUltima() {
        guard let url = tanda.ficheros.last else { return }
        Task {
            guard let foto = await Miniatura.fueraDelPrincipal({ Miniatura.de(url: url, ladoMaximo: 8000) })
            else { return avisarFalloPin() }
            CapturadorIPad.compartido.editar(foto)
        }
    }

    /// Quitar: se vacia y se va (las capturas siguen en la galeria).
    func quitar() {
        cerrarSola?.cancel()
        tanda.vaciar()
        miniatura = nil
        visible = false
    }

    private func ponerEnPortapapeles(_ ficheros: [URL]) {
        let proveedores = ficheros.compactMap { NSItemProvider(contentsOf: $0) }
        guard !proveedores.isEmpty else { return }
        UIPasteboard.general.setItemProviders(proveedores, localOnly: false, expirationDate: nil)
    }

    private func programarCierre() {
        cerrarSola?.cancel()
        guard !tanda.armada else { return }
        cerrarSola = Task { [weak self] in
            try? await Task.sleep(for: .seconds(TandaPila.segundosParaApilar))
            guard !Task.isCancelled, let self else { return }
            if !self.tanda.abierta(en: self.ahora) {
                withAnimation(.snappy) { self.visible = false }
            }
        }
    }
}

/// El recuadro de la esquina (abajo a la izquierda de la capa de pines).
struct VistaPilaCapturas: View {
    @ObservedObject var pila: PilaCapturasIPad

    var body: some View {
        if pila.visible, let mini = pila.miniatura {
            VStack(alignment: .leading, spacing: 6) {
                ZStack {
                    // Las de debajo asoman, como una pila de fotos.
                    if pila.tanda.cuantas > 1 {
                        RoundedRectangle(cornerRadius: 6).fill(Paleta.v2.tarjetaEncima)
                            .offset(x: 6, y: -6)
                    }
                    Image(decorative: mini, scale: 1)
                        .resizable()
                        .scaledToFill()
                        .frame(width: 128, height: 80)
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                }
                .frame(width: 128, height: 80)
                HStack(spacing: 6) {
                    Text(TandaPila.titulo(pila.tanda.cuantas))
                        .font(.caption.weight(.semibold))
                        .lineLimit(1)
                    Spacer(minLength: 0)
                    Button { withAnimation(.snappy) { pila.quitar() } } label: {
                        Image(systemName: "xmark").font(.caption2.weight(.bold)).frame(width: 22, height: 22)
                    }
                    .buttonStyle(.plain)
                    .help("Quitar")
                }
            }
            .padding(8)
            .frame(width: 144)
            .foregroundStyle(Paleta.v2.texto)
            .background(Paleta.v2.fondo.opacity(0.96), in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12)
                .strokeBorder(pila.tanda.armada ? Paleta.v2.acento : Paleta.v2.linea, lineWidth: pila.tanda.armada ? 2 : 1))
            .shadow(color: .black.opacity(0.35), radius: 10, y: 2)
            .environment(\.colorScheme, .dark)
            .contentShape(RoundedRectangle(cornerRadius: 12))
            .onTapGesture { pila.alternarArmada() }
            .contextMenu {
                Button { pila.copiarTodas() } label: { Label("Copiar todas", systemImage: "doc.on.doc") }
                Button { pila.pinearUltima() } label: { Label("Pinear", systemImage: "pin") }
                Button { pila.editarUltima() } label: { Label("Anotar", systemImage: "pencil") }
                Button { pila.alternarArmada() } label: {
                    Label(pila.tanda.armada ? "Dejar de apilar" : "Seguir apilando", systemImage: "square.stack")
                }
                Button(role: .destructive) { pila.quitar() } label: { Label("Quitar", systemImage: "xmark") }
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(TandaPila.titulo(pila.tanda.cuantas))
            .transition(.move(edge: .leading).combined(with: .opacity))
        }
    }
}
