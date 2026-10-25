// **Chats compartidos con otras personas** (Android 13-oct-2026), como
// `sincronizar/compartidos_ui.rs` de Windows: compartir un chat, invitar a
// alguien con un codigo y su QR (pintado con CoreImage), entrar con el
// codigo de otro, sincronizar con las personas que esten en la Wi-Fi y
// quitar a alguien (o salir).
//
// Lo de dentro (las firmas P-256, la valla de archivos, el protocolo
// `PXI1`/`PXC1`) es Rust (`pixpin_sincro::compartidos` e `invitacion`). Las
// claves viven en `<datos>/sincro/persona.json` como en Windows y Android
// (Windows no usa DPAPI para ellas, asi que aqui tampoco el Llavero: ver
// `apple/docs/sincro.md`).
//
// iPad: copia del Mac con `UIImage` y el nombre de este iPad; al entrar se
// puede escanear el QR de la invitacion con la camara.

import PixPinCore
import SwiftUI
import UIKit

@MainActor
public final class ModeloCompartidos: ObservableObject {
    @Published public private(set) var chats: [ChatCompartidoMac] = []
    @Published public private(set) var miNombre = ""
    /// La invitacion abierta: a que chat, su codigo y su QR.
    @Published public private(set) var invitando: (chat: String, codigo: String)?
    @Published public private(set) var qr: UIImage?
    @Published public private(set) var trabajando = false
    @Published public private(set) var aviso: String?
    @Published public private(set) var error: String?

    public let nucleo: Nucleo
    /// La sincronizacion del grupo: sus vecinos de Bonjour dan la direccion
    /// de cada persona (por el `id` de su TXT) y sus filas dicen cuales son
    /// aparatos propios (esos van por el grupo, no como personas).
    public weak var sincro: Sincronizador?
    private var invitacion: Invitacion?
    private let anuncio = AnuncioDePuerta()

    public init(nucleo: Nucleo, sincro: Sincronizador?) {
        self.nucleo = nucleo
        self.sincro = sincro
    }

    public func releer() async {
        let nucleo = self.nucleo
        do {
            let (lista, nombre) = try await Task.detached(priority: .utility) {
                try (nucleo.chatsCompartidos(), nucleo.nombreEnCompartidos())
            }.value
            chats = lista
            miNombre = nombre
        } catch {
            self.error = textoDeError(error)
        }
    }

    public func ponerNombre(_ nombre: String) async {
        await trabajar(nil) { try $0.ponerNombreEnCompartidos(nombre: nombre) }
    }

    /// Comparte el chat de la ficha `ficha` (`FichaChat.id`).
    public func compartir(ficha: String) async {
        await trabajar("Compartido: ahora puedes invitar a alguien.") { _ = try $0.compartirChat(ficha: ficha) }
        // Ya hay por que escuchar aunque no haya grupo.
        await sincro?.refrescar()
    }

    /// Quita a `id` de `chat` (o sale, si es uno mismo).
    public func quitar(chat: String, id: String) async {
        await trabajar("Hecho.") { _ = try $0.quitarDeChatCompartido(chat: chat, id: id) }
        // iPad: si ya no queda ningun chat compartido (y no hay grupo), se
        // cierra la puerta; si quedan, sigue abierta.
        await sincro?.refrescar()
    }

    // MARK: - Invitar

