// **El asistente dentro del chat** (E1, `ventana_chat/asistente.rs` de
// Windows; el `AsistenteEnElChat` del Mac): en el chat fijo «Asistente» todo
// lo que se escribe es para el; en cualquier otro, `@asistente`. Su
// respuesta es una NOTA normal del cuaderno con `aparato = "IA"` (la chapa
// dice `#48·IA`): se guarda, se busca y viaja a Windows y al movil como
// cualquier otra.
//
// - `alEnviar` (lo llama `Almacen` por `GanchosAlmacen.alEnviar` al
//   escribir una nota o adjuntar una foto): si es para el, el nucleo
//   escribe la respuesta («💭 Pensando…», `iaAlEnviar`) y el turno corre en
//   un hilo de GCD (`iaTurnoEnChat`). Lo que va escribiendo llega por un
//   `OyenteChatIa` (cada ~60 ms como mucho) y se pinta en su burbuja desde
//   `vivos` (`NotaDelAsistente`); el disco se reescribe cada 2 s y al acabar.
// - Lo que borra o envia pregunta en la burbuja con «Sí» y «No»
//   (`preguntas`); «Parar» en la barra de encima de la caja
//   (`BarraDelAsistente`).
// - Lo dibujado va a un lienzo del chat y, si el turno lo pide, se abre en
//   el visor del lienzo. Con Apple Intelligence, que no pasa por Rust, la
//   respuesta se escribe al acabar (`iaResponder`).
// - Las recetas: los pedidos que se repiten, como botones encima de la caja
//   del chat «Asistente».
//
// Las vistas (`NotaDelAsistente`, `BarraDelAsistente`) las pone el chat de
// Proyectos en su burbuja y encima de su caja.

import Foundation
import PixPinCore
import SwiftUI
import UIKit

/// Quien ve la respuesta en vivo: el `OyenteChatIa` del nucleo. Rust lo
/// llama desde el hilo del turno; aqui se pasa al principal.
final class OyenteChatDelAsistente: OyenteChatIa, @unchecked Sendable {
    let alVivo: @Sendable (VivoIa) -> Void

    init(alVivo: @escaping @Sendable (VivoIa) -> Void) {
        self.alVivo = alVivo
    }

    func vivo(v: VivoIa) {
        alVivo(v)
    }
}

public extension MensajeChat {
    /// Una respuesta del asistente (su chapa es `#48·IA`).
    var esDelAsistente: Bool { codigoChat?.hasSuffix("·IA") ?? false }
}

extension PensadoIa {
    /// Si hay algo que ensenar encima de la respuesta.
    var algo: Bool {
        !pensamiento.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !entiendo.isEmpty || reinicio
    }
}

public extension Notification.Name {
    /// El asistente dibujo en un lienzo del chat mientras contesta: el
    /// lienzo abierto se relee (`object`: el id del chat).
    static let pixpinLienzoDesdeElAsistente = Notification.Name("pixpin.lienzoDesdeElAsistente")
}

@MainActor
public final class AsistenteEnElChat: ObservableObject {
    public static let compartido = AsistenteEnElChat()

    /// Lo que se ve de cada respuesta en marcha, por el id de su nota.
    @Published public private(set) var vivos: [String: VivoIa] = [:]
    /// La pregunta de «¿seguro?» que espera en la burbuja, por nota.
    @Published public private(set) var preguntas: [String: ModeloAsistente.Pregunta] = [:]
    /// Los chats donde esta contestando, con su corte.
    @Published public private(set) var contestando: [String: CorteIa] = [:]
    /// Un aviso para la barra del chat (sin motor, ocupado...).
    @Published public private(set) var aviso: (chat: String, texto: String)?
    /// Si al aviso le falta una clave (la barra ofrece ponerla).
    @Published public private(set) var avisoDeClave = false
    /// Sube cuando cambian las recetas (la fila se relee).
    @Published public private(set) var recetasCambiadas = 0

    /// Lo pensado de las respuestas guardadas (`nil` dentro: no hay).
    private var pensados: [String: PensadoIa?] = [:]
    /// El modelo para los ajustes y el motor (uno por almacen).
    private var modelo: (almacen: ObjectIdentifier, modelo: ModeloAsistente)?

