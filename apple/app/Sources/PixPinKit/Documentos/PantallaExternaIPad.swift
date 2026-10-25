// **La presentacion en una pantalla externa** (Windows `diapositivas.rs`:
// la diapositiva en el proyector y la vista del orador en el portatil):
// con un proyector, un monitor o AirPlay, iPadOS conecta una escena de
// pantalla externa (`windowExternalDisplayNonInteractive`). Mientras se
// presenta, en ella va la diapositiva entera (con sus transiciones) y el
// iPad pasa a la **vista del orador**: la diapositiva de ahora, la
// siguiente, las notas y el reloj. Al acabar la presentacion se suelta la
// ventana y vuelve lo de siempre.
//
// Se engancha sola desde esta zona (`DocumentosDelIPad.activar`): mira las
// escenas que se conectan y desconectan. No hace falta tocar `Raiz/`.
//
// Tambien aqui: **el modo noche de los visores recordado** en el estado
// comun (`lectoresOscuros`, el de Windows y el Mac): al arrancar se pone el
// guardado y cada cambio se guarda.

import Combine
import PixPinCore
import UIKit

@MainActor
public final class PantallaExternaIPad {
    public static let compartida = PantallaExternaIPad()

    private var escena: UIWindowScene?
    private var ventana: UIWindow?
    private weak var pase: PresentacionIPad?
    private var reloj: Timer?
    private var vigias: [NSObjectProtocol] = []
    private var activa = false

    /// Si hay una pantalla externa conectada (para los avisos).
    public var hayPantalla: Bool { escena != nil }

    func activar() {
        guard !activa else { return }
        activa = true
        let centro = NotificationCenter.default
        vigias.append(centro.addObserver(forName: UIScene.willConnectNotification, object: nil, queue: .main) { n in
            let s = n.object as? UIWindowScene
            MainActor.assumeIsolated { PantallaExternaIPad.compartida.conectada(s) }
        })
        vigias.append(centro.addObserver(forName: UIScene.didDisconnectNotification, object: nil, queue: .main) { n in
            let s = n.object as? UIScene
            MainActor.assumeIsolated { PantallaExternaIPad.compartida.desconectada(s) }
        })
        for s in UIApplication.shared.connectedScenes { conectada(s as? UIWindowScene) }
    }

    private static func esExterna(_ s: UIScene) -> Bool {
        s.session.role == .windowExternalDisplayNonInteractive
    }

    private func conectada(_ s: UIWindowScene?) {
        guard let s, PantallaExternaIPad.esExterna(s) else { return }
        escena = s
        if pase != nil { montar() }
    }

    private func desconectada(_ s: UIScene?) {
        guard let s, s === escena else { return }
        desmontar()
        escena = nil
    }

    /// Empieza una presentacion: si hay pantalla, va a ella.
    func presentar(_ p: PresentacionIPad) {
        pase = p
        montar()
    }

    /// Acabo la presentacion `p`.
    func dejar(_ p: PresentacionIPad) {
        guard pase === p else { return }
        desmontar()
        pase = nil
    }

    private func montar() {
        guard let e = escena, let p = pase, ventana == nil else { return }
        let v = VistaPaseIPad()
        v.publico = true
        v.pase = p
        p.publico = v
        let w = UIWindow(windowScene: e)
        let c = UIViewController()
        c.view = v
        w.rootViewController = c
        w.isHidden = false
        ventana = w
        p.vista.orador = true
        p.vista.setNeedsDisplay()
        let r = Timer(timeInterval: 1, repeats: true) { _ in
            MainActor.assumeIsolated { PantallaExternaIPad.compartida.pase?.vista.setNeedsDisplay() }
        }
        RunLoop.main.add(r, forMode: .common)
        reloj = r
    }

    private func desmontar() {
        reloj?.invalidate()
        reloj = nil
        ventana?.isHidden = true
        ventana?.rootViewController = nil
        ventana = nil
        pase?.publico = nil
        pase?.vista.orador = false
        pase?.vista.setNeedsDisplay()
    }
}

/// El reloj de la vista del orador: «12:05» o «1:02:05». Pura.
public func relojDelOrador(segundos: Int) -> String {
    let s = max(segundos, 0)
    let h = s / 3600, m = (s % 3600) / 60, r = s % 60
    return h > 0 ? String(format: "%d:%02d:%02d", h, m, r) : String(format: "%d:%02d", m, r)
}

// MARK: - El modo noche recordado

@MainActor
enum NocheRecordada {
    private static var vigia: AnyCancellable?

    static func activar() {
        Arranque.apuntar { entorno in
            guard let nucleo = entorno.almacen?.nucleo, vigia == nil else { return }
            if let o = nucleo.lectoresOscuros(), ModoNoche.compartido.activo != o {
                ModoNoche.compartido.activo = o
            }
            vigia = ModoNoche.compartido.$activo
                .dropFirst()
                .removeDuplicates()
                .sink { o in
                    DispatchQueue.global(qos: .utility).async { try? nucleo.guardarLectoresOscuros(oscuros: o) }
                }
        }
    }
}
