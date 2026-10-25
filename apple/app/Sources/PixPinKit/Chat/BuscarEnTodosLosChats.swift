// **El buscador de la lista busca tambien dentro de todos los chats**
// (Windows 97e7f88, d106969 y 02b3124, 25-oct-2026, el usuario: «que el
// buscador busque tambien dentro de todos los chats»). Como en Telegram:
// debajo de los chats que casan por su nombre, la seccion «Mensajes · N»
// con los mensajes de cualquier chat que llevan lo escrito, del mas nuevo al
// mas viejo (`ventana_chat/buscar_en_chats.rs` de Windows).
//
// - El indice y la busqueda son del nucleo (`buscar_en_chats.rs`, sobre
//   `pixpin_ui::buscar_en_chats`): sin mayusculas ni tildes, la frase entera
//   en cualquier sitio, el trozo de una linea con lo encontrado resaltado.
//   Aqui, una `Task` de fondo tras **180 ms** desde la ultima tecla; lo que
//   conteste a una busqueda vieja se tira (`turno`).
// - Cada fila mide lo que una de chat (62): el avatar del chat, su nombre,
//   la fecha del mensaje a la derecha y el trozo debajo, con lo encontrado
//   en negrita sobre el color de enviar al 28 %. La cabecera, 30; «Ver N
//   más», 40, anade otros 20.
// - Pulsar un mensaje abre su chat llevado a el y resaltado (el salto de
//   «Ver en el chat»). Cuando cambian los chats (llego algo al
//   sincronizar) con una busqueda puesta, se repite sola.

import PixPinCore
import SwiftUI

enum TextosBuscarEnChats {
    static let idioma = Locale.preferredLanguages.first ?? "es"
    static let medidas = medidasDeBuscarEnChats()
}

@MainActor
final class MensajesDeLaLista: ObservableObject {
    @Published private(set) var encontrados: [MensajeEncontrado] = []
    @Published private(set) var total: UInt64 = 0
    /// El del nucleo, nacido con la primera busqueda (su indice tambien).
    private var buscador: BuscadorEnChats?
    /// Lo escrito la ultima vez (sin espacios de los lados).
    private var aguja = ""
    /// Cuantos se piden: crece con «Ver más».
    private var tope = TextosBuscarEnChats.medidas.dePrimeras
    private var turno = 0
    private var espera: Task<Void, Never>?

    /// Lo escrito en el buscador cambio: se rearma la espera. Vacio, la
    /// seccion se va ya.
    func escrito(_ texto: String, nucleo: Nucleo) {
        if buscador == nil { buscador = nucleo.buscadorEnChats() }
        let a = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard a != aguja else { return }
        aguja = a
        tope = TextosBuscarEnChats.medidas.dePrimeras
        turno += 1
        espera?.cancel()
        guard !a.isEmpty else {
            encontrados = []
            total = 0
            return
        }
        espera = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(Int(TextosBuscarEnChats.medidas.esperaMs)))
            guard !Task.isCancelled else { return }
            self?.pedir()
        }
    }

    /// Algo cambio en los chats: con una busqueda puesta se repite (el
    /// nucleo relee solo el chat que cambio).
    func releer() {
        if !aguja.isEmpty, !encontrados.isEmpty { pedir() }
    }

    /// «Ver más»: los siguientes.
    func verMas() {
        tope += TextosBuscarEnChats.medidas.dePrimeras
        pedir()
    }

    private func pedir() {
        guard !aguja.isEmpty, let buscador else { return }
        turno += 1
        let (mio, aguja, tope) = (turno, self.aguja, self.tope)
        Task {
            let r = await Task.detached(priority: .userInitiated) {
                buscador.buscar(aguja: aguja, tope: tope)
            }.value
            guard mio == turno else { return }
            encontrados = r.mensajes
            total = r.total
        }
    }
}

/// **La seccion «Mensajes · N»** debajo de las filas de chats.
struct SeccionMensajes: View {
    @ObservedObject var mensajes: MensajesDeLaLista
    let chats: [FichaChat]
    let abrir: (MensajeEncontrado) -> Void

    var body: some View {
        if !mensajes.encontrados.isEmpty {
            let m = TextosBuscarEnChats.medidas
            let vistos = UInt64(mensajes.encontrados.count)
            let mas = mensajes.total > vistos
            let rotulos = rotulosDeBuscarEnChats(locale: TextosBuscarEnChats.idioma, total: mensajes.total,
                                                 mas: mas ? min(UInt64(m.dePrimeras), mensajes.total - vistos) : 0)
            Text(rotulos.mensajes)
                .font(.system(size: 12))
                .foregroundStyle(Paleta.proyectos.apagado)
                .padding(.horizontal, 12)
                .frame(maxWidth: .infinity, minHeight: CGFloat(m.cabecera), maxHeight: CGFloat(m.cabecera),
                       alignment: .leading)
                .background(Paleta.chat.cabecera)
            ForEach(mensajes.encontrados, id: \.self) { e in
                FilaMensajeEncontrado(encontrado: e, chat: chats.first(where: { $0.id == e.chat }))
                    .onTapGesture { abrir(e) }
            }
            if mas {
                Button { mensajes.verMas() } label: {
                    Text(rotulos.verMas)
                        .font(.system(size: 13))
                        .foregroundStyle(Paleta.chat.enviar)
                        .frame(maxWidth: .infinity, minHeight: CGFloat(m.verMas), maxHeight: CGFloat(m.verMas))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }
}

/// Un mensaje encontrado: el avatar de su chat, su nombre, la fecha y el
/// trozo con lo encontrado resaltado.
private struct FilaMensajeEncontrado: View {
    let encontrado: MensajeEncontrado
    let chat: FichaChat?
    @State private var encima = false

    var body: some View {
        HStack(spacing: 12) {
            AvatarDelChat(id: encontrado.chat, nombre: chat?.nombre ?? "", lado: 46, foto: chat?.foto)
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Text(chat?.nombre ?? "")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Paleta.proyectos.texto)
                    Spacer(minLength: 0)
                    Text(TextosFicha.hora(encontrado.cuando))
                        .font(.system(size: 12))
                        .foregroundStyle(Paleta.proyectos.apagado)
                }
                Text(trozo)
                    .font(.system(size: 13))
                    .foregroundStyle(Paleta.proyectos.apagado)
            }
            .lineLimit(1)
        }
        .padding(.horizontal, 10)
        .frame(height: CGFloat(TextosBuscarEnChats.medidas.fila))
        .background(encima ? Paleta.proyectos.filaEncima : .clear)
        .contentShape(Rectangle())
        .onHover { encima = $0 }
    }

    /// El trozo con lo encontrado en negrita y sobre el color de enviar.
    private var trozo: AttributedString {
        let texto = encontrado.texto
        var a = AttributedString(texto)
        for t in encontrado.tramos {
            guard let r = Range(NSRange(location: Int(t.inicio), length: Int(t.largo)), in: texto),
                  let desde = AttributedString.Index(r.lowerBound, within: a),
                  let hasta = AttributedString.Index(r.upperBound, within: a) else { continue }
            a[desde ..< hasta].font = Font.system(size: 13, weight: .bold)
            a[desde ..< hasta].foregroundColor = Paleta.proyectos.texto
            a[desde ..< hasta].backgroundColor = Paleta.chat.enviar.opacity(0.28)
        }
        return a
    }
}
