// «Al chat» en un paso (`captura2/al_chat.rs` de Windows, `SelectorProyecto`
// del Mac): una hoja con buscador y los proyectos (el ultimo al que se mando
// una captura, el primero; luego los de la lista en su orden), un comentario
// opcional y «Enviar». Con el teclado del iPad, 1-6 eligen uno de los seis
// primeros e Intro manda.
//
// La captura entra como una imagen del chat llamada `captura-<ms>.png`, como
// la deja Windows, con `Almacen.adjuntar` (la misma copia que soltar un
// fichero en el chat); el comentario va detras como una nota.

import PixPinCore
import SwiftUI
import UIKit

enum SelectorProyectoIPad {
    /// Ensena la hoja encima de todo; al elegir, manda `imagen` a ese chat.
    @MainActor
    static func elegir(para imagen: CGImage, almacen: Almacen, alMandar: (() -> Void)? = nil) {
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 420, height: 560)) { cerrar in
            VistaSelectorProyecto(almacen: almacen) { chat, comentario in
                cerrar()
                guard let chat else { return }
                Task {
                    await mandar(imagen, a: chat, comentario: comentario, almacen: almacen)
                    alMandar?()
                }
            }
        }
    }

    /// La captura al chat como `captura-<ms>.png` (PNG fuera del hilo
    /// principal) y el comentario, si lo hay, como una nota detras.
    @MainActor
    static func mandar(_ imagen: CGImage, a chat: String, comentario: String, almacen: Almacen) async {
        UserDefaults.standard.set(chat, forKey: ListaDeProyectos.clave)
        let nombre = ListaDeProyectos.nombreDeFichero(fecha: Date())
        let fichero = await ImagenIPad.fuera { () -> URL? in
            guard let png = ImagenIPad.png(imagen) else { return nil }
            let carpeta = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-alchat-\(UUID().uuidString)", isDirectory: true)
            do {
                try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let f = carpeta.appendingPathComponent(nombre)
                try png.write(to: f)
                return f
            } catch {
                return nil
            }
        }
        guard let fichero else { return avisarFalloPin() }
        let hechos = await almacen.adjuntar(urls: [fichero], en: chat)
        try? FileManager.default.removeItem(at: fichero.deletingLastPathComponent())
        guard !hechos.isEmpty else { return avisarFalloPin() }
        let texto = comentario.trimmingCharacters(in: .whitespacesAndNewlines)
        if !texto.isEmpty { await almacen.escribir(nota: texto, en: chat) }
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }
}

/// La hoja: buscador, proyectos, comentario y «Enviar».
struct VistaSelectorProyecto: View {
    @ObservedObject var almacen: Almacen
    /// El chat elegido (nil: cancelado) y el comentario.
    let alElegir: (String?, String) -> Void

    @State private var texto = ""
    @State private var comentario = ""
    @State private var elegido: String?
    @FocusState private var enBuscador: Bool

    private var fichas: [FichaChat] {
        let ultimo = UserDefaults.standard.string(forKey: ListaDeProyectos.clave)
        let lista = almacen.chats.filter { !$0.esAsistente && !$0.archivado }
        return ListaDeProyectos.ordenar(lista, id: \.id, ultimo: ultimo)
    }

    private var filtradas: [FichaChat] {
        ListaDeProyectos.filtrar(fichas, nombre: { nombre($0) }, texto: texto)
    }

    private func nombre(_ f: FichaChat) -> String {
        f.esGuardados ? "Mensajes guardados" : f.nombre
    }

    var body: some View {
        let lista = filtradas
        let ultimo = UserDefaults.standard.string(forKey: ListaDeProyectos.clave)
        NavigationStack {
            VStack(spacing: 10) {
                HStack {
                    Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                    TextField("Buscar proyecto…", text: $texto)
                        .focused($enBuscador)
                        .submitLabel(.send)
                        .onSubmit { enviar(lista) }
                }
                .padding(10)
                .background(Color(uiColor: .secondarySystemBackground), in: RoundedRectangle(cornerRadius: 10))
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Paleta.v2.acento.opacity(enBuscador ? 1 : 0)))
                List {
                    Section(texto.isEmpty ? "Recientes" : "Proyectos") {
                        ForEach(Array(lista.prefix(80).enumerated()), id: \.element.id) { i, f in
                            Button { elegido = f.id } label: {
                                HStack(spacing: 10) {
                                    Image(systemName: f.esGuardados ? "bookmark" : "bubble.left")
                                        .foregroundStyle(Paleta.v2.acento)
                                    Text(nombre(f)).lineLimit(1)
                                    Spacer()
                                    if f.id == ultimo {
                                        Text("el último").font(.caption).foregroundStyle(.secondary)
                                    }
                                    if i < ListaDeProyectos.conTecla {
                                        Text("\(i + 1)")
                                            .font(.caption.monospacedDigit().bold())
                                            .frame(width: 22, height: 22)
                                            .background(RoundedRectangle(cornerRadius: 5).fill(Color.secondary.opacity(0.2)))
                                    }
                                }
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .listRowBackground(elegidoDe(lista) == f.id ? Paleta.v2.acento.opacity(0.18) : nil)
                        }
                    }
                }
                .listStyle(.plain)
                .overlay {
                    if almacen.chats.isEmpty {
                        ProgressView()
                    } else if lista.isEmpty {
                        Text("Ningún proyecto se llama así").foregroundStyle(.secondary)
                    }
                }
                HStack(spacing: 10) {
                    TextField("Añade un comentario (opcional)", text: $comentario)
                        .textFieldStyle(.roundedBorder)
                        .onSubmit { enviar(lista) }
                    Button("Enviar") { enviar(lista) }
                        .buttonStyle(.borderedProminent)
                        .disabled(elegidoDe(lista) == nil)
                        .keyboardShortcut(.defaultAction)
                }
            }
            .padding(14)
            .navigationTitle("Al chat")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") { alElegir(nil, "") }
                        .keyboardShortcut(.cancelAction)
                }
            }
        }
        .task {
            await almacen.cargarChats()
            enBuscador = true
        }
        .onChange(of: texto) { antes, ahora in
            // Con el buscador vacio, una cifra 1-6 elige y manda.
            guard antes.isEmpty, ahora.count == 1, let n = Int(ahora), n >= 1,
                  n <= ListaDeProyectos.conTecla, n <= fichas.count else { return }
            alElegir(fichas[n - 1].id, comentario)
        }
    }

    /// El elegido a mano o, si no, el primero de lo que se ve.
    private func elegidoDe(_ lista: [FichaChat]) -> String? {
        if let elegido, lista.contains(where: { $0.id == elegido }) { return elegido }
        return lista.first?.id
    }

    private func enviar(_ lista: [FichaChat]) {
        guard let chat = elegidoDe(lista) else { return }
        alElegir(chat, comentario)
    }
}
