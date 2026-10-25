(function () {
  "use strict";
  var doc = document;
  doc.documentElement.className += " js";
  var $ = function (id) { return doc.getElementById(id); };
  var todos = function (sel, en) { return Array.prototype.slice.call((en || doc).querySelectorAll(sel)); };

  var maestro = doc.querySelector("[data-maestro]");
  var audio = $("audio");
  var video = $("video");
  // Con audio y video, el audio manda y el video lo sigue sin sonido.
  var esclavo = audio && video ? video : null;
  var duracionApuntada = (parseInt(doc.body.getAttribute("data-duracion"), 10) || 0) / 1000;

  function minuto(seg) {
    seg = Math.max(0, Math.floor(seg || 0));
    var h = Math.floor(seg / 3600), m = Math.floor(seg / 60) % 60, s = seg % 60;
    var ss = (s < 10 ? "0" : "") + s;
    return h > 0 ? h + ":" + (m < 10 ? "0" : "") + m + ":" + ss : m + ":" + ss;
  }
  function duracion() {
    var d = maestro && isFinite(maestro.duration) ? maestro.duration : 0;
    return d > 0 ? d : duracionApuntada;
  }

  // --- Lo que se resalta: las lineas, los marcadores, las capturas ---
  var lineas = todos(".linea[data-ms]");
  var capturas = todos("figure.captura");
  var marcadoresLado = todos(".lista-marcadores li");
  function tiempos(lista) { return lista.map(function (e) { return parseInt(e.getAttribute("data-ms"), 10) || 0; }); }
  var tLineas = tiempos(lineas), tCapturas = tiempos(capturas), tMarcadores = tiempos(marcadoresLado);
  function ultimoAntes(ts, ms) {
    var lo = 0, hi = ts.length - 1, r = -1;
    while (lo <= hi) { var m = (lo + hi) >> 1; if (ts[m] <= ms) { r = m; lo = m + 1; } else { hi = m - 1; } }
    return r;
  }
  var actual = { l: -1, c: -1, m: -1 };
  var miniaturas = [];
  var seguir = $("seguir");
  var tocadoEn = 0;
  ["wheel", "touchmove", "keydown"].forEach(function (ev) {
    window.addEventListener(ev, function (e) {
      if (ev !== "keydown" || ["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"].indexOf(e.key) >= 0) tocadoEn = Date.now();
    }, { passive: true });
  });
  function marcar(lista, clave, i, conScroll) {
    if (actual[clave] === i) return;
    if (actual[clave] >= 0 && lista[actual[clave]]) lista[actual[clave]].classList.remove("ahora");
    actual[clave] = i;
    if (i >= 0 && lista[i]) {
      lista[i].classList.add("ahora");
      if (conScroll && seguir && seguir.checked && Date.now() - tocadoEn > 4000 && maestro && !maestro.paused) {
        lista[i].scrollIntoView({ block: "center", behavior: "smooth" });
      }
    }
  }
  function resaltar(ms) {
    marcar(lineas, "l", ultimoAntes(tLineas, ms), true);
    var c = ultimoAntes(tCapturas, ms);
    marcar(capturas, "c", c, false);
    miniaturas.forEach(function (b, i) { b.classList.toggle("ahora", i === c); });
    marcar(marcadoresLado, "m", ultimoAntes(tMarcadores, ms), false);
  }

  // --- El reproductor ---
  var play = $("play"), ahora = $("ahora"), total = $("total"), hecho = $("hecho");
  var barra = $("linea-tiempo"), globo = $("globo"), velocidad = $("velocidad");
  function ir(seg, reproducir) {
    if (!maestro) return;
    seg = Math.max(0, Math.min(seg, duracion() || seg));
    try { maestro.currentTime = seg; } catch (e) { /* sin metadatos aun */ }
    if (esclavo) { try { esclavo.currentTime = seg; } catch (e) { /* idem */ } }
    pintar();
    if (reproducir && maestro.paused) { var p = maestro.play(); if (p && p.catch) p.catch(function () {}); }
  }
  function pintar() {
    if (!maestro) return;
    var t = maestro.currentTime || 0, d = duracion();
    if (ahora) ahora.textContent = minuto(t);
    if (total && d) total.textContent = minuto(d);
    if (hecho && d) hecho.style.width = Math.min(100, t / d * 100) + "%";
    if (barra) barra.setAttribute("aria-valuenow", Math.floor(t));
    resaltar(Math.round(t * 1000));
  }
  if (maestro) {
    var mandos = doc.querySelector(".mandos");
    if (mandos) mandos.hidden = false;
    if (maestro === video) video.removeAttribute("controls");
    var bucle = 0;
    var cadaFotograma = function () {
      pintar();
      if (esclavo && Math.abs(esclavo.currentTime - maestro.currentTime) > 0.35) {
        try { esclavo.currentTime = maestro.currentTime; } catch (e) { /* idem */ }
      }
      bucle = maestro.paused ? 0 : requestAnimationFrame(cadaFotograma);
    };
    maestro.addEventListener("play", function () {
      doc.body.classList.add("sonando");
      if (esclavo) { esclavo.currentTime = maestro.currentTime; var p = esclavo.play(); if (p && p.catch) p.catch(function () {}); }
      if (!bucle) bucle = requestAnimationFrame(cadaFotograma);
    });
    maestro.addEventListener("pause", function () {
      doc.body.classList.remove("sonando");
      if (esclavo) esclavo.pause();
      pintar();
    });
    maestro.addEventListener("ratechange", function () { if (esclavo) esclavo.playbackRate = maestro.playbackRate; });
    maestro.addEventListener("loadedmetadata", pintar);
    maestro.addEventListener("seeked", pintar);
    maestro.addEventListener("timeupdate", function () { if (maestro.paused) pintar(); });
    var alternar = function () {
      if (maestro.paused) { var p = maestro.play(); if (p && p.catch) p.catch(function () {}); } else { maestro.pause(); }
    };
    if (play) play.addEventListener("click", alternar);
    if (video) video.addEventListener("click", alternar);
    if (velocidad) {
      var pasos = [1, 1.25, 1.5, 1.75, 2, 0.75];
      velocidad.addEventListener("click", function () {
        var i = (pasos.indexOf(maestro.playbackRate) + 1) % pasos.length;
        maestro.playbackRate = pasos[i];
        velocidad.textContent = pasos[i] + "×";
      });
    }
    if (barra) {
      var segDe = function (x) {
        var r = barra.getBoundingClientRect();
        return Math.max(0, Math.min(1, (x - r.left) / r.width)) * duracion();
      };
      var arrastrando = false;
      barra.addEventListener("pointerdown", function (e) {
        var senal = e.target.closest(".senal");
        if (senal) return;
        arrastrando = true;
        barra.setPointerCapture(e.pointerId);
        ir(segDe(e.clientX), false);
      });
      barra.addEventListener("pointermove", function (e) {
        var r = barra.getBoundingClientRect();
        var senal = e.target.closest(".senal");
        if (globo) {
          globo.hidden = false;
          globo.style.left = (e.clientX - r.left) + "px";
          globo.textContent = senal ? senal.getAttribute("title") : minuto(segDe(e.clientX));
        }
        if (arrastrando) ir(segDe(e.clientX), false);
      });
      barra.addEventListener("pointerup", function () { arrastrando = false; });
      barra.addEventListener("pointerleave", function () { if (globo) globo.hidden = true; });
      barra.addEventListener("keydown", function (e) {
        if (e.key === "ArrowLeft") { ir(maestro.currentTime - 5); e.preventDefault(); }
        if (e.key === "ArrowRight") { ir(maestro.currentTime + 5); e.preventDefault(); }
      });
    }
    doc.addEventListener("keydown", function (e) {
      var en = e.target && e.target.tagName;
      if (en === "INPUT" || en === "TEXTAREA" || (lupa && lupa.open) || e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key === " " || e.key === "k") { alternar(); e.preventDefault(); }
      else if (e.key === "j") ir(maestro.currentTime - 10);
      else if (e.key === "l") ir(maestro.currentTime + 10);
    });
  }

  // Cualquier minuto que se pulsa salta alli y suena.
  doc.addEventListener("click", function (e) {
    var a = e.target.closest && e.target.closest("[data-ms]");
    if (!a || !maestro) return;
    if (a.classList.contains("t") || a.classList.contains("senal") || a.classList.contains("mini")) {
      e.preventDefault();
      ir((parseInt(a.getAttribute("data-ms"), 10) || 0) / 1000, true);
    }
  });

  // La barra pegada ocupa lo suyo: lo de al lado se pega debajo.
  var fija = $("barra-fija");
  function altoBarra() { if (fija) doc.documentElement.style.setProperty("--alto-barra", (fija.offsetHeight + 12) + "px"); }
  altoBarra();
  window.addEventListener("resize", altoBarra);
  if (fija) doc.documentElement.style.scrollPaddingTop = (fija.offsetHeight + 12) + "px";

  // --- Las miniaturas del lado: las mismas imagenes de la transcripcion ---
  var cajaMinis = $("miniaturas");
  if (cajaMinis) {
    capturas.forEach(function (f, i) {
      var img = f.querySelector("img");
      if (!img) return;
      var b = doc.createElement("button");
      b.type = "button";
      b.className = "mini";
      b.setAttribute("data-ms", f.getAttribute("data-ms"));
      b.title = img.alt;
      var m = doc.createElement("img");
      m.src = img.src; m.alt = ""; m.loading = "lazy";
      var s = doc.createElement("span");
      s.textContent = minuto((parseInt(f.getAttribute("data-ms"), 10) || 0) / 1000);
      b.appendChild(m); b.appendChild(s);
      b.addEventListener("click", function () {
        if (!maestro) abrirLupa(i);
        f.scrollIntoView({ block: "center", behavior: "smooth" });
      });
      cajaMinis.appendChild(b);
      miniaturas.push(b);
    });
  }

  // --- La lupa de las capturas ---
  var lupa = $("lupa"), lupaImg = $("lupa-img"), lupaPie = $("lupa-pie"), enLupa = -1;
  function abrirLupa(i) {
    if (!lupa || !capturas[i]) return;
    enLupa = i;
    var img = capturas[i].querySelector("img");
    lupaImg.src = img.src;
    lupaImg.classList.remove("cerca");
    lupaPie.textContent = img.alt;
    if (!lupa.open) { if (lupa.showModal) lupa.showModal(); else lupa.setAttribute("open", ""); }
  }
  function cerrarLupa() { if (lupa && lupa.open) { if (lupa.close) lupa.close(); else lupa.removeAttribute("open"); } }
  capturas.forEach(function (f, i) {
    var b = f.querySelector(".ampliar");
    if (b) b.addEventListener("click", function () { abrirLupa(i); });
  });
  if (lupa) {
    $("lupa-cerrar").addEventListener("click", cerrarLupa);
    $("lupa-antes").addEventListener("click", function () { abrirLupa((enLupa - 1 + capturas.length) % capturas.length); });
    $("lupa-despues").addEventListener("click", function () { abrirLupa((enLupa + 1) % capturas.length); });
    var irBoton = $("lupa-ir");
    if (irBoton) irBoton.addEventListener("click", function () {
      var f = capturas[enLupa];
      cerrarLupa();
      if (f) ir((parseInt(f.getAttribute("data-ms"), 10) || 0) / 1000, true);
    });
    lupaImg.addEventListener("click", function () { lupaImg.classList.toggle("cerca"); });
    lupa.addEventListener("click", function (e) { if (e.target === lupa) cerrarLupa(); });
    lupa.addEventListener("keydown", function (e) {
      if (e.key === "ArrowLeft") $("lupa-antes").click();
      if (e.key === "ArrowRight") $("lupa-despues").click();
    });
  }

  // --- Buscar en la transcripcion ---
  var buscar = $("buscar");
  var originales = new Map();
  function sinAcentos(s) { return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase(); }
  function resaltarEn(span, q) {
    var texto = originales.get(span);
    if (texto === undefined) { texto = span.textContent; originales.set(span, texto); }
    span.textContent = "";
    if (!q) { span.textContent = texto; return true; }
    var plano = sinAcentos(texto), desde = 0, hay = false, i;
    while ((i = plano.indexOf(q, desde)) >= 0) {
      hay = true;
      span.appendChild(doc.createTextNode(texto.slice(desde, i)));
      var m = doc.createElement("mark");
      m.textContent = texto.slice(i, i + q.length);
      span.appendChild(m);
      desde = i + q.length;
    }
    span.appendChild(doc.createTextNode(texto.slice(desde)));
    return hay;
  }
  if (buscar) {
    buscar.addEventListener("input", function () {
      var q = sinAcentos(buscar.value.trim());
      var primera = null;
      todos(".texto .linea, .texto .marca").forEach(function (p) {
        var span = p.querySelector(".dicho");
        var hay = span ? resaltarEn(span, q) : false;
        p.classList.toggle("oculta", !!q && !hay);
        if (q && hay && !primera) primera = p;
      });
      capturas.forEach(function (f) { f.classList.toggle("oculta", !!q); });
      if (primera) primera.scrollIntoView({ block: "center" });
    });
  }

  // Un enlace con minuto (#t-12000) abre la pagina en ese punto.
  var m = /^#t-(\d+)$/.exec(location.hash);
  if (m && maestro) ir(parseInt(m[1], 10) / 1000, false);
  pintar();
})();