    /// Abre la puerta para invitar al chat de la ficha `ficha`: codigo, QR y
    /// anuncio `_pixpinchat._tcp` mientras siga abierta.
    public func invitar(ficha: String) async {
        cerrarInvitacion()
        error = nil
        let nucleo = self.nucleo
        do {
            let i = try await Task.detached(priority: .utility) { try nucleo.invitarAChat(ficha: ficha) }.value
            invitacion = i
            invitando = (ficha, i.codigo())
            qr = imagenQR(i.textoDelQr(host: ipv4Local()))
            let nombre = miNombre.isEmpty ? nombreDeEsteAparato() : miNombre
            anuncio.anunciar({ i.anuncio(nombre: nombre) }, puerto: i.puerto())
            atenderEnHilo(nombre: "pixpin-chat-invitacion", { try i.atender() },
                          esFin: { $0 == .cerrada || $0 == .quemada }) { [weak self] r in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.enLaPuerta(r) }
                }
            }
        } catch {
            self.error = textoDeError(error)
        }
    }

    private func enLaPuerta(_ r: Result<AvisoInvitacion, Error>) {
        switch r {
        case let .success(.entro(_, nombre)): aviso = "\(nombre) ha entrado."
        case let .success(.junto(_, nombre)):
            aviso = nombre.isEmpty ? "Listo." : "\(nombre) ya tiene el chat."
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        case let .success(.codigoMalo(intentos)): aviso = "Alguien probó otro código (\(intentos) de 5)."
        case .success(.quemada):
            aviso = "Demasiados códigos equivocados: la invitación se cerró."
            cerrarInvitacion()
        case let .success(.fallo(detalle)): aviso = "Se cortó: \(detalle)"
        case .success(.cerrada): break
        case let .failure(e): aviso = textoDeError(e)
        }
        Task { await releer() }
    }

    public func cerrarInvitacion() {
        invitacion?.cerrar()
        invitacion = nil
        anuncio.retirar()
        invitando = nil
        qr = nil
    }

    // MARK: - Entrar

    /// Entra con el codigo (o el texto del QR) de quien invita: con la
    /// direccion del QR o buscandole en la Wi-Fi.
    public func entrar(_ texto: String) async {
        error = nil
        guard let c = leerInvitacion(texto: texto) else {
            error = "Eso no es un código de invitación: son ocho signos (K7Q2-M9XP) o el texto de su QR."
            return
        }
        trabajando = true
        defer { trabajando = false }
        var destino: Direccion?
        if let host = c.host, c.puerto > 0 {
            destino = Direccion(host: host, puerto: c.puerto)
        } else {
            aviso = "Buscando la invitación en esta Wi-Fi…"
            let codigo = c.codigo
            let etiqueta = await Task.detached(priority: .utility) { etiquetaDeInvitacion(codigo: codigo) }.value
            destino = await buscarPuerta(tipo: tipoDeServicioChat(), etiqueta: etiqueta)
        }
        guard let destino else {
            aviso = nil
            error = "No apareció la invitación. ¿Tiene la otra persona la ventana de invitar abierta en esta Wi-Fi?"
            return
        }
        aviso = "Entrando y trayendo el chat…"
        let nucleo = self.nucleo
        do {
            let codigo = c.codigo
            let dentro = try await Task.detached(priority: .utility) {
                try nucleo.entrarEnChatCompartido(codigo: codigo, host: destino.host, puerto: destino.puerto)
            }.value
            aviso = "Ya estás en «\(dentro.nombre)»."
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            await sincro?.refrescar()
            sincro?.alTraerCambios?()
        } catch {
            aviso = nil
            self.error = textoDeError(error)
        }
        await releer()
    }

    // MARK: - Sincronizar

    /// **Sincroniza `chat` con cada persona que este en la Wi-Fi**
    /// (`sincronizar_ahora` de Windows): por su `id` en Bonjour, sin mis
    /// propios aparatos (esos van por el grupo). Tambien con quien ya no
    /// esta: asi se entera de que salio o de que lo quitaron.
    public func sincronizar(_ c: ChatCompartidoMac) async {
        guard let sincro else { return }
        let yo = c.personas.first(where: \.soyYo)?.id
        let mios = Set(sincro.filas.map(\.id))
        let otros = c.personas.filter { $0.id != yo && !mios.contains($0.id) }
        guard !otros.isEmpty else {
            aviso = "Aún no hay nadie más en este chat."
            return
        }
        trabajando = true
        defer { trabajando = false }
        var hechos: [String] = []
        let nucleo = self.nucleo
        for p in otros {
            guard let v = sincro.vecinos.todos.first(where: { $0.id == p.id }),
                  let d = await Vecinos.sondear(v.punto) else { continue }
            aviso = "Sincronizando con \(p.nombre)…"
            let id = p.id
            let chat = c.chat
            do {
                let r = try await Task.detached(priority: .utility) { () throws -> ResumenPersona in
                    let r = try nucleo.sincronizarConPersona(host: d.host, puerto: d.puerto, id: id, chats: [chat])
                    // Donde contesto, para la sincronizacion sola con ella
                    // (`apuntar_direccion` de Windows).
                    if r.puertoDelOtro > 0 {
                        _ = try? nucleo.apuntarDireccion(id: id, host: d.host, puerto: r.puertoDelOtro)
                    }
                    return r
                }.value
                hechos.append(r.otroNombre)
                if r.trajoAlgo { sincro.alTraerCambios?() }
            } catch {
                NSLog("PixPin: no se pudo sincronizar con una persona: %@", textoDeError(error))
            }
        }
        aviso = hechos.isEmpty ? "Nadie de este chat está ahora en esta Wi-Fi."
            : "Sincronizado con \(hechos.joined(separator: ", "))."
        await releer()
    }

    private func trabajar(_ hecho: String?, _ f: @escaping @Sendable (Nucleo) throws -> Void) async {
        error = nil
        let nucleo = self.nucleo
        do {
            try await Task.detached(priority: .utility) { try f(nucleo) }.value
            if let hecho { aviso = hecho }
        } catch {
            self.error = textoDeError(error)
        }
        await releer()
    }
}

/// La ventana «Chats compartidos».
public struct VistaCompartidos: View {
    @StateObject private var modelo: ModeloCompartidos
    @ObservedObject private var almacen: Almacen
    @State private var nombre = ""
    @State private var codigo = ""
    @State private var aCompartir: String?
    @State private var quitando: (chat: String, persona: PersonaCompartida)?
    @State private var escaneando = false

    public init(almacen: Almacen, sincro: Sincronizador?) {
        _almacen = ObservedObject(wrappedValue: almacen)
        _modelo = StateObject(wrappedValue: ModeloCompartidos(nucleo: almacen.nucleo, sincro: sincro))
    }

