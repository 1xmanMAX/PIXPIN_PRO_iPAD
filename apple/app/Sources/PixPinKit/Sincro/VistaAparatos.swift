// «Este aparato» y «Tus aparatos» de la ventana de Sincronizar
// (`sincronizar.rs` de Windows, copia de `SincronizarActivity.kt`): como se
// llama este Mac (y cambiarlo), todos los del grupo con «Disponible» o «No
// encontrado» y cuando se sincronizo con cada uno, y «Salir del grupo».
//
// Los textos son los de Windows (`sinc-*` de `main.ftl`).

import PixPinCore
import SwiftUI

/// Lo que se lee bajo el nombre de un aparato: si esta, y cuando se
/// sincronizo con el (`pintar_grupo` de Windows). Puro, para probarlo.
public func textoDeAparato(_ fila: Sincronizador.Fila, ahora: Int64) -> String {
    let base: String
    switch fila.estado {
    case .mirando: base = "Mirando si contesta…"
    case .contesta: base = "Disponible" + (fila.direccion.map { " (\($0.host))" } ?? "")
    case .noContesta: base = "No contesta"
    case .noEncontrado: base = "No encontrado"
    case .sincronizando: return "Sincronizando…"
    case let .hecho(texto): return texto
    case let .fallo(texto): return texto
    }
    guard fila.ultimaVez > 0 else { return base }
    return base + " · sincronizado " + haceCuanto(cuando: fila.ultimaVez, ahora: ahora)
}

/// «Este aparato»: su nombre (con «Cambiar…»), su letra y su puerto.
struct SeccionEsteAparato: View {
    @ObservedObject var sincro: Sincronizador
    @State private var cambiando = false

    var body: some View {
        Section("Este Mac") {
            LabeledContent("Nombre") {
                HStack {
                    Text(sincro.aparato?.nombre ?? "—").textSelection(.enabled)
                    Button("Cambiar…") { cambiando = true }
                        .disabled(sincro.ocupado || sincro.aparato == nil)
                        .help("Es como lo verán tus otros aparatos.")
                }
            }
            if let letra = sincro.aparato?.letra, !letra.isEmpty {
                LabeledContent("Letra en el grupo", value: letra.uppercased())
            }
            if let puerto = sincro.puerto {
                LabeledContent("Escucha en el puerto", value: String(puerto))
            }
        }
        .sheet(isPresented: $cambiando) {
            HojaNombre(nombre: sincro.aparato?.nombre ?? "") { nuevo in
                Task { await sincro.renombrar(nuevo) }
            }
        }
    }
}

/// «Nombre de este aparato» (`sinc-nombre-titulo`): el tope es el del
/// movil, 40 letras.
struct HojaNombre: View {
    @State var nombre: String
    let guardar: (String) -> Void
    @Environment(\.dismiss) private var cerrar

    static let tope = 40

    var limpio: String { String(nombre.trimmingCharacters(in: .whitespaces).prefix(Self.tope)) }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Nombre de este aparato").font(.headline)
            Text("Es como lo verán tus otros aparatos.").foregroundStyle(.secondary)
            TextField("Nombre", text: $nombre)
                .textFieldStyle(.roundedBorder)
                .onChange(of: nombre) { _, n in
                    if n.count > Self.tope { nombre = String(n.prefix(Self.tope)) }
                }
                .onSubmit(aceptar)
            HStack {
                Spacer()
                Button("Cancelar") { cerrar() }.keyboardShortcut(.cancelAction)
                Button("Guardar", action: aceptar)
                    .keyboardShortcut(.defaultAction)
                    .disabled(limpio.isEmpty)
            }
        }
        .padding(20)
        .frame(width: 360)
    }

    private func aceptar() {
        guard !limpio.isEmpty else { return }
        guardar(limpio)
        cerrar()
    }
}

/// «Tus aparatos»: todos los del grupo, los disponibles primero, con su
/// boton de sincronizar; debajo, «Conectar con una dirección» y «Salir del
/// grupo».
struct SeccionTusAparatos: View {
    @ObservedObject var sincro: Sincronizador
    @ObservedObject var vecinos: Vecinos
    @State private var saliendo = false
    @State private var conDireccion = false
    @State private var direccion = ""

