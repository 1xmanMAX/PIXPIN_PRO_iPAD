// Los avisos de los temporizadores y las alarmas de las mini-apps: lo que en
// Windows hace el vigia de los recordatorios con el prefijo `mini:`
// (`mini_panel::avisar`, `recordatorios::agenda_de`) y en el movil una
// alarma del sistema (`MiniActivity.kt:473-520`). Es el `AvisosMini` del Mac
// con lo de AppKit cambiado:
//
// - **Cuando**: lo dice el nucleo (`miniCuandoAvisa`, `avisosMiniapps`), el
//   mismo calculo que Windows. Se rehace entero en cada cambio de la
//   mini-app, al arrancar y tras sincronizar.
// - **Con la app delante**: un `DispatchSourceTimer` con `wallDeadline` (la
//   hora de la pared) y, al sonar, el aviso dentro de la capa de pines
//   (`VistaAvisoSonando`) con un sonido en bucle hasta «Parar» (o un minuto).
// - **Con la app en segundo plano o cerrada**: iOS no deja correr nada, asi
//   que va una **notificacion local** (`UNUserNotificationCenter`) a la misma
//   hora y con el mismo identificador (`mini:<id del mensaje>`). Al volver a
//   primer plano se revisa lo que ya vencio (sin sonar dos veces).
//
// Rendimiento: nada sondea. Un temporizador de despacho por aviso puesto, y
// la lectura de todos los chats va fuera del hilo principal.

import AudioToolbox
import PixPinCore
import SwiftUI
import UIKit
import UserNotifications

/// La hora de la pared en milisegundos desde 1970 (la del formato).
func milisegundos(_ fecha: Date = Date()) -> Int64 {
    Int64((fecha.timeIntervalSince1970 * 1000).rounded())
}

/// Un aviso que esta sonando (en la capa de pines).
struct AvisoSonando: Identifiable, Equatable {
    let id: String
    let titulo: String
    let detalle: String
    let miniapp: String
}

@MainActor
public final class AvisosMini: ObservableObject {
    public static let compartido = AvisosMini()

    /// El prefijo de los avisos de una mini-app guardada en un chat (el del
    /// movil y Windows). Las que viven solo en memoria llevan `local:`.
    nonisolated static let prefijo = "mini:"

    struct Puesto {
        let cuando: Date
        let texto: String
        let miniapp: String
        let fuente: DispatchSourceTimer
    }

    /// Lo que suena ahora (lo pinta la capa de pines).
    @Published private(set) var sonando: [AvisoSonando] = []

    private(set) var puestos: [String: Puesto] = [:]
    private var nucleo: Nucleo?
    private var vigias: [NSObjectProtocol] = []
    private var permisoPedido = false
    private var sonido: Task<Void, Never>?

