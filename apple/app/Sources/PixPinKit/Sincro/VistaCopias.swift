// «Copias de seguridad», dentro de Sincronizar: `sincronizar/copias_ui.rs`
// de Windows (copia de `sincro/CopiasActivity.kt` del movil), con sus dos
// textos de cabecera y su «¿Volver a esta copia?».
//
// Las copias se hacen solas antes de recibir algo por Wi-Fi y antes de cada
// vuelta de sincronizar (`pixpin_sincro::copias`, en el nucleo). Aqui se
// ven, con su fecha y por que se hizo cada una, y se vuelve a cualquiera.
// Volver no borra lo hecho despues y se puede deshacer, porque antes de
// volver se guarda otra copia.
//
// Arriba del todo, **«Proyectos borrados»**: lo que esta en la papelera de
// PixPin se recupera entero (nombre, hojas en su orden, PDF y chat) y deja
// de estar borrado para sincronizar.

import PixPinCore
import SwiftUI

/// Lo que pasa en la pantalla de copias: puro respecto a la vista, para
/// probarlo sin pantalla (`tocar` de Windows).
@MainActor
public final class ModeloCopias: ObservableObject {
    /// Como acabo una vuelta atras o una recuperacion, para decirlo arriba.
    public enum Hecho: Equatable {
        case bien(String)
        case mal(String)
        case recuperado(String)
        case noRecuperado(nombre: String, motivo: String)
        case recuperados(n: UInt32, total: UInt32)
    }

    @Published public private(set) var copias: [CopiaDeSeguridad] = []
    /// Lo que hay en la papelera, el borrado mas reciente primero.
    @Published public private(set) var borrados: [ProyectoBorrado] = []
    /// La que se va a restaurar, mientras se pregunta.
    @Published public var preguntando: CopiaDeSeguridad?
    @Published public private(set) var hecho: Hecho?

    /// Se llama cuando cambio la lista de proyectos (para releerla).
    public var alCambiar: (@MainActor () -> Void)?

    private let nucleo: Nucleo

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    /// Lee las copias y la papelera (fuera del hilo principal: recorre
    /// `copias/` entera).
    public func cargar() async {
        let nucleo = self.nucleo
        let (c, b) = await Task.detached(priority: .userInitiated) {
            (nucleo.copiasDeSeguridad(), nucleo.proyectosBorrados())
        }.value
        copias = c
        borrados = b
    }

    /// «Volver» de una fila: abre la pregunta, sin tocar nada.
    public func preguntar(_ copia: CopiaDeSeguridad) {
        hecho = nil
        preguntando = copia
    }

    public func cancelar() {
        preguntando = nil
    }

    /// «Volver» en la pregunta. Sin copia en pregunta (un toque que llega
    /// tarde), nada.
    public func confirmar() async {
        guard let copia = preguntando else { return }
        preguntando = nil
        let nucleo = self.nucleo
        do {
            let nombre = try await Task.detached(priority: .userInitiated) {
                try nucleo.volverACopia(chat: copia.chat, id: copia.id)
            }.value
            hecho = .bien(nombre)
            alCambiar?()
        } catch {
            hecho = .mal(textoDeError(error))
        }
        // La lista cambia: la vuelta atras dejo antes otra copia.
        await cargar()
    }

    /// «Recuperar» de un proyecto borrado. Sin dialogo, como en el movil:
    /// recuperar no pisa ni borra nada.
    public func recuperar(_ b: ProyectoBorrado) async {
        guard borrados.contains(where: { $0.carpeta == b.carpeta }) else { return }
        let nucleo = self.nucleo
        do {
            let nombre = try await Task.detached(priority: .userInitiated) {
                try nucleo.recuperarProyecto(carpeta: b.carpeta)
            }.value
            hecho = .recuperado(nombre)
        } catch {
            hecho = .noRecuperado(nombre: b.nombre, motivo: textoDeError(error))
        }
        alCambiar?()
        await cargar()
    }

