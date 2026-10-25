// **El reproductor flotante dentro de la app** (Windows
// `ventana_chat/reproductor_flotante.rs`): si algo suena y no se ve la barra
// del chat (se fue a la galeria, a las tareas o a un visor a pantalla
// completa), sale la misma barra flotando abajo a la izquierda, encima de
// todo lo de la app; se arrastra a donde estorbe menos y se va al parar.
//
// ⛔ iOS no deja nada encima de otras apps: fuera de PixPin lo que suena se
// maneja con «Ahora suena» del Centro de control y la pantalla bloqueada
// (`MandosDelSistema`), y sigue sonando de fondo (el modo `audio`).
//
// Es una `UIWindow` propia (la `VentanaDelAviso` de los avisos, que solo se
// queda los toques de lo suyo) que solo existe mientras hay algo cargado y
// el chat (que tiene su barra) no esta delante.

import Combine
import SwiftUI
import UIKit

@MainActor
final class ReproductorFlotante: ObservableObject {
    static let compartido = ReproductorFlotante()

    /// Donde la dejo el dedo (desde su sitio de siempre).
    @Published var desplazado = CGSize.zero
    private var ventana: UIWindow?
    private var vigias: Set<AnyCancellable> = []

    /// **Si debe salir**: algo cargado y el chat (con su barra) no delante:
    /// otra seccion, ningun chat abierto o un visor encima.
    nonisolated static func sale(hayAlgo: Bool, enProyectos: Bool, conChat: Bool, conVisor: Bool) -> Bool {
        hayAlgo && (!enProyectos || !conChat || conVisor)
    }

    /// Empieza a mirar lo que suena (una vez; repetirlo no hace nada).
    func vigilar() {
        guard vigias.isEmpty else { return }
        let r = ReproductorVoz.compartido
        let nav = Navegacion.compartida
        r.$actual.map { $0 != nil }
            .combineLatest(nav.$seccion.map { $0 == .proyectos }, nav.$chatAbierto.map { $0 != nil },
                           nav.$visor.map { $0 != nil })
            .removeDuplicates { $0 == $1 }
            .receive(on: DispatchQueue.main)
            .sink { [weak self] t in
                MainActor.assumeIsolated {
                    self?.poner(ReproductorFlotante.sale(hayAlgo: t.0, enProyectos: t.1, conChat: t.2, conVisor: t.3))
                }
            }
            .store(in: &vigias)
    }

    private func poner(_ sale: Bool) {
        guard sale else {
            ventana?.isHidden = true
            ventana = nil
            desplazado = .zero
            return
        }
        guard let escena = ArribaUIKit.escena() else { return }
        if let v = ventana, v.windowScene === escena {
            v.isHidden = false
            return
        }
        ventana?.isHidden = true
        let v = VentanaDelAviso(windowScene: escena)
        // Encima de la app y del visor, debajo de los avisos y la llamada.
        v.windowLevel = UIWindow.Level(rawValue: UIWindow.Level.alert.rawValue + 1)
        v.backgroundColor = .clear
        let raiz = UIHostingController(rootView: CapaReproductorFlotante(f: self))
        raiz.view.backgroundColor = .clear
        v.rootViewController = raiz
        v.isHidden = false
        ventana = v
    }
}

/// La barra flotando abajo a la izquierda, que se arrastra.
struct CapaReproductorFlotante: View {
    @ObservedObject var f: ReproductorFlotante
    @GestureState private var arrastre = CGSize.zero

    var body: some View {
        VStack {
            Spacer(minLength: 0)
            HStack {
                BarraReproductor()
                    .frame(maxWidth: 460)
                    .shadow(color: .black.opacity(0.18), radius: 10, y: 3)
                    .offset(x: f.desplazado.width + arrastre.width, y: f.desplazado.height + arrastre.height)
                    .gesture(DragGesture(minimumDistance: 8)
                        .updating($arrastre) { g, estado, _ in estado = g.translation }
                        .onEnded { g in
                            f.desplazado.width += g.translation.width
                            f.desplazado.height += g.translation.height
                        })
                Spacer(minLength: 0)
            }
        }
        .padding(.leading, 16)
        .padding(.bottom, 20)
    }
}
