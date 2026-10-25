// **El motor de las automatizaciones** (tanda E6): lo que en Windows es el
// hilo de `automatizaciones/motor.rs` (`arrancar`, `bucle`, `al_capturar`,
// `al_sincronizar`, `a_mano`). Lo que corre y como lo decide el nucleo
// (`automatizaciones.rs`: elegir, la guardia de bucles, los pasos); aqui
// solo se le avisa y se le prestan las manos. Es el del Mac con lo del iPad:
//
// - **Los disparos**: la hora, cada 30 s mientras la app esta abierta (y al
//   volver a primer plano: iOS no deja correr nada con la app cerrada, asi
//   que lo que tocaba entonces corre al abrirla, como en el Mac al
//   despertar, hasta 6 h despues); una foto nueva en la carpeta de
//   capturas (se vigila la carpeta, como la galeria); una vuelta de la
//   sincronizacion que trajo algo; y a mano (la lista, el pedido
//   `automatizacion`).
// - **Una vuelta a la vez**, fuera del hilo principal: los disparos esperan
//   en una cola, y un latido no se apunta si ya hay uno esperando.
// - **Las manos** (`ManosAutomatizacion`): las acciones van por el mismo
//   camino que las del asistente (`ManosDelAsistente`: tareas aqui, lo demas
//   a `HacerPedidos`); lo que borra pregunta en el cuadro de siempre
//   (`CuadrosIPad`); un paso «pedido al asistente» es un turno sin ventana
//   (`ModeloAsistente`), y mientras dura, `HacerPedidos` no deja hacer lo
//   que haria bucle (`bloqueado`).
// - **El aviso**: al acabar, lo que paso (el aviso flotante de Windows).

import Combine
import Foundation
import PixPinCore
import UIKit

@MainActor
public final class MotorAutomatizaciones: ObservableObject {
    public static let compartido = MotorAutomatizaciones()

    /// Cada cuanto se mira la hora (`motor::LATIDO`).
    nonisolated static let latido: TimeInterval = 30

    /// Lo que va cambiando (la lista lo mira para releer).
    @Published public private(set) var vueltas = 0
    /// La que corre ahora, si alguna (su nombre).
    @Published public private(set) var corriendo: String?
    /// Lo ultimo que dijo una (para la lista).
    @Published public private(set) var ultimoAviso: String?

    private var nucleo: Nucleo?
    private weak var almacen: Almacen?
    private var cola: [DisparoAutomatizacion] = []
    private var ocupado = false
    private var reloj: Timer?
    private var vigiaCapturas: VigiaCarpeta?
    /// Las capturas que ya estaban (solo las nuevas disparan).
    private var vistas: Set<String> = []
    private var vigias: [NSObjectProtocol] = []
    /// Mientras corre un paso del asistente: su disparador (`al_capturar`…).
    private(set) var bloqueo: String?
    /// El asistente de los pasos (sin ventana).
    private var asistente: ModeloAsistente?

