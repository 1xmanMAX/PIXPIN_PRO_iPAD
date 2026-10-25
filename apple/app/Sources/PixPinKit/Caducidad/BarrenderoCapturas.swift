// Las capturas caducadas se van solas: lo que hace en Windows
// `caducidad_capturas.rs` (barre al arrancar y luego cada hora) y en el Mac
// `BarrenderoCapturas` (a la Papelera del Mac).
//
// En el iPad no hay Papelera del sistema: lo caducado va a **la papelera de
// PixPin** con `Nucleo.borrarCapturas` (la misma que «Borrar» en la
// galeria, `papelera/capturas/`), como dice el nucleo. Lo que se va lo
// decide el nucleo con las reglas de siempre (`capturasCaducadas`:
// conservadas, fecha acordada con el grupo, el plazo del ajuste y «7 dias
// mas»); luego se le pide que olvide las conservadas y prorrogadas que ya
// no estan (`olvidarCapturasAusentes`).
//
// **El aviso «se van en 24 h»** (C4): tras cada barrido, las que se van en
// las proximas 24 h (`capturasPorAvisar`) salen en una notificacion con
// «Conservar» y «Abrir» (una) o «Ver la galeria» (varias), por
// `CentroAvisos`. Cada captura se avisa una vez por sesion.
//
// Rendimiento: iOS no deja correr nada en segundo plano sin pedirlo, asi
// que se barre poco despues de arrancar, al volver a primer plano (si paso
// una hora) y cada hora con la app abierta; siempre fuera del hilo
// principal, con prioridad `.utility`.

import Foundation
import os
import PixPinCore
import UIKit

public extension Notification.Name {
    /// Se llevaron capturas a la papelera de PixPin (caducadas o borradas
    /// desde fuera de la galeria): la galeria relee.
    static let pixpinCapturasBarridas = Notification.Name("PixPinCapturasBarridas")
}

public final class BarrenderoCapturas: @unchecked Sendable {
    /// Cada cuanto, como mucho (Windows: `CADA`, una hora).
    public static let cada: TimeInterval = 60 * 60
    /// La espera tras arrancar, para no competir con el arranque.
    public static let esperaAlArrancar: TimeInterval = 15

    /// Lo que hizo un barrido.
    public struct Barrido: Equatable, Sendable {
        /// Los nombres de las que se fueron a la papelera de PixPin.
        public var idas: [String] = []
        /// Las que no se pudieron mover (con el motivo).
        public var fallos: [String] = []
        /// Cuantos nombres se olvidaron del registro.
        public var olvidadas: UInt32 = 0
        /// Las que se avisaron porque se van en las proximas 24 h.
        public var avisadas: [String] = []
    }

    let nucleo: Nucleo
    /// Los dias del ajuste (`[capturas] dias_caducidad` de `pixpinmax.toml`,
    /// el de Windows). Si el fichero no se entiende, cero: mejor no borrar
    /// nada que borrar con un plazo inventado.
    var dias: @Sendable () -> Int64
    /// Llevar capturas a la papelera de PixPin (las pruebas lo cambian).
    var tirar: @Sendable ([String]) throws -> [CapturaBorrada]

    /// Lo que se hace con el aviso de las que se van (las pruebas lo
    /// cambian): por defecto, a `CentroAvisos`.
    var avisar: @Sendable (Aviso) -> Void = { aviso in
        Task { @MainActor in CentroAvisos.compartido.avisar(aviso) }
    }
    /// Las ya avisadas en esta sesion (por nombre).
    private var avisadas: Set<String> = []
    private var ultimo: Date?
    private var temporizador: Timer?
    private var vigias: [NSObjectProtocol] = []

