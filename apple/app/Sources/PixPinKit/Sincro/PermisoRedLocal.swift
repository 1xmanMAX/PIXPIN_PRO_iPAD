// **El permiso de red local** del iPad, explicado antes de que salga el
// aviso del sistema. iOS pregunta una sola vez («PixPin quiere buscar y
// conectarse a dispositivos de tu red local») la primera vez que la app usa
// Bonjour o abre una conexion en la Wi-Fi; si se dice que no, sincronizar
// no puede funcionar y solo se arregla en Ajustes › Privacidad › Red local.
// Por eso, antes, una pantalla propia dice para que es (como hace Android
// con sus permisos) y el aviso sale al pulsar «Continuar».
//
// iOS no tiene una API para saber si el permiso esta dado. Lo que si se ve:
// un `NWBrowser` con el permiso denegado se queda en `.waiting` con el error
// de DNS-SD `kDNSServiceErr_PolicyDenied` (-65570). Eso se mira al pulsar
// «Continuar» y cada vez que se abre Aparatos.

import Foundation
import Network
import PixPinCore
import SwiftUI
import UIKit

@MainActor
public final class PermisoRedLocal: ObservableObject {
    public enum Estado: Equatable {
        /// Aun no se ha explicado: se ensena la pantalla de antes.
        case sinExplicar
        /// Explicado; mirando si el sistema lo deja.
        case mirando
        /// No se vio que lo niegue (lo normal con el permiso dado).
        case bien
        /// El sistema lo niega: solo se arregla en Ajustes.
        case denegado
    }

    public static let compartido = PermisoRedLocal()

    /// La clave de `UserDefaults`: ya se explico una vez.
    static let claveExplicado = "pixpin.redLocalExplicada"
    /// `kDNSServiceErr_PolicyDenied` de `dns_sd.h`.
    nonisolated static let politicaDenegada: Int32 = -65570

    @Published public private(set) var estado: Estado
    private var buscador: NWBrowser?
    private var tope: Task<Void, Never>?
    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        estado = defaults.bool(forKey: PermisoRedLocal.claveExplicado) ? .bien : .sinExplicar
    }

    /// «Continuar» en la pantalla de antes: se apunta y se provoca el aviso
    /// del sistema buscando en Bonjour.
    public func continuar() {
        defaults.set(true, forKey: PermisoRedLocal.claveExplicado)
        mirar()
    }

    /// Mira si el sistema niega la red local (una busqueda corta de Bonjour).
    /// Sin explicar no hace nada: el aviso no debe salir antes de tiempo.
    public func mirar() {
        guard estado != .sinExplicar || defaults.bool(forKey: PermisoRedLocal.claveExplicado) else { return }
        buscador?.cancel()
        tope?.cancel()
        if estado != .denegado { estado = .mirando }
        let parametros = NWParameters()
        parametros.includePeerToPeer = false
        let b = NWBrowser(for: .bonjour(type: tipoDeServicio(), domain: nil), using: parametros)
        b.stateUpdateHandler = { [weak self] e in
            let negado = PermisoRedLocal.esNegado(e)
            let listo: Bool
            if case .ready = e { listo = true } else { listo = false }
            MainActor.assumeIsolated {
                guard let self else { return }
                if negado {
                    self.acabar(.denegado)
                } else if listo, self.estado == .denegado {
                    // Lo acaban de permitir en Ajustes.
                    self.estado = .mirando
                }
            }
        }
        b.browseResultsChangedHandler = { [weak self] resultados, _ in
            // Ver a alguien es la prueba de que se puede.
            if !resultados.isEmpty { MainActor.assumeIsolated { self?.acabar(.bien) } }
        }
        b.start(queue: .main)
        buscador = b
        // El aviso del sistema puede estar en pantalla: se espera de sobra.
        tope = Task { [weak self] in
            try? await Task.sleep(for: .seconds(20))
            guard !Task.isCancelled else { return }
            self?.acabar(nil)
        }
    }

    private func acabar(_ e: Estado?) {
        buscador?.cancel()
        buscador = nil
        tope?.cancel()
        tope = nil
        if let e { estado = e } else if estado == .mirando { estado = .bien }
    }

    /// Si un estado del buscador es el de la red local denegada.
    nonisolated static func esNegado(_ e: NWBrowser.State) -> Bool {
        switch e {
        case let .waiting(error), let .failed(error):
            if case let .dns(codigo) = error { return codigo == politicaDenegada }
            return false
        default:
            return false
        }
    }

    /// Ajustes › PixPin (donde esta el interruptor de «Red local»).
    public func abrirAjustes() {
        if let u = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(u) }
    }
}

/// **La pantalla de antes del aviso del sistema**: para que es la red local
/// y que va a salir.
struct PantallaRedLocal: View {
    @ObservedObject var permiso: PermisoRedLocal

    var body: some View {
        ScrollView {
            VStack(spacing: 22) {
                Image(systemName: "wifi")
                    .font(.system(size: 54, weight: .semibold))
                    .foregroundStyle(Paleta.v2.acento)
                    .padding(.top, 40)
                Text("Encontrar tus otros aparatos")
                    .font(.title.weight(.semibold))
                Text("PixPin sincroniza tus proyectos con tu PC, tu Mac y tu móvil **por tu Wi‑Fi**, sin pasar por internet ni por ninguna nube. Para encontrarlos, el iPad tiene que poder buscar en la red local.")
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 14) {
                    punto("checkmark.shield", "Solo en tu red: nada sale a internet.")
                    punto("lock", "Los aparatos de tu grupo se reconocen por el código del grupo; lo que viaja va cifrado.")
                    punto("hand.raised", "Ahora iOS te preguntará si PixPin puede buscar dispositivos en tu red local. Pulsa **Permitir**.")
                }
                .padding(18)
                .frame(maxWidth: 520, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 14).fill(Color(uiColor: .secondarySystemBackground)))
                Button {
                    permiso.continuar()
                } label: {
                    Text("Continuar").font(.headline).frame(maxWidth: 320).padding(.vertical, 6)
                }
                .buttonStyle(.borderedProminent)
                .tint(Paleta.v2.azulLleno)
                Text("Si dices que no, podrás cambiarlo en Ajustes › Privacidad y seguridad › Red local.")
                    .font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
            .padding(.horizontal, 32)
            .frame(maxWidth: 640)
            .frame(maxWidth: .infinity)
        }
    }

    private func punto(_ icono: String, _ texto: LocalizedStringKey) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: icono).frame(width: 24).foregroundStyle(Paleta.v2.acento)
            Text(texto).fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// El aviso de arriba cuando el sistema niega la red local.
struct AvisoRedLocalDenegada: View {
    @ObservedObject var permiso: PermisoRedLocal

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("PixPin no tiene permiso para usar la red local", systemImage: "wifi.exclamationmark")
                .font(.headline).foregroundStyle(Paleta.v2.rojo)
            Text("Sin él no puede encontrar ni sincronizar con tus otros aparatos. Actívalo en Ajustes › PixPin › Red local (o en Privacidad y seguridad › Red local).")
                .font(.callout).foregroundStyle(.secondary)
            HStack {
                Button("Abrir Ajustes") { permiso.abrirAjustes() }
                    .buttonStyle(.borderedProminent)
                Button("Volver a mirar") { permiso.mirar() }
            }
        }
        .padding(.vertical, 4)
    }
}