    /// **Otro que conteste** (las pruebas, sin red ni modelo): recibe el
    /// chat y lo que nace; sin el, el motor de los ajustes.
    public static var turnoDePrueba: ((Nucleo, String, InicioIa, OyenteChatIa, AppIa, CorteIa) throws -> FinChatIa)?

    public func ocupado(_ chat: String) -> Bool { contestando[chat] != nil }

    /// «Parar»: el modelo deja de escribir y queda lo que llevaba (y una
    /// pregunta pendiente cuenta como «no»).
    public func parar(_ chat: String) {
        contestando[chat]?.cancelar()
        for (nota, v) in vivos where v.pregunta != nil {
            responder(nota: nota, si: false)
        }
    }

    /// «Sí» o «No» en la tarjeta de la burbuja.
    public func responder(nota: String, si: Bool) {
        guard let p = preguntas.removeValue(forKey: nota) else { return }
        p.responder(si)
    }

    public func quitarAviso() {
        aviso = nil
        avisoDeClave = false
    }

    // MARK: Recetas

    /// **Las recetas** (los pedidos que se repiten, como botones encima de
    /// la caja del chat «Asistente»): una automatizacion `a_mano` con
    /// `boton: true` que viaja con el chat (`asistente/recetas.rs`).
    func cambiarRecetas(_ nucleo: Nucleo, en chat: String, _ hacer: @escaping @Sendable (Nucleo) throws -> Void) {
        Task {
            let r = await Task.detached(priority: .userInitiated) { Result { try hacer(nucleo) } }.value
            if case let .failure(e) = r { avisar(ModeloAsistente.texto(de: e), en: chat) }
            recetasCambiadas += 1
        }
    }

    /// «Guardar como receta» desde un mensaje (el menu del mensaje en el
    /// chat de Proyectos).
    public func guardarReceta(desde m: MensajeChat, chat: String, almacen: Almacen) {
        let nucleo = almacen.nucleo
        Task {
            let r = await Task.detached(priority: .userInitiated) {
                Result { try nucleo.iaRecetaDesde(chat: chat, mensaje: m.id) }
            }.value
            switch r {
            case let .success(receta?):
                avisar("Guardada como receta: \(receta.icono) \(receta.nombre). Está encima de la caja del chat «Asistente».",
                       en: chat)
            case .success(nil):
                avisar("Ese mensaje no pide nada que guardar.", en: chat)
            case let .failure(e):
                avisar(ModeloAsistente.texto(de: e), en: chat)
            }
            recetasCambiadas += 1
        }
    }

    /// Si el menu de `m` lleva «Guardar como receta»: una nota en una
    /// conversacion con el asistente.
    public static func sePuedeGuardarReceta(_ m: MensajeChat, chat: String, almacen: Almacen) -> Bool {
        guard m.clase == "NOTA" else { return false }
        if m.esDelAsistente || almacen.chats.first(where: { $0.id == chat })?.esAsistente == true { return true }
        return (almacen.mensajes[chat] ?? []).contains(where: \.esDelAsistente)
    }

    private func avisar(_ texto: String, en chat: String, clave: Bool = false) {
        aviso = (chat, texto)
        avisoDeClave = clave
        let ahora = texto
        DispatchQueue.main.asyncAfter(deadline: .now() + (clave ? 20 : 6)) { [weak self] in
            MainActor.assumeIsolated {
                if self?.aviso?.texto == ahora { self?.quitarAviso() }
            }
        }
    }

    private func modelo(_ almacen: Almacen) -> ModeloAsistente {
        if let m = modelo, m.almacen == ObjectIdentifier(almacen) { return m.modelo }
        let m = ModeloAsistente(almacen: almacen)
        modelo = (ObjectIdentifier(almacen), m)
        return m
    }

    // MARK: Al enviar