    init() {
        let centro = NotificationCenter.default
        vigias.append(centro.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil,
                                         queue: .main) { _ in
            MainActor.assumeIsolated { AvisosMini.compartido.revisarVencidos() }
        })
        vigias.append(centro.addObserver(forName: .pixpinTrajoCambios, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { AvisosMini.compartido.reprogramar() }
        })
    }

    /// Empieza a vigilar los avisos de los chats de `nucleo`. Una vez, al
    /// arrancar la app.
    public func vigilar(nucleo: Nucleo) {
        guard self.nucleo == nil else { return }
        self.nucleo = nucleo
        reprogramar()
    }

    /// Vuelve a leer los avisos de todos los chats (al arrancar y tras una
    /// sincronizacion que trajo cambios) y deja puestos justo esos.
    public func reprogramar() {
        guard let nucleo else { return }
        let desfase = desfaseLocalMs()
        Task { [weak self] in
            let avisos = await Task.detached(priority: .utility) {
                (try? nucleo.avisosMiniapps(desfaseLocalMs: desfase)) ?? []
            }.value
            guard let self else { return }
            let vivos = Set(avisos.map(\.id))
            for id in self.puestos.keys where id.hasPrefix(Self.prefijo) && !vivos.contains(id) {
                self.quitar(id)
            }
            for a in avisos {
                let texto = a.texto.isEmpty ? DocumentoMini.nombre(a.miniapp) : a.texto
                self.programar(id: a.id, miniapp: a.miniapp, texto: texto, cuando: fechaDe(ms: a.cuandoMs))
            }
        }
    }

    /// Pone, cambia o quita el aviso de una mini-app tras tocarla (se rehace
    /// entero, como `mini_panel::avisar` de Windows).
    func avisar(de doc: DocumentoMini) {
        guard doc.miniapp == "temporizador" || doc.miniapp == "alarma" else { return }
        let ahora = Date()
        let cuando = miniCuandoAvisa(miniapp: doc.miniapp, documento: doc.documento,
                                     ahoraMs: milisegundos(ahora), desfaseLocalMs: desfaseLocalMs(ahora))
        let texto = doc.titulo.isEmpty ? DocumentoMini.nombre(doc.miniapp) : doc.titulo
        programar(id: doc.idAviso, miniapp: doc.miniapp, texto: texto, cuando: cuando.map { fechaDe(ms: $0) })
    }

    /// Deja un aviso para `cuando` (o lo quita con `nil`). Lo que ya paso no
    /// se pone: ya aviso cuando tocaba.
    func programar(id: String, miniapp: String, texto: String, cuando: Date?) {
        if let p = puestos[id], p.cuando == cuando, p.texto == texto { return }
        quitar(id)
        guard let cuando, cuando > Date() else { return }
        let fuente = DispatchSource.makeTimerSource(queue: .main)
        fuente.schedule(wallDeadline: .now() + max(0, cuando.timeIntervalSinceNow), leeway: .milliseconds(250))
        fuente.setEventHandler {
            MainActor.assumeIsolated { AvisosMini.compartido.vencer(id) }
        }
        fuente.resume()
        puestos[id] = Puesto(cuando: cuando, texto: texto, miniapp: miniapp, fuente: fuente)
        notificar(id: id, miniapp: miniapp, texto: texto, cuando: cuando)
    }

    func quitar(_ id: String) {
        puestos.removeValue(forKey: id)?.fuente.cancel()
        Self.centro?.removePendingNotificationRequests(withIdentifiers: [id])
    }

    /// Lo vencido mientras la app estaba en segundo plano: la notificacion
    /// ya aviso, asi que se quita sin sonar (salvo lo de hace un momento).
    func revisarVencidos() {
        let ahora = Date()
        for (id, p) in puestos where p.cuando <= ahora {
            if ahora.timeIntervalSince(p.cuando) < 5 {
                vencer(id)
            } else {
                puestos.removeValue(forKey: id)?.fuente.cancel()
                if p.miniapp == "alarma", let manana = Calendar.current.date(byAdding: .day, value: 1, to: p.cuando) {
                    programar(id: id, miniapp: p.miniapp, texto: p.texto, cuando: manana)
                }
            }
        }
    }

    private func vencer(_ id: String) {
        guard let p = puestos.removeValue(forKey: id) else { return }
        p.fuente.cancel()
        let detalle: String
        if p.miniapp == "alarma" {
            let f = DateFormatter()
            f.timeStyle = .short
            f.dateStyle = .none
            detalle = "Alarma · \(f.string(from: p.cuando))"
            // Sigue encendida: manana a la misma hora del reloj de la pared.
            if let manana = Calendar.current.date(byAdding: .day, value: 1, to: p.cuando) {
                programar(id: id, miniapp: p.miniapp, texto: p.texto, cuando: manana)
            }
        } else {
            detalle = "Tiempo cumplido"
        }
        // Con la app delante suena aqui: la notificacion de esa hora sobra.
        Self.centro?.removeDeliveredNotifications(withIdentifiers: [id])
        mostrar(AvisoSonando(id: id, titulo: p.texto, detalle: detalle, miniapp: p.miniapp))
    }

    // MARK: Sonar

    func mostrar(_ aviso: AvisoSonando) {
        sonando.removeAll { $0.id == aviso.id }
        sonando.append(aviso)
        sonar()
    }

    /// «Parar»: calla y quita los avisos.
    func parar() {
        sonido?.cancel()
        sonido = nil
        sonando = []
    }

    /// Un sonido de alarma del sistema cada dos segundos, como mucho un
    /// minuto (iOS no tiene el bucle de `NSSound`).
    private func sonar() {
        guard sonido == nil else { return }
        sonido = Task { @MainActor [weak self] in
            let fin = Date().addingTimeInterval(60)
            while !Task.isCancelled, Date() < fin {
                AudioServicesPlayAlertSound(SystemSoundID(1005))
                try? await Task.sleep(for: .seconds(2))
            }
            self?.sonido = nil
        }
    }

    // MARK: Notificaciones del sistema

    /// El centro de notificaciones, solo dentro de una `.app` (en una prueba
    /// sin app anfitriona `UNUserNotificationCenter` se cae).
    private static var centro: UNUserNotificationCenter? {
        Bundle.main.bundleURL.pathExtension == "app" && Bundle.main.bundleIdentifier != nil
            ? UNUserNotificationCenter.current() : nil
    }

    private func notificar(id: String, miniapp: String, texto: String, cuando: Date) {
        guard let centro = Self.centro else { return }
        if !permisoPedido {
            permisoPedido = true
            centro.requestAuthorization(options: [.alert, .sound, .badge]) { _, _ in }
        }
        let contenido = UNMutableNotificationContent()
        contenido.title = texto
        contenido.body = miniapp == "alarma" ? "Alarma" : "Tiempo cumplido"
        contenido.sound = .default
        let partes = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute, .second], from: cuando)
        let peticion = UNNotificationRequest(identifier: id, content: contenido,
                                             trigger: UNCalendarNotificationTrigger(dateMatching: partes, repeats: false))
        centro.add(peticion) { _ in }
    }
}

/// El aviso que suena, arriba en el centro de la capa de pines, con «Parar».
struct VistaAvisoSonando: View {
    @ObservedObject var avisos: AvisosMini

    var body: some View {
        if !avisos.sonando.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(avisos.sonando) { a in
                    HStack(spacing: 12) {
                        Image(systemName: a.miniapp == "alarma" ? "alarm.fill" : "timer")
                            .font(.title)
                            .foregroundStyle(Paleta.v2.rojo)
                        VStack(alignment: .leading) {
                            Text(a.titulo).font(.headline).lineLimit(2)
                            Text(a.detalle).font(.subheadline).foregroundStyle(.secondary)
                        }
                    }
                }
                HStack {
                    Spacer()
                    Button("Parar") { avisos.parar() }
                        .keyboardShortcut(.defaultAction)
                        .buttonStyle(.borderedProminent)
                        .tint(Paleta.v2.rojo)
                }
            }
            .padding(16)
            .frame(width: 340)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
            .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(.separator.opacity(0.5)))
            .shadow(radius: 12)
            .padding(.top, 20)
            .transition(.move(edge: .top).combined(with: .opacity))
        }
    }
}
