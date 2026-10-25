// **Aparatos**: la ventana de Sincronizar del iPad (`VistaSincro.swift` del
// Mac, `sincronizar.rs` de Windows, `SincronizarActivity.kt` de Android).
// Lo mas importante de la app: que el iPad entre en el mismo grupo que el
// PC, el Mac y el movil y sincronice igual.
//
// - Antes de nada, la pantalla que explica el permiso de **red local**
//   (`PermisoRedLocal`): el aviso de iOS sale una vez y, si se dice que no,
//   solo se arregla en Ajustes.
// - **Estado**: cuando se sincronizo por ultima vez, lo que se esta haciendo,
//   la frase de la ultima vuelta y los errores.
// - **Este iPad** (su nombre, que se puede cambiar) y, con grupo, **el
//   codigo** (copiar, compartir y su QR para el otro iPad o movil), **tus
//   aparatos** (`VistaAparatos`) y la sincronizacion sola.
// - **Sin grupo**: crear uno o **unirse con el codigo** del PC (tecleado,
//   pegado o escaneando un QR con la camara), con la direccion si Bonjour no
//   llega.
// - Y lo demas de Sincronizar en Windows: «Enviar y recibir por Wi‑Fi» con
//   codigo de seis cifras (`EnvioPorCodigo`), «Chats compartidos» con
//   personas (`Compartidos`) y las copias de seguridad (`VistaCopias`).

import PixPinCore
import SwiftUI
import UIKit

public struct VistaSincroIPad: View {
    let almacen: Almacen
    let sincro: Sincronizador?

    public init(almacen: Almacen, sincro: Sincronizador?) {
        self.almacen = almacen
        self.sincro = sincro
    }

    public var body: some View {
        if let sincro {
            CuerpoSincro(almacen: almacen, sincro: sincro)
        } else {
            ContentUnavailableView("No se pudo sincronizar", systemImage: "exclamationmark.triangle",
                                   description: Text("La carpeta de datos no se abrió."))
        }
    }
}

/// A donde lleva cada fila de «Más».
enum DestinoSincro: Hashable {
    case envio
    case compartidos
}

struct CuerpoSincro: View {
    @ObservedObject var almacen: Almacen
    @ObservedObject var sincro: Sincronizador
    @ObservedObject var vecinos: Vecinos
    @ObservedObject private var permiso = PermisoRedLocal.compartido
    @ObservedObject private var ajustes: ModeloAjustes
    @State private var codigo = ""
    @State private var direccion = ""
    @State private var escaneando = false
    @State private var verCopias = false
    @State private var verQR = false
    @State private var creando = false
    @State private var copiado = false
    /// Lo abierto dentro de Aparatos (Envío, Chats compartidos).
    @State private var camino: [DestinoSincro] = []
    /// Un chat recien nacido compartido: se abre «Chats compartidos», que
    /// ya invita a el (`Chat/ConversacionCompartida.swift`).
    @ObservedObject private var peticionInvitar = PeticionDeInvitar.compartida

    init(almacen: Almacen, sincro: Sincronizador) {
        _almacen = ObservedObject(wrappedValue: almacen)
        _sincro = ObservedObject(wrappedValue: sincro)
        _vecinos = ObservedObject(wrappedValue: sincro.vecinos)
        // El mismo modelo que Ajustes: dos copias de `pixpinmax.toml` en
        // memoria se pisarian al guardar.
        _ajustes = ObservedObject(wrappedValue: ModeloAjustes.compartido(nucleo: almacen.nucleo, sincro: sincro))
    }

