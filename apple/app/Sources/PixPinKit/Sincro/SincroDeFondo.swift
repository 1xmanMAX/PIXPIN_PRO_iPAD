// **Sincronizar con la app cerrada, de vez en cuando** (`BGAppRefreshTask`
// `io.github.1xmanmax.pixpin.sincronizar`, declarado en el Info.plist).
//
// En el iPad la sincronizacion vive mientras la app esta abierta (ver
// `EntornoApp`): iOS cierra la red al irse a segundo plano. Con esto, iOS
// despierta la app algun rato (cuando le parece: carga, Wi-Fi, costumbre de
// uso; no antes de 15 min) y se hace una vuelta con los del grupo que
// contesten en su direccion recordada (Bonjour no anuncia desde el fondo).
// Unos 25 s como mucho: iOS da unos 30.
//
// Como se engancha: la tarea se registra en la escena de la app con
// `.backgroundTask(.appRefresh(SincroDeFondo.identificador)) {
// await SincroDeFondo.alDespertar() }` y `SincroDeFondo.activar()` al
// arrancar (las dos cosas en `PixPinApp`, que es de otra zona). Sin
// `activar()` no se pide nunca nada: pedir una tarea sin quien la atienda
// haria fallar el despertar.

import BackgroundTasks
import Foundation
import PixPinCore
import UIKit

@MainActor
public enum SincroDeFondo {
    public static let identificador = "io.github.1xmanmax.pixpin.sincronizar"
    /// Lo antes que se pide el siguiente despertar.
    static let cadaCuanto: TimeInterval = 15 * 60
    /// Lo que se espera a que contesten los recordados antes de la vuelta.
    static let esperaSondas: Duration = .seconds(4)

    private static var activado = false
    private static weak var entorno: EntornoApp?
    private static var observador: NSObjectProtocol?

    /// Lo llama la app al arrancar, despues de registrar la tarea con
    /// `.backgroundTask(.appRefresh(...))`: desde aqui, al irse al fondo se
    /// pide el siguiente despertar.
    public static func activar() {
        guard !activado else { return }
        activado = true
        Arranque.apuntar { e in SincroDeFondo.entorno = e }
        observador = NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
        ) { _ in
            MainActor.assumeIsolated { SincroDeFondo.programar() }
        }
    }

    /// Pide a iOS el siguiente despertar (sustituye al que hubiera). Solo
    /// con grupo: sin nadie con quien sincronizar no hay por que despertar.
    public static func programar() {
        guard activado, entorno?.sincro?.codigoLegible != nil else { return }
        let pedido = BGAppRefreshTaskRequest(identifier: identificador)
        pedido.earliestBeginDate = Date(timeIntervalSinceNow: cadaCuanto)
        do {
            try BGTaskScheduler.shared.submit(pedido)
        } catch {
            // En el simulador y sin «Actualizar en segundo plano» no se puede.
            NSLog("PixPin: no se pudo pedir sincronizar en segundo plano: %@", "\(error)")
        }
    }

    /// **El despertar**: arranca la sincronizacion, deja unos segundos a las
    /// sondas de las direcciones recordadas y hace una vuelta con todos los
    /// que contestan (sin preguntar, con lo elegido la ultima vez). Si la
    /// app sigue en el fondo, lo para todo otra vez. Pide el siguiente.
    public static func alDespertar() async {
        guard let sincro = entorno?.sincro else { return }
        let estabaEnMarcha = UIApplication.shared.applicationState == .active
        sincro.arrancar()
        await sincro.refrescar()
        // Ya se sabe si hay grupo: el siguiente despertar.
        programar()
        sincro.mirarRecordadas()
        try? await Task.sleep(for: esperaSondas)
        if !sincro.ocupado, sincro.filas.contains(where: { $0.direccion != nil }) {
            await sincro.sincronizarAhora()
        }
        // Lo que este a medias se acaba (la vuelta sola que arranco el reloj).
        var vueltas = 0
        while sincro.ocupado, vueltas < 30 {
            try? await Task.sleep(for: .milliseconds(500))
            vueltas += 1
        }
        if !estabaEnMarcha, UIApplication.shared.applicationState != .active {
            sincro.parar()
        }
    }
}
