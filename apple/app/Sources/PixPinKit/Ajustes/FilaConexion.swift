// Ajustes → Asistente → Conexiones (Windows, 17-oct): el ID de cliente que
// el usuario crea una vez, «Conectar» (abre el navegador y espera la
// vuelta) y «Desconectar». Los tokens van al Llavero.

import PixPinCore
import SwiftUI

struct FilaConexion: View {
    let id: String
    @State private var estado: ConexionIa?
    @State private var pegado = ""
    @State private var conectando = false
    @State private var aviso: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let e = estado {
                if e.conectado {
                    Text("Conectado\(e.cuenta.map { " como \($0)" } ?? "")").foregroundStyle(.green)
                } else if let c = e.cliente {
                    Text("ID de cliente: \(c)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            HStack {
                TextField("ID de cliente (o el JSON de Google)", text: $pegado)
                    .textFieldStyle(.roundedBorder)
                Button("Guardar") { guardar() }.disabled(pegado.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            HStack {
                if conectando {
                    ProgressView().controlSize(.small)
                    Text("Esperando al navegador…").font(.caption)
                    Button("Cancelar") { iaCancelarConexion() }
                } else if estado?.conectado == true {
                    Button("Desconectar") { desconectar() }
                } else {
                    Button("Conectar…") { conectar() }.disabled(estado?.cliente == nil)
                }
            }
            if let aviso { Text(aviso).font(.caption).foregroundStyle(.secondary) }
        }
        .onAppear(perform: leer)
    }

    private func leer() {
        LlaveroDelSistema.instalar()
        estado = iaConexiones().first { $0.id == id }
    }

    private func guardar() {
        do {
            _ = try iaGuardarCliente(servicioId: id, pegado: pegado)
            pegado = ""
            aviso = nil
        } catch {
            aviso = "\(error)"
        }
        leer()
    }

    private func conectar() {
        conectando = true
        aviso = nil
        TransporteHttps.instalar()
        let id = self.id
        Task {
            let r = await Task.detached { Result { try iaConectar(servicioId: id) } }.value
            conectando = false
            if case let .failure(e) = r { aviso = "\(e)" }
            if case .success = r, id == "google_calendar" { Recordatorios.compartido.leerCalendario() }
            leer()
        }
    }

    private func desconectar() {
        do { try iaDesconectar(servicioId: id) } catch { aviso = "\(error)" }
        leer()
    }
}