    /// Si lo recien escrito en `chat` es para el asistente, contesta.
    /// Devuelve los turnos lanzados (las pruebas los esperan).
    @discardableResult
    public func alEnviar(_ nuevos: [MensajeChat], chat: String, almacen: Almacen) -> [Task<Void, Never>] {
        let suyo = almacen.chats.first { $0.id == chat }?.esAsistente ?? false
        var lanzados: [Task<Void, Never>] = []
        for m in nuevos where !m.esDelAsistente && (m.clase == "NOTA" || m.clase == "IMAGEN") {
            guard suyo || iaMencionaAlAsistente(texto: m.texto) else { continue }
            if ocupado(chat) {
                avisar("El asistente aún está contestando: espera o pulsa Parar.", en: chat)
                continue
            }
            // Ocupado desde ya: dos preguntas seguidas no lanzan dos turnos.
            let corte = CorteIa.nuevo()
            contestando[chat] = corte
            lanzados.append(Task { await lanzar(m, chat: chat, almacen: almacen, corte: corte) })
        }
        return lanzados
    }

    private func lanzar(_ m: MensajeChat, chat: String, almacen: Almacen, corte: CorteIa) async {
        let nucleo = almacen.nucleo
        defer { if contestando[chat] === corte { contestando[chat] = nil } }
        let modelo = self.modelo(almacen)
        let motor: ModeloAsistente.Motor
        if AsistenteEnElChat.turnoDePrueba != nil {
            motor = .descargado("prueba")
        } else {
            motor = await modelo.comprobarMotor()
        }
        // Sin motor no nace ninguna burbuja: lo dice la barra (Windows).
        switch motor {
        case .ninguno:
            avisar(ModeloAsistente.textoSinMotor, en: chat, clave: true)
            return
        case let .nube(nombre, conClave) where !conClave:
            avisar("Falta la clave de \(nombre): pulsa «Poner la clave».", en: chat, clave: true)
            return
        default: break
        }
        let pregunta = m.id
        let inicio: InicioIa?
        do {
            inicio = try await Task.detached(priority: .userInitiated) {
                try nucleo.iaAlEnviar(chat: chat, mensaje: pregunta)
            }.value
        } catch {
            avisar("El asistente no pudo contestar: \(ModeloAsistente.texto(de: error))", en: chat)
            return
        }
        guard let inicio else { return }
        let nota = inicio.respuesta.id
        almacen.anadir([inicio.respuesta], en: chat)
        if case .apple = motor {
            await conApple(inicio, pregunta: m, chat: chat, almacen: almacen)
            return
        }
        let manos = ManosDelAsistente(nucleo: nucleo, corte: corte) { [weak self] texto, responder in
            guard let self else { return responder(false) }
            self.preguntas[nota] = ModeloAsistente.Pregunta(texto: texto, responder: responder)
        }
        let oyente = OyenteChatDelAsistente { v in
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    let s = AsistenteEnElChat.compartido
                    if !v.acabado { s.vivos[v.nota] = v }
                    // Lo dibujado en un lienzo abierto: que se relea.
                    NotificationCenter.default.post(name: .pixpinLienzoDesdeElAsistente, object: chat)
                }
            }
        }
        let ajustes = modelo.ajustes()
        let desfase = Int64(TimeZone.current.secondsFromGMT()) * 1000
        let deprueba = AsistenteEnElChat.turnoDePrueba
        let r: Result<FinChatIa, Error> = await withCheckedContinuation { c in
            DispatchQueue.global(qos: .utility).async {
                c.resume(returning: Result {
                    if let deprueba { return try deprueba(nucleo, chat, inicio, oyente, manos, corte) }
                    return try iaTurnoEnChat(ajustes: ajustes, nucleo: nucleo, chat: chat, inicio: inicio,
                                             desfaseMs: desfase, app: manos, oyente: oyente, corte: corte)
                })
            }
        }
        vivos[nota] = nil
        preguntas[nota] = nil
        pensados[nota] = nil
        switch r {
        case let .success(fin):
            await acabar(fin.respuesta, chat: chat, almacen: almacen)
            if let ref = fin.abrirLienzo {
                if almacen.mensajes[chat]?.contains(where: { $0.referencia == ref }) != true {
                    await almacen.abrir(chat: chat)
                }
                if let lienzo = almacen.mensajes[chat]?.last(where: { $0.referencia == ref }) {
                    Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: lienzo, foto: nil))
                }
            }
        case let .failure(e):
            avisar("El asistente no pudo contestar: \(ModeloAsistente.texto(de: e))", en: chat)
            await almacen.abrir(chat: chat)
        }
    }

    /// La respuesta acabada en lo cargado; y lo que nacio con ella (su
    /// lienzo, lo leido) se ve releyendo la ultima pagina.
    private func acabar(_ respuesta: MensajeChat, chat: String, almacen: Almacen) async {
        almacen.sustituir(respuesta, en: chat)
        if !almacen.quedanPosteriores(chat: chat) { await almacen.abrir(chat: chat) }
        await almacen.cargarChats()
    }

    /// **Con Apple Intelligence** (no pasa por Rust): la respuesta entera y,
    /// si dibujo, su lienzo.
    private func conApple(_ inicio: InicioIa, pregunta m: MensajeChat, chat: String, almacen: Almacen) async {
        let nucleo = almacen.nucleo
        let historial = (almacen.mensajes[chat] ?? [])
            .filter { $0.clase == "NOTA" && $0.id != m.id && $0.id != inicio.respuesta.id && !$0.texto.isEmpty }
            .suffix(ModeloAsistente.historialMaximo)
            .map { MensajeIa(delUsuario: !$0.esDelAsistente, texto: $0.texto) }
        let suyo = almacen.chats.first { $0.id == chat }?.esAsistente ?? false
        var pedido = m.texto
        if !suyo, let i = pedido.range(of: "@asistente", options: .caseInsensitive) {
            pedido.removeSubrange(i)
        }
        pedido = pedido.trimmingCharacters(in: .whitespacesAndNewlines)
        var texto: String
        var lienzo: String?
        do {
            if let r = try await preguntarAApple(pedido.isEmpty ? "Hola" : pedido, historial: Array(historial)) {
                texto = r.texto
                lienzo = r.lienzo
            } else {
                texto = ModeloAsistente.textoSinMotor
            }
        } catch {
            texto = "⚠️ " + ModeloAsistente.texto(de: error)
        }
        let nota = inicio.respuesta.id
        let hecho = try? await Task.detached(priority: .userInitiated) {
            try nucleo.iaResponder(chat: chat, nota: nota, texto: texto, lienzo: lienzo)
        }.value
        if let hecho { await acabar(hecho, chat: chat, almacen: almacen) }
    }

    // MARK: Lo pensado

    /// Lo pensado de una respuesta guardada (del disco la primera vez).
    func pensado(nota: String, chat: String, nucleo: Nucleo) async -> PensadoIa? {
        if let p = pensados[nota] { return p }
        let p = await Task.detached(priority: .utility) { nucleo.iaPensado(chat: chat, nota: nota) }.value
        pensados[nota] = .some(p)
        return p
    }
}