    /// **Empieza** (una vez, al arrancar la app).
    public func arrancar(nucleo: Nucleo, almacen: Almacen?) {
        guard self.nucleo == nil else { return }
        self.nucleo = nucleo
        self.almacen = almacen
        vigilarCapturas()
        reloj = Timer.scheduledTimer(withTimeInterval: Self.latido, repeats: true) { _ in
            MainActor.assumeIsolated { MotorAutomatizaciones.compartido.disparar(.reloj) }
        }
        let centro = NotificationCenter.default
        vigias.append(centro.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil,
                                         queue: .main) { _ in
            MainActor.assumeIsolated { MotorAutomatizaciones.compartido.disparar(.reloj) }
        })
        vigias.append(centro.addObserver(forName: .pixpinTrajoCambios, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { MotorAutomatizaciones.compartido.alSincronizar("") }
        })
        // Lo que toco con la app cerrada (hasta 6 h despues) corre ya.
        disparar(.reloj)
    }

    // MARK: Los disparos

    /// Una vuelta de la sincronizacion trajo algo (`texto`: lo que dijo).
    public func alSincronizar(_ texto: String) {
        disparar(.sincro(texto: texto))
    }

    /// Correr esa ya (la lista y el pedido `automatizacion`).
    public func aMano(_ id: String) {
        disparar(.aMano(id: id))
    }

    func disparar(_ d: DisparoAutomatizacion) {
        guard nucleo != nil else { return }
        if case .reloj = d, ocupado || cola.contains(.reloj) { return }
        cola.append(d)
        siguiente()
    }

    private func siguiente() {
        guard !ocupado, let nucleo, !cola.isEmpty else { return }
        let d = cola.removeFirst()
        ocupado = true
        let manos = ManosDeLasAutomatizaciones(nucleo: nucleo)
        let desfase = desfaseLocalMs()
        Task {
            let informes = await Task.detached(priority: .utility) {
                (try? nucleo.automatizacionesDisparar(disparo: d, desfaseMs: desfase, manos: manos)) ?? []
            }.value
            self.ocupado = false
            self.corriendo = nil
            if !informes.isEmpty { self.vueltas += 1 }
            for i in informes where i.saltada != nil {
                NSLog("PixPin: automatización «%@» saltada: %@", i.nombre, i.saltada ?? "")
            }
            self.siguiente()
        }
    }

    /// La carpeta de capturas: lo que entra nuevo es una captura nueva (la
    /// camara, el escaner, lo compartido desde otra app…).
    private func vigilarCapturas() {
        guard let nucleo else { return }
        let carpeta = URL(fileURLWithPath: nucleo.carpetaCapturas(), isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        vistas = Set(Self.imagenes(en: carpeta))
        vigiaCapturas = VigiaCarpeta(carpeta, espera: 0.5) {
            MainActor.assumeIsolated { MotorAutomatizaciones.compartido.mirarCapturas(carpeta) }
        }
    }

    private func mirarCapturas(_ carpeta: URL) {
        let ahora = Set(Self.imagenes(en: carpeta))
        let nuevas = ahora.subtracting(vistas).sorted()
        vistas = ahora
        for n in nuevas {
            disparar(.captura(ruta: carpeta.appendingPathComponent(n).path))
        }
    }

    /// Los PNG y JPG de la carpeta (sin los ficheros a medias de una
    /// escritura atomica, que empiezan por punto).
    nonisolated static func imagenes(en carpeta: URL) -> [String] {
        let nombres = (try? FileManager.default.contentsOfDirectory(atPath: carpeta.path)) ?? []
        return nombres.filter { n in
            !n.hasPrefix(".") && ["png", "jpg", "jpeg", "heic"].contains((n as NSString).pathExtension.lowercased())
        }
    }

    // MARK: Lo que piden las manos (en el principal)

    func empieza(_ nombre: String) {
        corriendo = nombre
    }

    func acabo(_ i: InformeAutomatizacion) {
        ultimoAviso = "⚙️ \(i.nombre): \(i.resumen)"
        if UIApplication.shared.applicationState == .active {
            CuadrosIPad.avisar(i.resumen, titulo: "⚙️ " + String(i.nombre.prefix(44)))
        }
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }

    /// **Un turno del asistente sin ventana** (`encargo::hacer` de
    /// Windows): con las imagenes adjuntas, el chat dicho con `@`, y lo que
    /// pregunte, en el cuadro de confirmar. Mientras dura, lo que haria
    /// saltar a la automatizacion no se hace (`bloqueo`).
    func turno(_ pedido: String, proyecto: String?, imagenes: [String], bloquear: String) async -> RespuestaPaso {
        let modelo = asistente ?? ModeloAsistente(almacen: almacen)
        asistente = modelo
        modelo.nuevaConversacion()
        modelo.adjuntar(imagenes.map { URL(fileURLWithPath: $0) })
        let nombre = proyecto.flatMap { id in almacen?.chats.first { $0.id == id }?.nombre }
        let texto = nombre.map { "\(pedido) @\($0)" } ?? pedido
        let preguntas = modelo.$pregunta.sink { p in
            guard let p else { return }
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    CuadrosIPad.confirmar("Automatización:\n\n" + p.texto, peligro: true) { modelo.responder($0) }
                }
            }
        }
        bloqueo = bloquear
        defer {
            bloqueo = nil
            preguntas.cancel()
        }
        await modelo.enviar(texto)
        guard let r = modelo.burbujas.last(where: { !$0.delUsuario }) else {
            return RespuestaPaso(bien: false, texto: "el asistente no contestó")
        }
        let t = r.texto.trimmingCharacters(in: .whitespacesAndNewlines)
        if r.esAviso || t.isEmpty {
            return RespuestaPaso(bien: false, texto: t.isEmpty ? "el asistente no contestó" : t)
        }
        return RespuestaPaso(bien: true, texto: t)
    }

    /// **Si `HacerPedidos` no debe hacer este pedido** porque lo pide el
    /// asistente dentro de un paso: el motivo (`bloqueado` de Windows).
    func bloqueado(_ accion: String) -> String? {
        automatizacionBloqueada(disparador: bloqueo, accion: accion)
    }
}

