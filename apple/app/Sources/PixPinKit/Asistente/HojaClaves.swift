// **Las claves de la nube** (Ajustes → Asistente de Windows, en una hoja):
// quien contesta (Claude, DeepSeek o Apple Intelligence en el iPad) y la
// clave de cada proveedor. La clave va **cifrada al Llavero** del iPad
// (`LlaveroDelSistema`, por `iaGuardarClave` del nucleo): nunca se escribe
// en claro, no viaja a los otros aparatos y solo se ensena enmascarada
// (`sk-ant-…AbCd`). «Probar» hace una llamada minima (unos pocos tokens).

import PixPinCore
import SwiftUI

/// Un proveedor de la nube.
struct ProveedorNube: Identifiable, Hashable {
    let id: String
    let nombre: String
    let pista: String
    let dondeSacarla: URL

    static let todos: [ProveedorNube] = [
        ProveedorNube(id: "claude", nombre: "Claude (Anthropic)", pista: "sk-ant-…",
                      dondeSacarla: URL(string: "https://console.anthropic.com/settings/keys")!),
        ProveedorNube(id: "deepseek", nombre: "DeepSeek", pista: "sk-…",
                      dondeSacarla: URL(string: "https://platform.deepseek.com/api_keys")!),
    ]
}

struct HojaClaves: View {
    let almacen: Almacen
    var alAcabar: () -> Void = {}
    @Environment(\.dismiss) private var cerrar
    @State private var motor = ""
    @State private var escritas: [String: String] = [:]
    @State private var guardadas: [String: String] = [:]
    @State private var estado: [String: String] = [:]
    @State private var probando: String?
    @State private var hayApple = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Quién contesta", selection: Binding(get: { motor }, set: elegir)) {
                        Text("Claude Haiku (nube)").tag("claude")
                        Text("DeepSeek (nube)").tag("deepseek")
                        Text(hayApple ? "Apple Intelligence (en el iPad)" : "En el iPad (necesita Apple Intelligence)")
                            .tag("")
                    }
                } footer: {
                    Text("El mismo ajuste que Ajustes → Asistente en Windows y el Mac.")
                }
                ForEach(ProveedorNube.todos) { p in
                    seccion(p)
                }
            }
            .navigationTitle("Asistente: claves")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Hecho") {
                        alAcabar()
                        cerrar()
                    }
                }
            }
        }
        .onAppear(perform: releer)
    }

    @ViewBuilder
    private func seccion(_ p: ProveedorNube) -> some View {
        Section {
            if let enmascarada = guardadas[p.id] {
                LabeledContent("Guardada", value: enmascarada)
            }
            SecureField(guardadas[p.id] == nil ? "Pega aquí la clave (\(p.pista))" : "Otra clave para sustituirla",
                        text: Binding(get: { escritas[p.id] ?? "" }, set: { escritas[p.id] = $0 }))
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .textContentType(.password)
                .onSubmit { guardar(p) }
            HStack {
                Button("Guardar") { guardar(p) }
                    .disabled((escritas[p.id] ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                Spacer()
                if probando == p.id {
                    ProgressView()
                } else {
                    Button("Probar") { probar(p) }
                        .disabled(guardadas[p.id] == nil || probando != nil)
                }
                Spacer()
                Button("Borrar", role: .destructive) { borrar(p) }
                    .disabled(guardadas[p.id] == nil)
            }
            .buttonStyle(.borderless)
            if let e = estado[p.id] {
                Text(e).font(.callout).foregroundStyle(.secondary)
            }
        } header: {
            Text(p.nombre)
        } footer: {
            Link("Sacar una clave de \(p.nombre)", destination: p.dondeSacarla)
        }
    }

    private func releer() {
        ModeloAsistente.instalarLaNube()
        motor = (try? almacen.nucleo.ajustes().asistenteMotor) ?? ""
        hayApple = motorAppleDisponible()
        for p in ProveedorNube.todos {
            guardadas[p.id] = iaClaveEnmascarada(proveedorId: p.id)
        }
    }

    private func elegir(_ nuevo: String) {
        guard var a = try? almacen.nucleo.ajustes() else { return }
        a.asistenteMotor = nuevo
        do {
            try almacen.nucleo.guardarAjustes(nuevos: a)
            motor = nuevo
        } catch {
            estado[nuevo.isEmpty ? "claude" : nuevo] = ModeloAsistente.texto(de: error)
        }
    }

    private func guardar(_ p: ProveedorNube) {
        let clave = (escritas[p.id] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clave.isEmpty else { return }
        ModeloAsistente.instalarLaNube()
        do {
            try iaGuardarClave(proveedorId: p.id, clave: clave)
            escritas[p.id] = ""
            guardadas[p.id] = iaClaveEnmascarada(proveedorId: p.id)
            estado[p.id] = "Guardada en el Llavero."
            // Sin motor en el iPad, la nube con clave pasa a contestar.
            if motor.isEmpty, !hayApple { elegir(p.id) }
            probar(p)
        } catch {
            estado[p.id] = ModeloAsistente.texto(de: error)
        }
    }

    private func probar(_ p: ProveedorNube) {
        probando = p.id
        estado[p.id] = "Probando…"
        let id = p.id
        Task {
            let r = await Task.detached(priority: .userInitiated) { Result { try iaProbarClave(proveedorId: id) } }.value
            probando = nil
            switch r {
            case let .success(t): estado[id] = t
            case let .failure(e): estado[id] = ModeloAsistente.texto(de: e)
            }
        }
    }

    private func borrar(_ p: ProveedorNube) {
        do {
            try iaBorrarClave(proveedorId: p.id)
            guardadas[p.id] = nil
            estado[p.id] = "Borrada."
        } catch {
            estado[p.id] = ModeloAsistente.texto(de: error)
        }
    }
}
