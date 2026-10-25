// **Las automatizaciones** en el iPad (tanda E6): la lista que en Windows
// esta en Ajustes → Asistente, con lo mismo por fila —su interruptor,
// «Correr ahora» y borrar, que pregunta—, mas lo del Mac: el editor (crear y
// cambiar sin pedirselo al asistente) y el historial de las ultimas vueltas
// de este iPad. Se abre desde la cabecera del asistente.
//
// Lo que se lee y se escribe va al nucleo fuera del hilo principal; el
// fichero es el mismo que el de Windows, Mac y Android.

import PixPinCore
import SwiftUI

@MainActor
final class ModeloAutomatizaciones: ObservableObject {
    let nucleo: Nucleo
    @Published private(set) var lista: [FichaAutomatizacion] = []
    @Published private(set) var historial: [VueltaAutomatizacion] = []
    @Published private(set) var cargado = false
    @Published var error: String?

    init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    func cargar() async {
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .userInitiated) { () -> (Result<[FichaAutomatizacion], Error>, [VueltaAutomatizacion]) in
            (Result { try nucleo.automatizaciones() }, nucleo.automatizacionesHistorial())
        }.value
        switch r.0 {
        case let .success(l):
            lista = l
            error = nil
        case let .failure(e):
            error = "Las automatizaciones no se entienden: " + ModeloAsistente.texto(de: e)
        }
        historial = r.1
        cargado = true
    }

    /// Hace algo con el nucleo y relee; si falla, lo dice abajo.
    func cambiar(_ trabajo: @escaping @Sendable (Nucleo) throws -> Void) async {
        let nucleo = self.nucleo
        do {
            try await Task.detached(priority: .userInitiated) { try trabajo(nucleo) }.value
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        } catch {
            self.error = ModeloAsistente.texto(de: error)
        }
        await cargar()
    }

    func activar(_ f: FichaAutomatizacion, _ activa: Bool) async {
        let id = f.id
        await cambiar { _ = try $0.automatizacionActivar(id: id, activa: activa) }
    }

    func borrar(_ f: FichaAutomatizacion) async {
        let id = f.id
        await cambiar { _ = try $0.automatizacionBorrar(id: id) }
    }

    func guardar(_ f: FichaAutomatizacion) async -> String? {
        let nucleo = self.nucleo
        do {
            _ = try await Task.detached(priority: .userInitiated) { try nucleo.automatizacionGuardar(ficha: f) }.value
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            await cargar()
            return nil
        } catch {
            return ModeloAsistente.texto(de: error)
        }
    }
}

public struct VistaAutomatizaciones: View {
    @StateObject private var modelo: ModeloAutomatizaciones
    @ObservedObject private var motor = MotorAutomatizaciones.compartido
    @State private var editando: FichaAutomatizacion?
    @State private var borrando: FichaAutomatizacion?
    @State private var buscar = ""

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloAutomatizaciones(nucleo: almacen.nucleo))
    }

    private var visibles: [FichaAutomatizacion] {
        let q = buscar.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return modelo.lista }
        return modelo.lista.filter { ($0.nombre + " " + $0.detalle).lowercased().contains(q) }
    }

    private var subtitulo: String {
        if let c = motor.corriendo { return "Corriendo «\(c)»…" }
        return modelo.lista.count == 1 ? "1 automatización" : "\(modelo.lista.count) automatizaciones"
    }

    public var body: some View {
        List {
            Section {
                if modelo.cargado, modelo.lista.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Ninguna todavía").font(.headline)
                        Text("Pídeselas al asistente: «cada vez que capture, ponlo en el chat de la obra», «cada día a las 8, resumen de tareas pendientes». O crea una con «Nueva».")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 6)
                }
                ForEach(visibles, id: \.id) { f in
                    FilaAutomatizacion(ficha: f,
                                       activar: { a in Task { await modelo.activar(f, a) } },
                                       correr: { MotorAutomatizaciones.compartido.aMano(f.id) },
                                       editar: { editando = f })
                        .swipeActions {
                            Button("Borrar", role: .destructive) { borrando = f }
                        }
                        .contextMenu {
                            if f.rota == nil {
                                Button("Correr ahora", systemImage: "play.fill") { MotorAutomatizaciones.compartido.aMano(f.id) }
                                Button("Cambiar", systemImage: "pencil") { editando = f }
                            }
                            Button("Borrar", systemImage: "trash", role: .destructive) { borrando = f }
                        }
                }
            } header: {
                Text(subtitulo)
            } footer: {
                if let e = modelo.error { Text(e).foregroundStyle(.red) }
            }
            if !modelo.historial.isEmpty {
                Section("Historial de este iPad") {
                    ForEach(Array(modelo.historial.prefix(30).enumerated()), id: \.offset) { _, v in
                        FilaVuelta(vuelta: v)
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .searchable(text: $buscar, prompt: "Buscar automatizaciones")
        .navigationTitle("Automatizaciones")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button { Task { await modelo.cargar() } } label: { Image(systemName: "arrow.clockwise") }
                    .keyboardShortcut("r")
                Button { editando = EditorAutomatizacion.nueva() } label: { Label("Nueva", systemImage: "plus") }
                    .keyboardShortcut("n")
            }
        }
        .sheet(item: Binding(get: { editando.map(Editando.init) }, set: { editando = $0?.ficha })) { e in
            EditorAutomatizacion(ficha: e.ficha, nucleo: modelo.nucleo) { f in
                await modelo.guardar(f)
            }
        }
        .confirmationDialog("¿Borro la automatización «\(borrando?.nombre ?? "")»? No se puede deshacer.",
                            isPresented: Binding(get: { borrando != nil }, set: { if !$0 { borrando = nil } }),
                            titleVisibility: .visible) {
            Button("Borrar", role: .destructive) {
                if let f = borrando { Task { await modelo.borrar(f) } }
                borrando = nil
            }
            Button("Cancelar", role: .cancel) { borrando = nil }
        }
        .task { await modelo.cargar() }
        .onReceive(motor.$vueltas) { _ in Task { await modelo.cargar() } }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinCambioLocal)) { _ in
            Task { await modelo.cargar() }
        }
    }
}