/// **Las manos que pide el nucleo** (`ManosAutomatizacion`): las llama desde
/// el hilo de la vuelta y esperan a lo que se hace en el principal.
final class ManosDeLasAutomatizaciones: ManosAutomatizacion, @unchecked Sendable {
    let nucleo: Nucleo
    /// Lo que se espera a un turno del asistente como mucho.
    var topeAsistente: TimeInterval = 15 * 60

    init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    /// Espera en este hilo a lo que haga `trabajo` en el principal; pasado
    /// `tope`, `porDefecto`.
    private func esperar<T: Sendable>(_ porDefecto: T, tope: TimeInterval,
                                      _ trabajo: @escaping @MainActor (@escaping @Sendable (T) -> Void) -> Void) -> T {
        let caja = CajaDeEspera<T>()
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                trabajo { v in
                    caja.valor = v
                    caja.s.signal()
                }
            }
        }
        guard caja.s.wait(timeout: .now() + tope) == .success else { return porDefecto }
        return caja.valor ?? porDefecto
    }

    func empieza(id: String, nombre: String) {
        DispatchQueue.main.async {
            MainActor.assumeIsolated { MotorAutomatizaciones.compartido.empieza(nombre) }
        }
    }

    func confirmar(pregunta: String) -> Bool {
        esperar(false, tope: 600) { responder in
            CuadrosIPad.confirmar("Automatización:\n\n" + pregunta, peligro: true) { responder($0) }
        }
    }

    func pedido(json: String) -> String? {
        let nucleo = self.nucleo
        return esperar(Optional("PixPin no contestó"), tope: 120) { responder in
            Task { @MainActor in
                if ManosDelAsistente.esDeTareas(json) {
                    responder(await ManosDelAsistente.hacerAqui(json, nucleo: nucleo))
                } else {
                    responder(ManosDelAsistente.aLaApp(json))
                }
            }
        }
    }

    func asistente(pedido: String, proyecto: String?, imagenes: [String], bloquear: String) -> RespuestaPaso {
        esperar(RespuestaPaso(bien: false, texto: "el asistente tardó demasiado"), tope: topeAsistente) { responder in
            Task { @MainActor in
                responder(await MotorAutomatizaciones.compartido.turno(pedido, proyecto: proyecto,
                                                                       imagenes: imagenes, bloquear: bloquear))
            }
        }
    }

    func acabo(informe: InformeAutomatizacion) {
        DispatchQueue.main.async {
            MainActor.assumeIsolated { MotorAutomatizaciones.compartido.acabo(informe) }
        }
    }
}