    public func recuperarTodos() async {
        preguntando = nil
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .userInitiated) { nucleo.recuperarTodos() }.value
        hecho = .recuperados(n: r.n, total: r.total)
        alCambiar?()
        await cargar()
    }

    // MARK: - Textos

    /// «13/09/26 · 16:44», en la hora local. La que no hay no se inventa.
    public nonisolated static func cuando(_ utcMs: Int64) -> String {
        guard utcMs > 0 else { return "" }
        let fecha = Date(timeIntervalSince1970: Double(utcMs) / 1000)
        let f = DateFormatter()
        f.locale = Locale(identifier: "es_ES")
        f.dateFormat = "dd/MM/yy · HH:mm"
        return f.string(from: fecha)
    }

    /// Lo que se lee bajo la fecha: de que proyecto es, por que se hizo y
    /// que tiene dentro.
    public nonisolated static func detalle(_ c: CopiaDeSeguridad) -> String {
        "«\(c.nombre)» · \(c.motivo)\n\(c.hojas) hojas · \(c.mensajes) mensajes"
            + (c.sinGrandes ? " · sin el PDF (demasiado grande)" : "")
    }

    /// El texto de «¿Volver a esta copia?» (`cop-volver-texto`).
    public nonisolated static func textoDeVolver(_ c: CopiaDeSeguridad) -> String {
        let motivo = c.motivo.prefix(1).lowercased() + c.motivo.dropFirst()
        return "«\(c.nombre)» vuelve a como estaba el \(cuando(c.cuando)) (\(motivo)): \(c.hojas) hojas. "
            + "Los lienzos que añadiste después se quedan. Antes se guarda una copia de cómo está "
            + "ahora, así que puedes deshacerlo."
    }

    public nonisolated static func texto(_ h: Hecho) -> (String, Bool) {
        switch h {
        case let .bien(n): return ("«\(n)» ha vuelto a la copia", true)
        case let .mal(m): return ("No se pudo volver a la copia: \(m)", false)
        case let .recuperado(n): return ("«\(n)» ha vuelto entero", true)
        case let .noRecuperado(n, m): return ("No se pudo recuperar «\(n)»: \(m)", false)
        case let .recuperados(n, t): return ("Recuperados \(n) de \(t) proyectos", n == t)
        }
    }
}

public struct VistaCopias: View {
    @StateObject private var modelo: ModeloCopias
    @Environment(\.dismiss) private var cerrar

    public init(nucleo: Nucleo, alCambiar: (@MainActor () -> Void)? = nil) {
        let m = ModeloCopias(nucleo: nucleo)
        m.alCambiar = alCambiar
        _modelo = StateObject(wrappedValue: m)
    }

    public var body: some View {
        VStack(spacing: 0) {
            Form {
                // Lo que ha pasado va arriba del todo: tras recuperar, la
                // fila se va de la lista y el aviso tiene que verse.
                if let h = modelo.hecho {
                    let (texto, bien) = ModeloCopias.texto(h)
                    Section {
                        Label(texto, systemImage: bien ? "checkmark.circle" : "exclamationmark.triangle")
                            .foregroundStyle(bien ? .green : .red)
                    }
                }
                if !modelo.borrados.isEmpty { borrados }
                Section {
                    ForEach(modelo.copias, id: \.clave) { c in
                        HStack(alignment: .center) {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(ModeloCopias.cuando(c.cuando)).font(.headline)
                                Text(ModeloCopias.detalle(c)).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("Volver") { modelo.preguntar(c) }
                        }
                    }
                } header: {
                    Text("Versiones anteriores")
                } footer: {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(modelo.copias.isEmpty
                            ? "Aún no hay copias. Se hacen solas antes de recibir algo por Wi-Fi y antes de sincronizar."
                            : "Cómo estaba el proyecto antes de cada envío recibido y de cada sincronización. "
                            + "Volver a una no borra lo que hiciste después.")
                        Text("Se guardan las 30 últimas de cada conversación. Los archivos de más de 40 MB "
                            + "no entran en la copia: de esos no se puede volver atrás.")
                    }
                    .font(.caption).foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            Divider()
            HStack {
                Spacer()
                Button("Cerrar") { cerrar() }.keyboardShortcut(.cancelAction)
            }
            .padding(12)
        }
        .frame(minWidth: 480, minHeight: 460)
        .task { await modelo.cargar() }
        .alert("¿Volver a esta copia?", isPresented: Binding(
            get: { modelo.preguntando != nil },
            set: { if !$0 { modelo.cancelar() } }
        ), presenting: modelo.preguntando) { _ in
            Button("Cancelar", role: .cancel) { modelo.cancelar() }
            Button("Volver") { Task { await modelo.confirmar() } }
        } message: { c in
            Text(ModeloCopias.textoDeVolver(c))
        }
    }

    private var borrados: some View {
        Section {
            ForEach(modelo.borrados, id: \.carpeta) { b in
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("«\(b.nombre)»").font(.headline)
                        Text("\(b.hojas) hojas · \(b.mensajes) mensajes · borrado el \(ModeloCopias.cuando(b.borrado))")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("Recuperar") { Task { await modelo.recuperar(b) } }
                        .buttonStyle(.borderedProminent)
                }
            }
            if modelo.borrados.count > 1 {
                Button("Recuperar todos") { Task { await modelo.recuperarTodos() } }
            }
        } header: {
            Text("Proyectos borrados")
        } footer: {
            Text("Se recuperan enteros: con su nombre, sus hojas en su orden, su PDF y su chat. Y dejan de "
                + "estar borrados, así que sincronizar no los vuelve a quitar.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }
}

extension CopiaDeSeguridad {
    /// El id de una copia solo es unico dentro de su chat.
    var clave: String { chat + "/" + id }
}