    var body: some View {
        NavigationStack(path: $camino) {
            Group {
                if permiso.estado == .sinExplicar {
                    PantallaRedLocal(permiso: permiso)
                } else {
                    formulario
                }
            }
            .navigationTitle("Aparatos")
            .navigationDestination(for: DestinoSincro.self) { d in
                switch d {
                case .envio:
                    VistaEnvio(nucleo: almacen.nucleo, sincro: sincro,
                               nombre: sincro.aparato?.nombre ?? nombreDeEsteAparato())
                case .compartidos:
                    VistaCompartidos(almacen: almacen, sincro: sincro)
                }
            }
            .toolbar {
                if sincro.codigoLegible != nil {
                    ToolbarItem(placement: .primaryAction) {
                        Button {
                            Task { await sincro.sincronizarAhora() }
                        } label: {
                            Label("Sincronizar ahora", systemImage: "arrow.triangle.2.circlepath")
                        }
                        .disabled(sincro.ocupado || !sincro.filas.contains { $0.direccion != nil })
                    }
                }
            }
        }
        // Los del grupo que Bonjour no ensena, en su direccion recordada; y
        // si el sistema sigue dejando la red local.
        .onReceive(peticionInvitar.$ficha) { ficha in
            guard ficha != nil, camino.last != .compartidos else { return }
            camino.append(.compartidos)
        }
        .onAppear {
            sincro.mirarRecordadas()
            if permiso.estado != .sinExplicar { permiso.mirar() }
        }
        .task { if !ajustes.cargado { await ajustes.cargar() } }
        // «¿Qué sincronizar?» antes de una vuelta con un aparato.
        .sheet(item: $sincro.pregunta) { p in VistaElegirSincro(p) }
        .sheet(isPresented: $verCopias) { [sincro] in
            VistaCopias(nucleo: sincro.nucleo) { [weak sincro] in sincro?.alTraerCambios?() }
        }
        .sheet(isPresented: $verQR) {
            if let legible = sincro.codigoLegible { HojaQRGrupo(legible: legible) }
        }
        .fullScreenCover(isPresented: $escaneando) {
            HojaEscanerQR(titulo: "Escanear el código del grupo",
                          ayuda: "Apunta al QR del código del grupo que enseña tu otro aparato.",
                          acepta: { codigoDeGrupoEn($0) != nil }) { texto in
                guard let c = codigoDeGrupoEn(texto) else { return }
                codigo = c
                unirse()
            }
        }
        .confirmationDialog("¿Crear un grupo nuevo?", isPresented: $creando, titleVisibility: .visible) {
            Button("Crear un grupo") { Task { await sincro.crearGrupo() } }
            Button("Cancelar", role: .cancel) {}
        } message: {
            Text("Si ya tienes PixPin en tu PC, tu Mac o tu móvil con un grupo, no crees otro: únete al suyo con su código.")
        }
    }

    // MARK: - El formulario

    private var formulario: some View {
        Form {
            if permiso.estado == .denegado {
                Section { AvisoRedLocalDenegada(permiso: permiso) }
            }

            estado

            SeccionEsteAparato(sincro: sincro)
            // iOS solo da «iPad» como nombre: con dos iPad no se distinguen.
            if sincro.aparato?.nombre == "iPad" {
                Section {
                    Label("Ponle un nombre a este iPad (Cambiar…): así lo reconocerás en el PC, el Mac y el móvil.",
                          systemImage: "info.circle")
                        .font(.callout).foregroundStyle(.secondary)
                }
            }

            if let legible = sincro.codigoLegible {
                grupo(legible)
            } else {
                sinGrupo
            }

            Section {
                NavigationLink(value: DestinoSincro.envio) {
                    FilaMasSincro(titulo: "Enviar y recibir por Wi‑Fi", icono: "paperplane",
                            texto: "Con un código de seis cifras, sin grupo; y a tus aparatos sin código.")
                }
                NavigationLink(value: DestinoSincro.compartidos) {
                    FilaMasSincro(titulo: "Chats compartidos", icono: "person.2",
                            texto: "Un chat con otras personas: invitar con un código o su QR, entrar, sincronizar.")
                }
                Button {
                    verCopias = true
                } label: {
                    FilaMasSincro(titulo: "Copias de seguridad", icono: "clock.arrow.circlepath",
                            texto: "Cómo estaba cada conversación antes de recibir algo y antes de cada sincronización. Y los proyectos borrados.")
                }
                .buttonStyle(.plain)
            } header: {
                Text("Más")
            }

            if !sincro.recordadas.isEmpty {
                SeccionDireccionesRecordadas(sincro: sincro)
            }
        }
        .formStyle(.grouped)
    }

    // MARK: - Estado