// MARK: - La burbuja

/// **El texto de una respuesta del asistente**: lo pensado encima, el texto
/// (en vivo mientras contesta) y la tarjeta de «¿seguro?» debajo. El chat
/// de Proyectos lo pone en la burbuja de una NOTA con `esDelAsistente`.
public struct NotaDelAsistente: View {
    /// **La respuesta como se lee** (Windows, 17-oct): las listas del
    /// Markdown como listas (•, ◦, 1.) y las negritas en negrita. Lo
    /// guardado no cambia: copiar copia lo que escribio el modelo.
    static func formateado(_ texto: String) -> AttributedString {
        let f = iaFormatoDeRespuesta(texto: texto)
        let ns = NSMutableAttributedString(string: f.texto)
        let largo = (f.texto as NSString).length
        for t in f.negritas where Int(t.inicio) + Int(t.largo) <= largo {
            ns.addAttribute(.inlinePresentationIntent, value: InlinePresentationIntent.stronglyEmphasized.rawValue,
                            range: NSRange(location: Int(t.inicio), length: Int(t.largo)))
        }
        var a = (try? AttributedString(ns, including: \.foundation)) ?? AttributedString(f.texto)
        conEnlaces(&a)
        return a
    }

    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @ObservedObject private var ia = AsistenteEnElChat.compartido
    @State private var guardado: PensadoIa?
    @Environment(\.agujaDelChat) private var aguja
    @Environment(\.escalaChat) private var escala

