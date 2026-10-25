// **Las ventanitas de la voz desde un chat** (Mac `VozVentanas/VozEnElChat.swift`,
// Windows `abrir_telepronter`, `abrir_pronunciar`, `abrir_conversacion` de
// `ventana_chat.rs` y `voz_en_chat.rs`). La hoja del clip
// (`Chat/CajaEscribir.swift`) da `AccionAdjuntar.voz(_)` y el chat llama a
// `VozEnElChat.abrir`; lo grabado entra en el chat como nota de voz
// (`Nucleo.adjuntarGrabacion`, que es lo que viaja) y, si no trae su texto,
// se pasa a texto.
//
// Y lo de **quien llama** en la llamada secreta (`ventana_chat/quien_llama.rs`):
// el nombre que sale al sonar, apuntado en el iPad y no en el mensaje (no
// viaja, como en el movil). Lo usa el menu «Recordármelo» de una nota de voz
// (`EntradaQuienLlama`).

import PixPinCore
import SwiftUI
import UIKit

/// Las ventanitas de la voz que se abren desde la hoja del clip.
public enum HerramientaDeVoz: String, CaseIterable, Hashable, Sendable {
    case conversacion, telepronter, pronunciar, clase

    /// Lo que pone en la hoja de adjuntar (los textos de Windows).
    public var rotulo: String {
        switch self {
        case .conversacion: return "Conversación"
        case .telepronter: return "Teleprónter"
        case .pronunciar: return "Pronunciar"
        case .clase: return "Grabar clase"
        }
    }

    public var simbolo: String {
        switch self {
        case .conversacion: return "person.2.wave.2"
        case .telepronter: return "text.alignleft"
        case .pronunciar: return "waveform.and.mic"
        case .clase: return "record.circle"
        }
    }
}

// MARK: - Que se lee en el teleprónter (puro)

enum FuentesTelepronter {
    enum Fuente: Equatable {
        case borrador
        /// Una nota escrita (su texto).
        case nota(String)
        /// Un `.txt`/`.md` adjunto (su ruta).
        case adjunto(ruta: String)
    }

    struct Entrada: Equatable {
        var rotulo: String
        var fuente: Fuente
    }

    /// Si ese nombre de fichero es de los que el teleprónter sabe leer:
    /// solo texto plano (un `.docx` pintaria su ZIP).
    static func esLeible(_ nombre: String) -> Bool {
        let n = nombre.lowercased()
        return n.hasSuffix(".txt") || n.hasSuffix(".md")
    }

    /// **De donde sacar el texto** (`menu_del_telepronter`): lo escrito en
    /// la caja, las notas de la conversacion y los `.txt`/`.md` adjuntos,
    /// lo mas nuevo primero y doce como mucho. Una nota sale por su primera
    /// linea recortada; un adjunto, por su nombre.
    static func entradas(borrador: String, mensajes: [MensajeChat]) -> [Entrada] {
        var v: [Entrada] = []
        if !borrador.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            v.append(Entrada(rotulo: "Lo escrito en la caja", fuente: .borrador))
        }
        for m in mensajes.reversed() {
            if v.count >= 12 { break }
            if let ruta = m.ruta {
                if esLeible(m.nombre) { v.append(Entrada(rotulo: m.nombre, fuente: .adjunto(ruta: ruta))) }
                continue
            }
            guard m.clase != "VOZ", let linea = m.texto.split(separator: "\n").first(where: {
                !$0.trimmingCharacters(in: .whitespaces).isEmpty
            }) else { continue }
            let l = linea.trimmingCharacters(in: .whitespaces)
            let corto = l.count > 48 ? String(l.prefix(48)) + "…" : l
            v.append(Entrada(rotulo: corto, fuente: .nota(m.texto)))
        }
        return v
    }
}

/// «¿Qué quieres leer?»: de donde sale el texto del teleprónter.
struct ElegirFuenteTelepronter: View {
    let entradas: [FuentesTelepronter.Entrada]
    let elegir: (Int?) -> Void

    var body: some View {
        NavigationStack {
            List(entradas.indices, id: \.self) { i in
                Button { elegir(i) } label: {
                    Label(entradas[i].rotulo,
                          systemImage: entradas[i].fuente == .borrador ? "doc.text" : "captions.bubble")
                        .lineLimit(1)
                }
            }
            .navigationTitle("Teleprónter: qué leer")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { elegir(nil) } }
            }
        }
    }
}

// MARK: - Abrirlas desde un chat

@MainActor
public enum VozEnElChat {
    static let sinTexto = "No hay ningún texto que leer: escribe algo en la caja o adjunta un .txt o un .md"

    /// Lo que se dice: en la tarjeta flotante (encima de todo).
    static func avisar(_ t: String) {
        AvisoFlotante.compartido.mostrar(t, titulo: "Voz")
    }