    /// Lo de arriba: cuando se sincronizo, lo que se hace ahora, el ultimo
    /// resultado y los errores.
    private var estado: some View {
        Section {
            TimelineView(.periodic(from: .now, by: 60)) { contexto in
                let ahora = Int64(contexto.date.timeIntervalSince1970 * 1000)
                HStack(spacing: 10) {
                    Image(systemName: iconoDeEstado).foregroundStyle(colorDeEstado)
                    Text(frasePrincipal(ahora: ahora)).font(.headline)
                }
            }
            if !sincro.presencia {
                Label("«Dejarse encontrar» está apagado en Ajustes: desde aquí puedes sincronizar, "
                    + "pero tus otros aparatos no encontrarán este iPad.", systemImage: "eye.slash")
                    .foregroundStyle(Paleta.v2.naranja)
            }
            if sincro.codigoLegible == nil, sincro.hayCompartidos, sincro.presencia {
                Label("Sin grupo, pero estás en chats compartidos: este iPad escucha y se anuncia para que esas personas lo encuentren.",
                      systemImage: "person.2.wave.2")
                    .font(.callout).foregroundStyle(.secondary)
            }
            if let linea = lineaAutomatica {
                Label(linea, systemImage: "clock.arrow.2.circlepath")
                    .font(.callout).foregroundStyle(.secondary)
            }
            if sincro.fallosAuto > 0 {
                Label(sincro.fallosAuto == 1 ? "La última sincronización sola falló."
                    : "La sincronización sola lleva \(sincro.fallosAuto) fallos seguidos.",
                      systemImage: "exclamationmark.arrow.circlepath")
                    .font(.callout).foregroundStyle(Paleta.v2.naranja)
            }
            if let fase = sincro.fase {
                HStack(alignment: .top) {
                    ProgressView().controlSize(.small)
                    Text(fase)
                }
            }
            if let r = sincro.ultimoResultado {
                Text(r).font(.callout).textSelection(.enabled)
            }
            if let e = sincro.error {
                Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                    .textSelection(.enabled)
            }
        } header: {
            Text("Estado")
        } footer: {
            Text("En el iPad se sincroniza mientras PixPin está abierto: al volver a la app se pone al día con quien esté en la Wi‑Fi.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    private var hayAlguien: Bool { sincro.filas.contains { $0.direccion != nil } }

    /// Con quien va la sincronizacion sola ahora (el reloj): «Sincronizando
    /// sola con PC-Casa y Móvil». `nil` si esta apagada o no hay nadie.
    private var lineaAutomatica: String? {
        guard sincro.automatica, sincro.presencia, !sincro.bajoConsumo else { return nil }
        let todos = sincro.conQuienAuto + sincro.personasQueContestan.map { "\($0) (chat compartido)" }
        guard !todos.isEmpty else { return nil }
        let lista = todos.count == 1 ? todos[0]
            : todos.dropLast().joined(separator: ", ") + " y " + (todos.last ?? "")
        return "Sincronizando sola con \(lista)."
    }

    private var iconoDeEstado: String {
        if sincro.ocupado { return "arrow.triangle.2.circlepath" }
        if sincro.error != nil { return "exclamationmark.triangle.fill" }
        if sincro.codigoLegible == nil { return "person.badge.plus" }
        return hayAlguien ? "checkmark.circle.fill" : "wifi"
    }

    private var colorDeEstado: Color {
        if sincro.error != nil { return Paleta.v2.rojo }
        if sincro.codigoLegible == nil { return .secondary }
        return hayAlguien ? Paleta.v2.verde : .secondary
    }

    /// «Sincronizado hace 5 min con PC-Casa», «Sin grupo todavía»…
    private func frasePrincipal(ahora: Int64) -> String {
        if sincro.ocupado { return "Sincronizando…" }
        guard sincro.codigoLegible != nil else {
            return sincro.hayCompartidos ? "Sin grupo · en chats compartidos" : "Sin grupo todavía"
        }
        let ultima = sincro.filas.max { $0.ultimaVez < $1.ultimaVez }
        let disponibles = sincro.filas.filter { $0.direccion != nil }.count
        guard let ultima, ultima.ultimaVez > 0 else {
            return disponibles == 0 ? "En un grupo · aún sin sincronizar" : "En un grupo · listo para sincronizar"
        }
        return "Sincronizado " + haceCuanto(cuando: ultima.ultimaVez, ahora: ahora) + " con \(ultima.nombre)"
    }

    // MARK: - Con grupo

    private func grupo(_ legible: String) -> some View {
        Group {
            Section {
                HStack(spacing: 14) {
                    Text(legible)
                        .font(.system(.title2, design: .monospaced).weight(.semibold))
                        .textSelection(.enabled)
                    Spacer()
                    Button {
                        UIPasteboard.general.string = legible
                        copiado = true
                    } label: {
                        Image(systemName: copiado ? "checkmark" : "doc.on.doc")
                    }
                    .accessibilityLabel("Copiar el código")
                    ShareLink(item: legible) { Image(systemName: "square.and.arrow.up") }
                        .accessibilityLabel("Compartir el código")
                    Button {
                        verQR = true
                    } label: {
                        Image(systemName: "qrcode")
                    }
                    .accessibilityLabel("Ver el QR del código")
                }
                .buttonStyle(.borderless)
                .font(.title3)
            } header: {
                Text("Código del grupo")
            } footer: {
                Text("En el otro aparato: Sincronizar › Unirse a un grupo, con este código (o escaneando su QR desde otro iPad o el móvil).")
                    .font(.caption).foregroundStyle(.secondary)
            }

            SeccionTusAparatos(sincro: sincro, vecinos: vecinos)

            Section {
                Toggle(isOn: Binding(get: { sincro.automatica },
                                     set: { ajustes.fijar(sincroAutomatica: $0) })) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Sincronizar sola")
                        Text("Al aparecer un aparato, 30 s después de un cambio, cada 10 min y las tareas al instante.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                // Hasta leer el TOML no se guarda nada (se guardaria lo de fabrica).
                .disabled(!sincro.presencia || !ajustes.cargado)
                if sincro.bajoConsumo {
                    Label("Modo de bajo consumo: solo a mano.", systemImage: "battery.25")
                        .foregroundStyle(.secondary)
                }
                Button("Sincronizar ahora con todos") {
                    Task { await sincro.sincronizarAhora() }
                }
                .disabled(sincro.ocupado || !hayAlguien)
            } footer: {
                Text(sincro.filas.filter { $0.direccion != nil }.count >= 2
                    ? "Sin preguntar nada, uno detrás de otro y en dos rondas, para que todos acaben con todo."
                    : "Sin preguntar nada, con lo elegido la última vez. Para elegir qué, usa «Sincronizar…» en cada aparato.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .onChange(of: legible) { _, _ in copiado = false }
    }

    // MARK: - Sin grupo

    private var sinGrupo: some View {
        Group {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    TextField("Código del grupo (K7Q2M-9XMPA)", text: $codigo)
                        .font(.system(.title3, design: .monospaced))
                        .textInputAutocapitalization(.characters)
                        .autocorrectionDisabled()
                        .submitLabel(.join)
                        .onSubmit(unirse)
                    HStack(spacing: 18) {
                        Button {
                            escaneando = true
                        } label: {
                            Label("Escanear el QR", systemImage: "qrcode.viewfinder")
                        }
                        Button {
                            if let t = UIPasteboard.general.string, let c = codigoDeGrupoEn(t) { codigo = c }
                        } label: {
                            Label("Pegar", systemImage: "doc.on.clipboard")
                        }
                    }
                    .buttonStyle(.borderless)
                    .disabled(sincro.ocupado)
                }
                TextField("Dirección del otro (opcional: 192.168.1.20:47474)", text: $direccion)
                    .keyboardType(.numbersAndPunctuation)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                Button(action: unirse) {
                    HStack {
                        if sincro.ocupado { ProgressView().controlSize(.small) }
                        Text("Unirse al grupo").font(.headline)
                    }
                }
                .disabled(sincro.ocupado || !codigoValido(codigo: codigo)
                    || (!direccion.trimmingCharacters(in: .whitespaces).isEmpty && Direccion(texto: direccion) == nil))
            } header: {
                Text("Unirse a tu grupo")
            } footer: {
                Text("En el PC, el Mac o el móvil: abre Sincronizar en PixPin y copia el código del grupo (diez signos). Ten PixPin abierto allí y los dos en la misma Wi‑Fi. Sin dirección, el iPad lo busca solo; si no lo encuentra, escribe la dirección que sale allí.")
                    .font(.caption).foregroundStyle(.secondary)
            }

            Section {
                Button("Crear un grupo nuevo") { creando = true }
                    .disabled(sincro.ocupado)
            } header: {
                Text("¿Es tu primer aparato con PixPin?")
            } footer: {
                Text("Crea el grupo aquí y únete desde tus otros aparatos con su código.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    private func unirse() {
        guard !sincro.ocupado, codigoValido(codigo: codigo) else { return }
        let texto = direccion.trimmingCharacters(in: .whitespaces)
        let dir = texto.isEmpty ? nil : Direccion(texto: texto)
        if !texto.isEmpty, dir == nil { return }
        let c = codigo
        Task { await sincro.unirse(codigo: c, direccion: dir) }
    }
}

/// Una fila de «Más»: icono, titulo y su explicacion.
private struct FilaMasSincro: View {
    let titulo: String
    let icono: String
    let texto: String

    var body: some View {
        HStack(spacing: 14) {
            Image(systemName: icono)
                .font(.title3)
                .foregroundStyle(Paleta.v2.acento)
                .frame(width: 30)
            VStack(alignment: .leading, spacing: 2) {
                Text(titulo).foregroundStyle(.primary)
                Text(texto).font(.caption).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
        .contentShape(Rectangle())
        .padding(.vertical, 2)
    }
}

/// El QR del codigo del grupo, grande, para escanearlo desde otro iPad o el
/// movil. Lleva el codigo tal cual: cualquier lector lo ensena como texto.
struct HojaQRGrupo: View {
    let legible: String
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        NavigationStack {
            VStack(spacing: 18) {
                if let qr = imagenQR(legible, lado: 300) {
                    Image(uiImage: qr).interpolation(.none).resizable()
                        .frame(width: 300, height: 300)
                        .padding(12)
                        .background(Color.white, in: RoundedRectangle(cornerRadius: 12))
                }
                Text(legible)
                    .font(.system(.title, design: .monospaced).weight(.semibold))
                    .textSelection(.enabled)
                Text("En el otro aparato: Sincronizar › Unirse a un grupo › Escanear el QR. O escribe el código.")
                    .font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
            .padding(30)
            .navigationTitle("Código del grupo")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Listo") { cerrar() } }
            }
        }
    }
}

/// **Las direcciones recordadas** (`sincro/direcciones.txt`, el fichero del
/// movil): donde contesto cada aparato del grupo y cada persona de un chat
/// compartido la ultima vez. Es lo que se prueba antes que Bonjour. «Probar»
/// mira si contesta ahi ahora.
struct SeccionDireccionesRecordadas: View {
    @ObservedObject var sincro: Sincronizador
    @State private var abierta = false
    @State private var probadas: [String: Bool] = [:]
    @State private var probando: Set<String> = []

    var body: some View {
        Section {
            DisclosureGroup("Direcciones recordadas (\(sincro.recordadas.count))", isExpanded: $abierta) {
                ForEach(sincro.recordadas) { d in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(d.nombre ?? "Aparato desconocido")
                            Text("\(d.direccion.host):\(String(d.direccion.puerto))"
                                + (d.esDelGrupo ? "" : d.nombre == nil ? "" : " · chat compartido"))
                                .font(.caption.monospaced()).foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                        Spacer()
                        if probando.contains(d.id) {
                            ProgressView().controlSize(.small)
                        } else if let si = probadas[d.id] {
                            Image(systemName: si ? "checkmark.circle.fill" : "xmark.circle")
                                .foregroundStyle(si ? Paleta.v2.verde : .secondary)
                                .accessibilityLabel(si ? "Contesta" : "No contesta")
                        }
                        Button("Probar") { probar(d) }
                            .disabled(probando.contains(d.id))
                        // Una direccion vieja (otra Wi‑Fi, un aparato que ya
                        // no esta): se quita de `sincro/direcciones.txt`.
                        Button("Olvidar", role: .destructive) { olvidar(d) }
                            .disabled(probando.contains(d.id))
                    }
                }
            }
            .buttonStyle(.borderless)
        } footer: {
            Text("Donde contestó cada uno la última vez. Se prueba antes que buscar en la Wi‑Fi; si cambia, se apunta sola la nueva.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    private func olvidar(_ d: DireccionConNombre) {
        let nucleo = sincro.nucleo
        let id = d.id
        Task {
            _ = await Task.detached(priority: .utility) { try? nucleo.olvidarDireccion(id: id) }.value
            probadas[id] = nil
            await sincro.refrescar()
        }
    }

    private func probar(_ d: DireccionConNombre) {
        probando.insert(d.id)
        let nucleo = sincro.nucleo
        let dir = d.direccion
        Task {
            let si = await Task.detached(priority: .utility) {
                nucleo.sondearDireccion(host: dir.host, puerto: dir.puerto)
            }.value
            probando.remove(d.id)
            probadas[d.id] = si
            if si { sincro.mirarRecordadas() }
        }
    }
}