    public init(mensaje: MensajeChat, chat: String, almacen: Almacen) {
        self.mensaje = mensaje
        self.chat = chat
        self.almacen = almacen
    }

    private var vivo: VivoIa? { ia.vivos[mensaje.id] }

    public var body: some View {
        VStack(alignment: .leading, spacing: 4 * escala) {
            if let p = vivo?.pensado ?? guardado, p.algo {
                SeccionPensado(pensado: p, nota: mensaje.id)
            }
            let texto = vivo?.texto ?? mensaje.texto
            Text(aguja.isEmpty ? NotaDelAsistente.formateado(texto) : textoResaltado(texto, aguja: aguja))
                .textSelection(.enabled)
            if let p = ia.preguntas[mensaje.id] {
                TarjetaConfirmar(pregunta: p.texto) { si in ia.responder(nota: mensaje.id, si: si) }
            }
        }
        .task(id: mensaje.texto) {
            guard vivo == nil else { return }
            guardado = await ia.pensado(nota: mensaje.id, chat: chat, nucleo: almacen.nucleo)
        }
    }
}

/// **Lo que penso, encima de la respuesta** (`ventana_chat/pensado.rs`):
/// «Pensando…» abierto mientras piensa; al acabar, «Pensó 12 s» plegado; la
/// peticion mejorada («📝 …») y «Contexto renovado» si el turno empezo con
/// la conversacion reiniciada.
struct SeccionPensado: View {
    let pensado: PensadoIa
    let nota: String
    @State private var abierto = false
    @Environment(\.escalaChat) private var escala

    /// Mientras llega se ensena solo el final (lo que se esta leyendo).
    private static let enVivoLetras = 700

    private var cabecera: String {
        if pensado.pensando { return "Pensando…" }
        if let s = pensado.segundos { return "Pensó \(max(1, Int(s.rounded()))) s" }
        return "Lo pensado"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 3 * escala) {
            if !pensado.pensamiento.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                Button {
                    abierto.toggle()
                } label: {
                    Label(cabecera, systemImage: (abierto || pensado.pensando) ? "chevron.down" : "chevron.right")
                        .font(.system(size: 13 * escala, weight: .medium))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                if abierto || pensado.pensando {
                    Text(pensado.pensando ? String(pensado.pensamiento.suffix(SeccionPensado.enVivoLetras))
                                          : pensado.pensamiento)
                        .font(.system(size: 13 * escala))
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                        .padding(.leading, 8 * escala)
                        .overlay(alignment: .leading) {
                            Rectangle().fill(.tertiary).frame(width: 2)
                        }
                }
            }
            if !pensado.entiendo.isEmpty {
                Text("📝 " + pensado.entiendo)
                    .font(.system(size: 13 * escala))
                    .foregroundStyle(.secondary)
                    .lineLimit(abierto ? nil : 1)
                    .onTapGesture { abierto.toggle() }
            }
            if pensado.reinicio {
                Label("Contexto renovado", systemImage: "arrow.triangle.2.circlepath")
                    .font(.system(size: 12 * escala))
                    .foregroundStyle(.tertiary)
            }
        }
    }
}

/// **La tarjeta de «¿seguro?» dentro de la burbuja**: lo que va a hacer y
/// «Sí» (en rojo: borra o envia) y «No» (lo de por omision).
struct TarjetaConfirmar: View {
    let pregunta: String
    let contestar: (Bool) -> Void
    @Environment(\.escalaChat) private var escala

    var body: some View {
        VStack(alignment: .trailing, spacing: 8 * escala) {
            Text("⚠️ " + pregunta)
                .font(.system(size: 14 * escala))
                .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 10 * escala) {
                Button("Sí") { contestar(true) }
                    .buttonStyle(.bordered)
                    .tint(.red)
                Button("No") { contestar(false) }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.cancelAction)
            }
        }
        .padding(10 * escala)
        .frame(minWidth: 260 * escala)
        .background(Color.red.opacity(0.10), in: RoundedRectangle(cornerRadius: 10 * escala))
    }
}

// MARK: - La barra de encima de la caja