/// Para el `sheet(item:)`: la ficha con un id que no cambia al editarla.
private struct Editando: Identifiable {
    let ficha: FichaAutomatizacion
    var id: String { ficha.id.isEmpty ? "nueva" : ficha.id }
}

/// Una fila de la lista: nombre, cuando y que hace, y sus botones.
struct FilaAutomatizacion: View {
    let ficha: FichaAutomatizacion
    let activar: (Bool) -> Void
    let correr: () -> Void
    let editar: () -> Void

    private var linea: String {
        if let r = ficha.rota { return "No se entiende: \(r)" }
        var t = ficha.detalle
        if let a = ficha.otroAparato { t += " · corre en \(a.isEmpty ? "otro aparato" : a)" }
        if let u = ficha.ultimaVez {
            // `ultima_vez` va en la hora de aqui (ms locales).
            let f = fechaDe(ms: u - desfaseLocalMs()).formatted(date: .abbreviated, time: .shortened)
            t += " · última vez: \(f)"
        }
        return t
    }

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: ficha.rota != nil ? "exclamationmark.triangle" : icono)
                .font(.title3)
                .foregroundStyle(ficha.rota != nil ? Color.orange : ficha.activa ? Color.accentColor : Color.secondary)
                .frame(width: 30)
            VStack(alignment: .leading, spacing: 3) {
                Text(ficha.nombre).font(.body.weight(.semibold))
                Text(linea)
                    .font(.caption)
                    .foregroundStyle(ficha.rota != nil ? Color.orange : Color.secondary)
                    .lineLimit(2)
            }
            Spacer(minLength: 8)
            if ficha.rota == nil {
                Button(action: correr) { Image(systemName: "play.fill") }
                    .buttonStyle(.borderless)
                    .help("Correr ahora")
                Button(action: editar) { Image(systemName: "pencil") }
                    .buttonStyle(.borderless)
                    .help("Cambiar")
                Toggle("", isOn: Binding(get: { ficha.activa }, set: activar))
                    .labelsHidden()
            }
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
    }

    private var icono: String {
        switch ficha.disparador {
        case "al_capturar": "camera.viewfinder"
        case "a_la_hora": "clock"
        case "al_sincronizar": "arrow.triangle.2.circlepath"
        default: "hand.tap"
        }
    }
}

/// Una vuelta del historial.
struct FilaVuelta: View {
    let vuelta: VueltaAutomatizacion

    private var disparo: String {
        switch vuelta.disparo {
        case "al_capturar": "al capturar"
        case "a_la_hora": "a su hora"
        case "al_sincronizar": "al sincronizar"
        default: "a mano"
        }
    }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: vuelta.bien ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
                .foregroundStyle(vuelta.bien ? Color.green : Color.orange)
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(vuelta.nombre).font(.subheadline.weight(.medium))
                    Text(disparo).font(.caption).foregroundStyle(.secondary)
                    Spacer()
                    Text(fechaDe(ms: vuelta.cuando).formatted(date: .abbreviated, time: .shortened))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Text(vuelta.resumen)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(3)
                    .textSelection(.enabled)
            }
        }
    }
}
