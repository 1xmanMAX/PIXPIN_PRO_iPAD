// **Un HTML** en su visor (el `VisorHtml.swift` del Mac, sin la tinta web
// todavia): `WKWebView` con el fichero y lo que tenga a su lado (imagenes,
// estilos), el buscador del sistema y el modo noche comun de los visores
// (la luz invertida con el tono conservado, el `invert(1) hue-rotate(180deg)`
// del movil).

import SwiftUI
import UIKit
import WebKit

@MainActor
final class ControladorHtmlIPad: ObservableObject {
    let web: WKWebView

    init() {
        let config = WKWebViewConfiguration()
        config.dataDetectorTypes = [.link]
        web = WKWebView(frame: .zero, configuration: config)
        web.isFindInteractionEnabled = true
        web.allowsBackForwardNavigationGestures = true
        web.isOpaque = false
        web.backgroundColor = .systemBackground
    }

    func cargar(_ url: URL) {
        web.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
    }

    func buscar() {
        web.findInteraction?.presentFindNavigator(showingReplace: false)
    }

    /// El modo noche: un filtro sobre la pagina entera (las imagenes vuelven
    /// a su color con el mismo filtro otra vez).
    func ponerNoche(_ si: Bool) {
        let js = si
            ? """
              (function(){var s=document.getElementById('pixpin-noche');if(!s){s=document.createElement('style');s.id='pixpin-noche';document.documentElement.appendChild(s);}
              s.textContent='html{filter:invert(1) hue-rotate(180deg);background:#fff}img,video,picture,canvas,svg image{filter:invert(1) hue-rotate(180deg)}';})();
              """
            : "(function(){var s=document.getElementById('pixpin-noche');if(s){s.remove();}})();"
        web.evaluateJavaScript(js, completionHandler: nil)
    }
}

private struct WebHtmlIPad: UIViewRepresentable {
    let controlador: ControladorHtmlIPad
    let noche: Bool

    func makeCoordinator() -> Delegado { Delegado(controlador: controlador, noche: noche) }

    func makeUIView(context: Context) -> WKWebView {
        controlador.web.navigationDelegate = context.coordinator
        return controlador.web
    }

    func updateUIView(_ v: WKWebView, context: Context) {
        if context.coordinator.noche != noche {
            context.coordinator.noche = noche
            controlador.ponerNoche(noche)
        }
    }

    @MainActor
    final class Delegado: NSObject, WKNavigationDelegate {
        weak var controlador: ControladorHtmlIPad?
        var noche: Bool

        init(controlador: ControladorHtmlIPad, noche: Bool) {
            self.controlador = controlador
            self.noche = noche
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            if noche { controlador?.ponerNoche(true) }
        }

        /// Los enlaces de fuera se abren en Safari; los del propio fichero, aqui.
        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction,
                     decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
            if let u = navigationAction.request.url, !u.isFileURL, navigationAction.navigationType == .linkActivated {
                UIApplication.shared.open(u)
                decisionHandler(.cancel)
                return
            }
            decisionHandler(.allow)
        }
    }
}

struct VisorHtmlIPad: View {
    let url: URL
    let titulo: String
    @StateObject private var c = ControladorHtmlIPad()
    @ObservedObject private var noche = ModoNoche.compartido

    var body: some View {
        ZStack(alignment: .top) {
            Color(uiColor: .systemBackground).ignoresSafeArea()
            WebHtmlIPad(controlador: c, noche: noche.activo)
                .ignoresSafeArea(edges: .bottom)
                .padding(.top, 58)
            BarraVisorIPad(tipo: "HTML", icono: "globe", titulo: titulo, alFijar: {
                if !GestorPinesIPad.compartido.pinear(url: url) { GestorPinesIPad.compartido.pinearLoQueSeVe() }
            }) {
                Button { c.buscar() } label: { Image(systemName: "magnifyingglass") }
                    .keyboardShortcut("f", modifiers: .command)
                    .help("Buscar (⌘F)")
                Button { c.web.reload() } label: { Image(systemName: "arrow.clockwise") }
                    .help("Volver a cargar")
                ShareLink(item: url) { Image(systemName: "square.and.arrow.up") }
                    .help("Compartir")
            }
        }
        .onAppear { c.cargar(url) }
    }
}