/// **Encima de la caja de escribir** del chat de Proyectos: «Contestando…
/// Parar» mientras el asistente contesta en este chat, su aviso (con
/// «Poner la clave» si falta), las recetas del chat «Asistente» y la lista
/// de `@` mientras se escribe una mencion.
public struct BarraDelAsistente: View {
    let chat: String
    @Binding var borrador: String
    let almacen: Almacen
    @ObservedObject private var ia = AsistenteEnElChat.compartido
    @State private var opciones: [OpcionMencionIa] = []
    @State private var recetas: [RecetaIa] = []
    /// La receta que se cambia (su hoja).
    @State private var cambiando: RecetaIa?
    /// La hoja de una receta nueva («+» al final de la fila).
    @State private var nueva = false
    @State private var poniendoClave = false
    @Environment(\.escalaChat) private var f

    public init(chat: String, borrador: Binding<String>, almacen: Almacen) {
        self.chat = chat
        _borrador = borrador
        self.almacen = almacen
    }

    private var esSuChat: Bool { almacen.chats.first { $0.id == chat }?.esAsistente ?? false }

    /// Lo escrito tras el `@` que se esta escribiendo al final, si lo hay.
    private var escrito: String? { iaConsultaDeMencion(antes: borrador) }

    public var body: some View {
        VStack(alignment: .leading, spacing: 4 * f) {
            if let a = ia.aviso, a.chat == chat {
                HStack(alignment: .top) {
                    Text(a.texto).font(.system(size: 13 * f)).foregroundStyle(.secondary)
                    Spacer()
                    if ia.avisoDeClave {
                        Button("Poner la clave") { poniendoClave = true }
                            .buttonStyle(.borderedProminent)
                            .controlSize(.small)
                    }
                    Button { ia.quitarAviso() } label: { Image(systemName: "xmark") }
                        .buttonStyle(.plain).foregroundStyle(.secondary)
                }
            }
            if ia.ocupado(chat) {
                HStack {
                    ProgressView().controlSize(.small)
                    Text("El asistente está contestando…").font(.system(size: 13 * f)).foregroundStyle(.secondary)
                    Spacer()
                    Button("Parar") { ia.parar(chat) }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .keyboardShortcut(".", modifiers: .command)
                }
            }
            if esSuChat, escrito == nil {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6 * f) {
                        ForEach(Array(recetas.enumerated()), id: \.element.id) { i, r in
                            Button { pedir(r) } label: {
                                Text(r.icono.isEmpty ? r.nombre : r.icono + " " + r.nombre).lineLimit(1)
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .disabled(ia.ocupado(chat))
                            .contextMenu { menu(r, i) }
                        }
                        // Una receta desde cero (`asistente/recetas.rs`).
                        Button { nueva = true } label: {
                            if recetas.isEmpty { Label("Receta", systemImage: "plus") } else { Image(systemName: "plus") }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .help("Una receta nueva: un pedido que se repite, como botón encima de la caja")
                        .accessibilityLabel("Receta nueva")
                    }
                }
            }
            if escrito != nil, !opciones.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6 * f) {
                        ForEach(opciones, id: \.mencion) { o in
                            Button(o.rotulo) { poner(o) }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                        }
                    }
                }
            }
        }
        .padding(.horizontal, 14 * f)
        .task(id: "\(chat)·\(ia.recetasCambiadas)·\(esSuChat)") {
            guard esSuChat else {
                recetas = []
                return
            }
            let nucleo = almacen.nucleo
            recetas = await Task.detached(priority: .utility) { nucleo.iaRecetas() }.value
        }
        .sheet(item: $cambiando) { r in
            HojaReceta(receta: r) { nueva in
                cambiando = nil
                guard let nueva else { return }
                ia.cambiarRecetas(almacen.nucleo, en: chat) {
                    try $0.iaRecetaCambiar(id: nueva.id, nombre: nueva.nombre, icono: nueva.icono, pedido: nueva.pedido)
                }
            }
        }
        .sheet(isPresented: $nueva) {
            HojaReceta(receta: RecetaIa(id: "", nombre: "", icono: "", pedido: ""), nueva: true) { hecha in
                nueva = false
                guard let hecha else { return }
                ia.cambiarRecetas(almacen.nucleo, en: chat) {
                    _ = try $0.iaRecetaNueva(nombre: hecha.nombre, icono: hecha.icono, pedido: hecha.pedido)
                }
            }
        }
        .sheet(isPresented: $poniendoClave) {
            HojaClaves(almacen: almacen) { ia.quitarAviso() }
        }
        .task(id: escrito) {
            guard let escrito else {
                opciones = []
                return
            }
            let nucleo = almacen.nucleo
            let chat = self.chat
            opciones = await Task.detached(priority: .userInitiated) {
                nucleo.iaOpcionesMencion(chat: chat, escrito: escrito)
            }.value
        }
    }

    /// Un toque en una receta: su pedido, como si se hubiera escrito.
    private func pedir(_ r: RecetaIa) {
        let almacen = self.almacen
        let chat = self.chat
        Task {
            if let m = await almacen.escribir(nota: r.pedido, en: chat) {
                AsistenteEnElChat.compartido.alEnviar([m], chat: chat, almacen: almacen)
            }
        }
    }

    /// La pulsacion larga en una receta: cambiarla, su icono, moverla o
    /// borrarla.
    @ViewBuilder
    private func menu(_ r: RecetaIa, _ i: Int) -> some View {
        let nucleo = almacen.nucleo
        Button("Cambiar el nombre o el pedido…") { cambiando = r }
        Menu("Icono") {
            ForEach(iaIconosDeReceta(), id: \.self) { icono in
                Button(icono) {
                    ia.cambiarRecetas(nucleo, en: chat) {
                        try $0.iaRecetaCambiar(id: r.id, nombre: r.nombre, icono: icono, pedido: r.pedido)
                    }
                }
            }
        }
        Divider()
        Button("Mover a la izquierda") { ia.cambiarRecetas(nucleo, en: chat) { try $0.iaRecetaMover(id: r.id, paso: -1) } }
            .disabled(i == 0)
        Button("Mover a la derecha") { ia.cambiarRecetas(nucleo, en: chat) { try $0.iaRecetaMover(id: r.id, paso: 1) } }
            .disabled(i == recetas.count - 1)
        Divider()
        Button("Borrar la receta", role: .destructive) {
            ia.cambiarRecetas(nucleo, en: chat) { try $0.iaRecetaBorrar(id: r.id) }
        }
    }

    /// Escribe la opcion elegida en lugar de lo escrito desde el `@`.
    private func poner(_ o: OpcionMencionIa) {
        guard escrito != nil, let arroba = borrador.lastIndex(of: "@") else { return }
        borrador = String(borrador[..<arroba]) + o.mencion + " "
        opciones = []
    }
}