    public var body: some View {
        Form {
            Section {
                HStack {
                    TextField("Tu nombre", text: $nombre)
                    Button("Guardar") { Task { await modelo.ponerNombre(nombre) } }
                        .disabled(nombre.trimmingCharacters(in: .whitespaces).isEmpty || nombre == modelo.miNombre)
                        .buttonStyle(.borderless)
                }
            } header: {
                Text("Cómo te ven")
            }

            if let inv = modelo.invitando {
                Section {
                    VStack(spacing: 10) {
                        Text(invitacionLegible(codigo: inv.codigo))
                            .font(.system(size: 30, weight: .semibold, design: .monospaced))
                            .textSelection(.enabled)
                        if let qr = modelo.qr {
                            Image(uiImage: qr).interpolation(.none).resizable().frame(width: 220, height: 220)
                        }
                        Button("Cerrar la invitación") { modelo.cerrarInvitacion() }
                    }
                    .frame(maxWidth: .infinity)
                } header: {
                    Text("Invitar")
                } footer: {
                    Text("La otra persona: «Entrar en un chat compartido» con este código o escaneando el QR. Vale mientras esté abierta.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }

            Section("Chats compartidos") {
                if modelo.chats.isEmpty {
                    Text("Ninguno todavía.").foregroundStyle(.secondary)
                }
                ForEach(modelo.chats, id: \.chat) { c in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text(c.nombre).font(.headline)
                            Spacer()
                            if c.estoyDentro {
                                Button("Sincronizar") { Task { await modelo.sincronizar(c) } }
                                if let ficha = c.ficha {
                                    Button("Invitar…") { Task { await modelo.invitar(ficha: ficha) } }
                                }
                            } else {
                                Text("Ya no estás").font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        ForEach(c.personas.filter(\.dentro), id: \.id) { p in
                            HStack {
                                Text(p.soyYo ? "\(p.nombre) (tú)" : p.nombre)
                                Text(p.codigo).font(.caption.monospaced()).foregroundStyle(.secondary)
                                Spacer()
                                if p.puedoQuitar {
                                    Button(p.soyYo ? "Salir" : "Quitar") { quitando = (c.chat, p) }
                                        .buttonStyle(.borderless)
                                }
                            }
                            .font(.callout)
                        }
                    }
                    // iPad: varios botones en una fila de `Form`, cada uno
                    // con su toque (si no, la fila entera los pulsa todos).
                    .buttonStyle(.borderless)
                }
            }

            Section {
                Picker("Chat", selection: $aCompartir) {
                    Text("Elige un chat").tag(String?.none)
                    ForEach(almacen.chats.filter { !$0.esGuardados && !$0.esAsistente }, id: \.id) { f in
                        Text(f.nombre).tag(Optional(f.id))
                    }
                }
                Button("Compartir con personas") {
                    if let f = aCompartir { Task { await modelo.compartir(ficha: f) } }
                }
                .disabled(aCompartir == nil || modelo.trabajando)
            } header: {
                Text("Compartir un chat")
            }

            Section {
                TextField("Código (K7Q2-M9XP) o texto del QR", text: $codigo)
                    .font(.system(.body, design: .monospaced))
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                HStack {
                    Button("Entrar") { Task { await modelo.entrar(codigo) } }
                        .disabled(leerInvitacion(texto: codigo) == nil || modelo.trabajando)
                    Spacer()
                    Button {
                        escaneando = true
                    } label: {
                        Label("Escanear el QR", systemImage: "qrcode.viewfinder")
                    }
                    .disabled(modelo.trabajando)
                }
                .buttonStyle(.borderless)
            } header: {
                Text("Entrar en un chat compartido")
            }

            if modelo.trabajando || modelo.aviso != nil || modelo.error != nil {
                Section("Actividad") {
                    if modelo.trabajando { ProgressView().controlSize(.small) }
                    if let a = modelo.aviso { Text(a) }
                    if let e = modelo.error {
                        Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .task {
            await modelo.releer()
            nombre = modelo.miNombre
        }
        .onDisappear { modelo.cerrarInvitacion() }
        .navigationTitle("Chats compartidos")
        .fullScreenCover(isPresented: $escaneando) {
            HojaEscanerQR(titulo: "Escanear la invitación",
                          ayuda: "Apunta al QR de «Invitar» en el otro aparato.",
                          acepta: { leerInvitacion(texto: $0) != nil }) { texto in
                codigo = texto
                Task { await modelo.entrar(texto) }
            }
        }
        .confirmationDialog(textoDeQuitar, isPresented: Binding(
            get: { quitando != nil }, set: { if !$0 { quitando = nil } })) {
            Button(quitando?.persona.soyYo == true ? "Salir" : "Quitar", role: .destructive) {
                if let q = quitando { Task { await modelo.quitar(chat: q.chat, id: q.persona.id) } }
                quitando = nil
            }
        }
    }

    private var textoDeQuitar: String {
        guard let q = quitando else { return "" }
        return q.persona.soyYo ? "¿Salir de este chat compartido?" : "¿Quitar a \(q.persona.nombre) de este chat?"
    }
}