    /// **Abre una ventanita de la voz** para el chat `chat`. `borrador`: lo
    /// escrito ahora en la caja (el teleprónter lo ofrece).
    public static func abrir(_ h: HerramientaDeVoz, chat: String, almacen: Almacen, borrador: String = "") {
        if GrabadoraVoz.compartida.estado != .quieta {
            return avisar("Primero termina la nota de voz que se está grabando")
        }
        switch h {
        case .conversacion: conversacion(chat: chat, almacen: almacen)
        case .telepronter: telepronter(chat: chat, almacen: almacen, borrador: borrador)
        case .pronunciar: pronunciar(chat: chat, almacen: almacen)
        case .clase: GrabarClase.abrir(chat: chat, almacen: almacen)
        }
    }

    /// **Teleprónter**: de donde sacar el texto (si hay mas de una cosa), y
    /// la ventanita.
    static func telepronter(chat: String, almacen: Almacen, borrador: String) {
        if Telepronter.abierto { return avisar("El teleprónter ya está abierto") }
        let entradas = FuentesTelepronter.entradas(borrador: borrador, mensajes: almacen.mensajes[chat] ?? [])
        guard !entradas.isEmpty else { return avisar(sinTexto) }
        if entradas.count == 1 {
            return leer(entradas[0].fuente, chat: chat, almacen: almacen, borrador: borrador)
        }
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 440, height: 520)) { cerrar in
            ElegirFuenteTelepronter(entradas: entradas) { i in
                cerrar()
                guard let i else { return }
                // Una vuelta despues: que la hoja ya se haya ido.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                    MainActor.assumeIsolated {
                        leer(entradas[i].fuente, chat: chat, almacen: almacen, borrador: borrador)
                    }
                }
            }
        }
    }

    /// El texto de una fuente (un adjunto se lee del disco al elegirlo) y
    /// la ventanita con el.
    private static func leer(_ f: FuentesTelepronter.Fuente, chat: String, almacen: Almacen, borrador: String) {
        _ = Task<Void, Never> {
            let texto: String
            switch f {
            case .borrador: texto = borrador
            case let .nota(t): texto = t
            case let .adjunto(ruta):
                guard let u = await almacen.url(deRuta: ruta, en: chat),
                      let d = await Task.detached(operation: { try? Data(contentsOf: u) }).value
                else { return avisar(sinTexto) }
                // Sin exigir UTF-8: una tilde mal codificada no impide leer.
                texto = String(decoding: d, as: UTF8.self)
            }
            let abierto = Telepronter.abrir(texto: texto) { lectura in
                _ = Task<Void, Never> {
                    let g = GrabacionVoz(rutaOrigen: lectura.nota.url.path, duracionMs: lectura.nota.duracionMs,
                                         picos: lectura.nota.picos, texto: lectura.cuerpo, turnos: [])
                    let m = await almacen.adjuntarGrabacion(g, en: chat)
                    avisar(m != nil ? "Lectura guardada como nota de voz" : "No se pudo guardar la lectura")
                }
            }
            if !abierto { avisar(sinTexto) }
        }
    }

    /// **Pronunciar**, con lo del chat que sirve de guia. Cada toma guardada
    /// entra como nota de voz y se pasa a texto en el idioma practicado.
    static func pronunciar(chat: String, almacen: Almacen) {
        let guias = Pronunciar.guiasDelChat(almacen.mensajes[chat] ?? [])
        Pronunciar.abrir(delChat: guias, resolver: { await almacen.url(deRuta: $0, en: chat) }) { toma in
            _ = Task<Void, Never> {
                let g = GrabacionVoz(rutaOrigen: toma.nota.url.path, duracionMs: toma.nota.duracionMs,
                                     picos: toma.nota.picos, texto: nil, turnos: [])
                guard let m = await almacen.adjuntarGrabacion(g, en: chat) else { return }
                await almacen.transcribir(m, en: chat, idioma: toma.idioma)
            }
        }
    }

    /// **La conversacion por turnos**: entra con sus turnos y se pasa a
    /// texto con los nombres.
    static func conversacion(chat: String, almacen: Almacen) {
        if ConversacionPorTurnos.abierto { return avisar("La conversación ya está abierta") }
        ConversacionPorTurnos.abrir { nota, turnos in
            _ = Task<Void, Never> {
                let g = GrabacionVoz(rutaOrigen: nota.url.path, duracionMs: nota.duracionMs, picos: nota.picos,
                                     texto: nil, turnos: turnos)
                guard let m = await almacen.adjuntarGrabacion(g, en: chat) else { return }
                avisar("Pasando la conversación a texto…")
                await almacen.transcribir(m, en: chat)
            }
        }
    }
}