extension RecetaIa: Identifiable {}

/// **Cambiar una receta**: su nombre, su icono y lo que pide.
struct HojaReceta: View {
    let receta: RecetaIa
    /// Una desde cero: sin nombre ni icono los pone el nucleo (el principio
    /// del pedido y uno segun lo que pide).
    var nueva = false
    let acabar: (RecetaIa?) -> Void
    @State private var nombre = ""
    @State private var icono = ""
    @State private var pedido = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Icono", selection: $icono) {
                        if nueva { Text("Según lo que pide").tag("") }
                        ForEach(iaIconosDeReceta(), id: \.self) { Text($0).tag($0) }
                        if !icono.isEmpty, !iaIconosDeReceta().contains(icono) { Text(icono).tag(icono) }
                    }
                    TextField(nueva ? "Nombre (si no, el principio del pedido)" : "Nombre", text: $nombre)
                }
                Section("Lo que pide") {
                    TextEditor(text: $pedido).frame(minHeight: 120)
                }
            }
            .navigationTitle(nueva ? "Receta nueva" : "Receta")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") { acabar(nil) }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Guardar") {
                        acabar(RecetaIa(id: receta.id, nombre: nombre, icono: icono, pedido: pedido))
                    }
                    .disabled((!nueva && nombre.trimmingCharacters(in: .whitespaces).isEmpty)
                        || pedido.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
        .onAppear {
            nombre = receta.nombre
            icono = receta.icono
            pedido = receta.pedido
        }
    }
}
