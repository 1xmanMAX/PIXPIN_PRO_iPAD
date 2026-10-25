// **El aviso al recibir algo suelto** («Enviar a…» desde otro aparato del
// grupo, Windows 17-oct): lo que llega sin escanear nada tiene que notarse.
//
// - Con la app delante, en la ventanita de avisos de la app (el globo de
//   Windows), con su sonido.
// - Con la app en el fondo (acabando una vuelta en el rato que da iOS, o el
//   despertar de `SincroDeFondo`), una notificacion local del sistema.
//
// Si otra zona pone `GanchosSincro.alLlegar`, manda esa; si no, este.

import Foundation
import UIKit

@MainActor
func avisarDeLlegada(titulo: String, texto: String) {
    let aviso = Aviso(id: "llegada-\(Int(Date().timeIntervalSince1970 * 1000))", categoria: .mini,
                      titulo: titulo, texto: texto)
    if UIApplication.shared.applicationState == .active {
        CentroAvisos.compartido.mostrarEnLaApp(aviso, sonar: true)
    } else {
        // `.mini` no sale como notificacion con la app delante (ya suena en
        // la app); en el fondo, si. Sin permiso, se ensena al volver.
        if !CentroAvisos.compartido.poner(aviso) {
            CentroAvisos.compartido.mostrarEnLaApp(aviso, sonar: false)
        }
    }
}