extension Almacen {
    /// Guarda en `chat` una nota de las ventanitas de la voz
    /// (`Nucleo.adjuntarGrabacion`) y la anade al final de lo cargado. El
    /// fichero grabado se borra: ya esta dentro del proyecto.
    @discardableResult
    func adjuntarGrabacion(_ g: GrabacionVoz, en chat: String) async -> MensajeChat? {
        let nucleo = self.nucleo
        let resultado = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
            Result { try nucleo.adjuntarGrabacion(chat: chat, grabacion: g) }
        }.value
        switch resultado {
        case let .success(m):
            try? FileManager.default.removeItem(atPath: g.rutaOrigen)
            anadir([m], en: chat)
            ponerError(nil)
            return m
        case let .failure(e):
            ponerError("\(e)")
            return nil
        }
    }
}

// MARK: - Quien llama

/// **Quien llama** en la llamada secreta de una nota de voz: el nombre
/// apuntado o, si no, el de la nota sin extension. Se guarda en el iPad
/// (`Nucleo.ponerQuienLlama`); aqui se recuerda lo leido para no ir al
/// disco cada vez que sale el menu.
@MainActor
enum QuienLlama {
    /// Lo mas largo que se deja teclear: un nombre, no una frase (`MAXIMO`).
    static let maximo = 40
    private static var leidos: [String: String?] = [:]

    /// Si poner hora a `m` programa una llamada secreta (`es_llamada`).
    static func esLlamada(_ m: MensajeChat) -> Bool { m.clase == "VOZ" }

    /// El que se ensena ahora para la nota `m` (vacio: «Llamada»).
    static func actual(_ m: MensajeChat, nucleo: Nucleo) -> String {
        let apuntado: String?
        if let ya = leidos[m.id] {
            apuntado = ya
        } else {
            apuntado = nucleo.quienLlama(mensaje: m.id)
            leidos[m.id] = .some(apuntado)
        }
        let n = apuntado ?? nombreDeLlamada(nombre: m.nombre)
        return n.trimmingCharacters(in: .whitespaces).isEmpty ? "Llamada" : n
    }

    /// Lo escrito, limpio: sin blancos a los lados y `maximo` letras.
    nonisolated static func limpiar(_ nombre: String) -> String {
        String(nombre.trimmingCharacters(in: .whitespacesAndNewlines).prefix(maximo))
    }

    /// Apunta quien llama (en blanco, lo olvida y vuelve el de la nota).
    static func poner(_ nombre: String, en m: MensajeChat, nucleo: Nucleo) {
        let limpio = limpiar(nombre)
        leidos[m.id] = .some(limpio.isEmpty ? nil : limpio)
        let id = m.id
        Task.detached(priority: .utility) { try? nucleo.ponerQuienLlama(mensaje: id, nombre: limpio) }
    }

    /// Al ponerle la hora a una nota de voz se apunta quien llama (el que se
    /// ve, aunque no se haya tocado), como el movil al elegir.
    static func alPonerLaHora(_ m: MensajeChat, nucleo: Nucleo) {
        guard esLlamada(m) else { return }
        poner(actual(m, nucleo: nucleo), en: m, nucleo: nucleo)
    }

    /// **«Quién llama:»**: el cuadro para escribirlo (la barra de Windows
    /// encima de la caja). `despues`: al guardar (para seguir con la hora).
    static func preguntar(_ m: MensajeChat, nucleo: Nucleo, despues: (@MainActor () -> Void)? = nil) {
        let alerta = UIAlertController(title: "Quién llama:",
                                       message: "El nombre que sale en la pantalla de la llamada.",
                                       preferredStyle: .alert)
        alerta.addTextField { campo in
            campo.text = actual(m, nucleo: nucleo)
            campo.placeholder = nombreDeLlamada(nombre: m.nombre)
            campo.autocapitalizationType = .words
            campo.clearButtonMode = .whileEditing
            campo.returnKeyType = .done
        }
        alerta.addAction(UIAlertAction(title: "Cancelar", style: .cancel))
        alerta.addAction(UIAlertAction(title: "Guardar", style: .default) { [weak alerta] _ in
            let texto = alerta?.textFields?.first?.text ?? ""
            MainActor.assumeIsolated {
                poner(texto, en: m, nucleo: nucleo)
                despues?()
            }
        })
        ArribaUIKit.presentar(alerta)
    }
}

/// La primera linea del menu de las horas de una nota de voz: «Quién llama:
/// Recado» (`chat-quien-llama`). Nada en lo demas.
struct EntradaQuienLlama: View {
    let mensaje: MensajeChat
    let nucleo: Nucleo

    var body: some View {
        if QuienLlama.esLlamada(mensaje) {
            Button {
                // Una vuelta despues: que el menu ya se haya ido.
                let (m, n) = (mensaje, nucleo)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) {
                    MainActor.assumeIsolated { QuienLlama.preguntar(m, nucleo: n) }
                }
            } label: {
                Label("Quién llama: \(QuienLlama.actual(mensaje, nucleo: nucleo))", systemImage: "person.fill")
            }
        }
    }
}