    private func conectar() {
        guard let d = Direccion(texto: direccion) else { return }
        Task { await sincro.sincronizarCon(direccion: d) }
    }

    var body: some View {
        Section {
            if sincro.filas.isEmpty {
                Text(vecinos.buscando
                    ? "Todavía no hay otro aparato en el grupo. Añade uno con el código de arriba."
                    : "No se está buscando.")
                    .foregroundStyle(.secondary)
            }
            // `TimelineView` para que «hace X» avance solo, una vez por minuto.
            TimelineView(.periodic(from: .now, by: 60)) { contexto in
                let ahora = Int64(contexto.date.timeIntervalSince1970 * 1000)
                ForEach(sincro.filas) { fila in
                    FilaVecino(fila: fila, ocupado: sincro.ocupado, ahora: ahora) {
                        Task { await sincro.sincronizarAhora(id: fila.id) }
                    }
                }
            }
            DisclosureGroup("Conectar con una dirección", isExpanded: $conDireccion) {
                Text("Escribe la dirección que sale en el otro aparato, en su ventana de Sincronizar.")
                    .font(.caption).foregroundStyle(.secondary)
                HStack {
                    TextField("192.168.1.20:47474", text: $direccion)
                        .onSubmit(conectar)
                    Button("Conectar", action: conectar)
                        .disabled(sincro.ocupado || Direccion(texto: direccion) == nil)
                }
                if let puerto = sincro.puerto {
                    Text("La de este aparato: \(ipv4Local() ?? "—"):\(String(puerto))")
                        .font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                }
            }
            Button("Salir del grupo", role: .destructive) { saliendo = true }
                .disabled(sincro.ocupado)
        } header: {
            Text("Tus aparatos")
        } footer: {
            Text("Si no aparece: que tenga PixPin abierto y esté en esta Wi-Fi.")
                .font(.caption).foregroundStyle(.secondary)
        }
        .confirmationDialog("¿Salir del grupo?", isPresented: $saliendo) {
            Button("Salir", role: .destructive) { Task { await sincro.salirDelGrupo() } }
            Button("Cancelar", role: .cancel) {}
        } message: {
            Text("Este aparato dejará de sincronizarse con los demás. No se borra nada. Si vuelves a "
                + "unirte, la primera vez te preguntará por lo que haya cambiado en los dos lados.")
        }
    }
}

struct FilaVecino: View {
    let fila: Sincronizador.Fila
    let ocupado: Bool
    let ahora: Int64
    let sincronizar: () -> Void

    var body: some View {
        HStack {
            Circle()
                .fill(fila.direccion != nil ? Color.green : Color.secondary.opacity(0.4))
                .frame(width: 8, height: 8)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(fila.nombre).font(.headline)
                    if !fila.letra.isEmpty {
                        Text(fila.letra.uppercased()).font(.caption.monospaced())
                            .padding(.horizontal, 4)
                            .background(.quaternary, in: RoundedRectangle(cornerRadius: 3))
                    }
                }
                estado.font(.caption).foregroundStyle(.secondary).lineLimit(2)
            }
            Spacer()
            // Sin direccion se puede probar igual: se busca antes de llamar
            // (la de siempre y, si no contesta, el anuncio).
            Button(fila.direccion != nil ? "Sincronizar…" : "Probar", action: sincronizar)
                .disabled(ocupado)
                .help(fila.direccion != nil ? "Elige qué sincronizar con \(fila.nombre)"
                    : "Busca a \(fila.nombre) en esta Wi-Fi y, si contesta, sincroniza")
        }
    }

    @ViewBuilder private var estado: some View {
        switch fila.estado {
        case .sincronizando:
            HStack(spacing: 4) { ProgressView().controlSize(.mini); Text("Sincronizando…") }
        case .fallo:
            Text(textoDeAparato(fila, ahora: ahora)).foregroundStyle(.red)
        default:
            Text(textoDeAparato(fila, ahora: ahora))
        }
    }
}