    private let cerrojo = NSLock()
    private static let registro = Logger(subsystem: "io.github.1xmanmax.pixpin", category: "caducidad")

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
        dias = { AjusteCaducidad.leer(nucleo) }
        tirar = { nombres in try nucleo.borrarCapturas(nombres: nombres) }
    }

    deinit {
        temporizador?.invalidate()
        for v in vigias { NotificationCenter.default.removeObserver(v) }
    }

    /// **Barre una vez**: lo caducado a la papelera de PixPin. Bloqueante
    /// (lee el disco y mueve ficheros): fuera del hilo principal. Dos
    /// barridos a la vez no se pisan.
    @discardableResult
    public func barrer() -> Barrido {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        ultimo = Date()
        var hecho = Barrido()
        let plazo = dias()
        if plazo > 0 {
            let caducadas = nucleo.capturasCaducadas(dias: plazo)
            if !caducadas.isEmpty {
                do {
                    hecho.idas = try tirar(caducadas.map(\.nombre)).map(\.nombre)
                } catch {
                    hecho.fallos = caducadas.map { "\($0.nombre): \(error)" }
                }
            }
        }
        do {
            hecho.olvidadas = try nucleo.olvidarCapturasAusentes()
        } catch {
            BarrenderoCapturas.registro.error("no se pudo limpiar el registro de caducidad: \(String(describing: error), privacy: .public)")
        }
        if !hecho.idas.isEmpty {
            BarrenderoCapturas.registro.info("capturas caducadas a la papelera de PixPin: \(hecho.idas.count)")
            DispatchQueue.main.async {
                NotificationCenter.default.post(name: .pixpinCapturasBarridas, object: nil)
            }
        }
        for f in hecho.fallos {
            BarrenderoCapturas.registro.error("no se pudo llevar a la papelera: \(f, privacy: .public)")
        }
        if plazo > 0 {
            let seVan = nucleo.capturasPorAvisar(dias: plazo).filter { !avisadas.contains($0.nombre) }
            avisadas.formUnion(seVan.map(\.nombre))
            hecho.avisadas = seVan.map(\.nombre)
            if let aviso = AvisoCaducidad.aviso(de: seVan, ahora: Date()) {
                avisar(aviso)
            }
        }
        return hecho
    }

    /// Barre en segundo plano si toca (paso `cada` desde el ultimo).
    func barrerSiToca() {
        cerrojo.lock()
        let toca = ultimo.map { Date().timeIntervalSince($0) >= BarrenderoCapturas.cada } ?? true
        cerrojo.unlock()
        guard toca else { return }
        DispatchQueue.global(qos: .utility).async { [weak self] in self?.barrer() }
    }

    /// **Empieza**: un barrido poco despues de arrancar, al volver a primer
    /// plano si paso una hora, y cada hora con la app abierta. Llamarlo
    /// otra vez no hace nada.
    @MainActor
    public func arrancar() {
        guard temporizador == nil else { return }
        let nucleo = self.nucleo
        CentroAvisos.compartido.alResponder([.captura, .capturas]) { accion, datos in
            AvisoCaducidad.responder(accion, datos, nucleo: nucleo)
        }
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + BarrenderoCapturas.esperaAlArrancar) { [weak self] in
            self?.barrer()
        }
        let t = Timer(timeInterval: BarrenderoCapturas.cada, repeats: true) { [weak self] _ in
            self?.barrerSiToca()
        }
        t.tolerance = 20 * 60
        RunLoop.main.add(t, forMode: .common)
        temporizador = t
        vigias.append(NotificationCenter.default.addObserver(
            forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main
        ) { [weak self] _ in
            self?.barrerSiToca()
        })
    }

    /// Deja de barrer.
    public func parar() {
        temporizador?.invalidate()
        temporizador = nil
    }
}

/// El aviso de las capturas que se van (`notificaciones::de_capturas` de
/// Windows) y lo que hacen sus botones.
enum AvisoCaducidad {
    static let id = "pixpin.capturas-se-van"
    static let claveNombre = "nombre"
    static let claveRuta = "ruta"

    /// Una: con su imagen, «Conservar» y «Abrir». Varias: cuantas y la
    /// galeria. Ninguna: nada.
    static func aviso(de seVan: [CapturaCaducada], ahora: Date, calendario: Calendar = .current) -> Aviso? {
        switch seVan.count {
        case 0:
            return nil
        case 1:
            let c = seVan[0]
            let cuando = AtajosDeHora.legible(fechaDe(ms: c.seVa), ahora: ahora, calendario: calendario)
            return Aviso(id: id, categoria: .captura, titulo: "Capturas que caducan",
                         texto: "Esta captura se va a la papelera de PixPin: \(cuando)",
                         imagen: URL(fileURLWithPath: c.ruta),
                         datos: [claveNombre: c.nombre, claveRuta: c.ruta])
        default:
            return Aviso(id: id, categoria: .capturas, titulo: "Capturas que caducan",
                         texto: "\(seVan.count) capturas se van a la papelera de PixPin en las próximas 24 h")
        }
    }

    /// «Conservar» la manda a «Mensajes guardados» (y ya no caduca);
    /// «Abrir», en su visor; pulsar o «Ver la galeria», la galeria.
    @MainActor
    static func responder(_ accion: AccionAviso, _ datos: [String: String], nucleo: Nucleo) {
        switch accion {
        case .conservar:
            guard let nombre = datos[claveNombre] else { return }
            Task.detached(priority: .userInitiated) {
                if (try? nucleo.conservarCaptura(nombre: nombre)) != nil {
                    await MainActor.run {
                        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                        NotificationCenter.default.post(name: .pixpinCapturasBarridas, object: nil)
                    }
                }
            }
        case .abrir:
            guard let ruta = datos[claveRuta], FileManager.default.fileExists(atPath: ruta) else {
                Navegacion.compartida.seccion = .galeria
                return
            }
            Navegacion.compartida.abrirFichero(URL(fileURLWithPath: ruta))
        case .galeria:
            Navegacion.compartida.seccion = .galeria
        case .hecha, .posponer, .cerrar:
            break
        }
    }
}
