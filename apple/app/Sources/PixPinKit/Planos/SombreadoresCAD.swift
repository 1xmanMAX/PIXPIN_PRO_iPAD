// Los sombreadores de los planos y del 3D: los HLSL de Windows
// (`pixpin-cad/src/gpu.rs` y `gpu3d.rs`) traducidos a MSL. Van como texto y
// se compilan al usarlos por primera vez (`MTLDevice.makeLibrary(source:)`),
// asi el paquete de Swift no necesita un paso de `metallib`; las tuberias
// se guardan (`MotorMetal`).
//
// Sin descriptores de vertices: cada sombreador lee sus buferes por
// `vertex_id` / `instance_id` con las estructuras de Rust (`packed_*` para
// que la alineacion sea la de 4 bytes de Rust y no la de 8 o 16 de MSL).
// En un dibujo con indices, `vertex_id` es el indice; en uno con
// instancias, `instance_id` ya lleva sumada la instancia de base.

enum SombreadoresCAD {
    /// El plano 2D: rellenos, sombreados con patron, rayas, arcos y letras.
    static let plano = """
    #include <metal_stdlib>
    using namespace metal;

    struct Vista {
        float2 escala;     // del plano a NDC
        float2 centro;     // el punto del grupo en el centro de la ventana
        float4 color7;     // el color 7 de AutoCAD sobre este fondo
        float  px;         // unidades del plano por pixel
        float  modo;       // 0 tal cual, 1 fondo oscuro, 2 fondo claro
        float2 relleno_;
    };

    struct Vertice { packed_float2 p; uint color; };
    struct VerticeTrama { packed_float2 p; uint color; uint trama; };
    struct Trama { packed_float2 centro; packed_float4 inv; float escala; uint desde; uint cuantas; };
    struct Familia { packed_float2 base; packed_float2 dir; float paso; float corr; float largo; uint n; float trazos[8]; };
    struct Letra { packed_float2 pos; packed_float4 m; uint color; uint glifo; };
    struct Arco { packed_float2 centro; float radio; float inicio; float barrido; uint color; uint r0; uint r1; };

    struct Sal { float4 pos [[position]]; float4 color; };

    float4 color_de(uint c, constant Vista& v) {
        float4 r = float4(float(c & 255u), float((c >> 8) & 255u), float((c >> 16) & 255u), float((c >> 24) & 255u)) / 255.0;
        if (r.a == 0.0) r = v.color7;
        return r;
    }

    // Los colores del plano se pensaron para un fondo; en el otro, los que
    // no se leen se aclaran (u oscurecen) sin perder su tono.
    float luz(float3 c) { return dot(c, float3(0.2126, 0.7152, 0.0722)); }
    float4 tinta(float4 c, constant Vista& v) {
        if (v.modo < 0.5) return c;
        float l = luz(c.rgb);
        float sat = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
        if (v.modo < 1.5) {
            if (sat < 0.15 && l < 0.5) c.rgb = 1.0 - c.rgb * 0.8;
            else if (l < 0.55) c.rgb = mix(c.rgb, float3(1.0), (0.55 - l) / (1.0 - l));
        } else if (l > 0.62) {
            c.rgb *= 0.5 / l;
        }
        return c;
    }
    // Los rellenos grandes, en oscuro, se apagan.
    float4 apagado(float4 c, constant Vista& v) {
        if (v.modo < 0.5 || v.modo > 1.5) return c;
        float sat = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
        if (sat < 0.15) c.rgb = 0.17 + (1.0 - c.rgb) * 0.22;
        else c.rgb *= 0.42;
        return c;
    }

    float4 a_pantalla(float2 p, constant Vista& v) { return float4((p - v.centro) * v.escala, 0.5, 1.0); }

    vertex Sal vs_simple(uint vid [[vertex_id]],
                         const device Vertice* vs [[buffer(0)]],
                         constant Vista& v [[buffer(1)]]) {
        Vertice x = vs[vid];
        Sal o;
        o.pos = a_pantalla(float2(x.p), v);
        o.color = color_de(x.color, v);
        return o;
    }

    fragment float4 ps_color(Sal i [[stage_in]], constant Vista& v [[buffer(0)]]) { return tinta(i.color, v); }
    fragment float4 ps_relleno(Sal i [[stage_in]], constant Vista& v [[buffer(0)]]) { return apagado(i.color, v); }

    // ---- sombreados con patron: sus rayas, pixel a pixel
    struct SalT { float4 pos [[position]]; float4 color; float2 mundo; uint trama [[flat]]; };

    vertex SalT vs_trama(uint vid [[vertex_id]],
                         const device VerticeTrama* vs [[buffer(0)]],
                         constant Vista& v [[buffer(1)]]) {
        VerticeTrama x = vs[vid];
        SalT o;
        o.pos = a_pantalla(float2(x.p), v);
        o.color = color_de(x.color, v);
        o.mundo = float2(x.p);
        o.trama = x.trama;
        return o;
    }

    fragment float4 ps_trama(SalT i [[stage_in]],
                             constant Vista& v [[buffer(0)]],
                             const device Trama* tramas [[buffer(1)]],
                             const device Familia* familias [[buffer(2)]]) {
        Trama tr = tramas[i.trama];
        float4 inv = float4(tr.inv);
        float2 d = i.mundo - float2(tr.centro);
        float2 q = float2(inv.x * d.x + inv.y * d.y, inv.z * d.x + inv.w * d.y);
        float pxl = v.px / max(tr.escala, 1e-30);
        float cob = 0.0;
        for (uint k = 0; k < tr.cuantas && k < 32u; k++) {
            Familia f = familias[tr.desde + k];
            float2 dir = float2(f.dir);
            float2 w = q - float2(f.base);
            float2 n = float2(-dir.y, dir.x);
            float s = dot(w, n);
            float paso = abs(f.paso);
            // Rayas a menos de 3 pixeles: un tono, no un muare.
            if (paso < pxl * 3.0) { cob = max(cob, 0.3); continue; }
            float kk = round(s / f.paso);
            float dist = abs(s - kk * f.paso) / pxl;
            float a = saturate(1.25 - dist);
            if (a <= 0.0) continue;
            if (f.n > 0u && f.largo > pxl) {
                float t = dot(w, dir) - kk * f.corr;
                float ph = t - floor(t / f.largo) * f.largo;
                float acc = 0.0;
                float dentro = 0.0;
                for (uint j = 0; j < f.n && j < 8u; j++) {
                    float L = f.trazos[j];
                    float largo = max(abs(L), pxl);
                    if (ph >= acc && ph < acc + largo) { dentro = L >= 0.0 ? 1.0 : 0.0; break; }
                    acc += abs(L);
                }
                a *= dentro;
            }
            cob = max(cob, a);
        }
        if (cob <= 0.004) discard_fragment();
        float4 t = tinta(i.color, v);
        return float4(t.rgb, t.a * cob);
    }

    // ---- letras: cada letra distinta esta una vez; las demas son instancias
    vertex Sal vs_letra(uint vid [[vertex_id]], uint iid [[instance_id]],
                        const device Letra* letras [[buffer(0)]],
                        constant Vista& v [[buffer(1)]],
                        const device packed_float2* malla [[buffer(2)]],
                        const device packed_uint2* glifos [[buffer(3)]]) {
        Letra l = letras[iid];
        uint2 gl = uint2(glifos[l.glifo]);
        uint n = gl.y & 0x7fffffffu;
        Sal o;
        o.color = color_de(l.color, v);
        if (vid >= n) { o.pos = float4(0.0, 0.0, -2.0, 1.0); return o; }
        float2 e = float2(malla[gl.x + vid]);
        float4 m = float4(l.m);
        float2 w = float2(l.pos) + float2(m.x * e.x + m.y * e.y, m.z * e.x + m.w * e.y);
        o.pos = a_pantalla(w, v);
        return o;
    }

    // ---- circulos y arcos, exactos a cualquier zoom: un cuadrado por arco
    struct SalA { float4 pos [[position]]; float4 color; float2 mundo; float4 arco [[flat]]; float barrido [[flat]]; };

    constant float2 ESQUINAS[6] = { float2(-1, -1), float2(1, -1), float2(-1, 1), float2(1, -1), float2(1, 1), float2(-1, 1) };

    vertex SalA vs_arco(uint vid [[vertex_id]], uint iid [[instance_id]],
                        const device Arco* arcos [[buffer(0)]],
                        constant Vista& v [[buffer(1)]]) {
        Arco a = arcos[iid];
        float2 c = float2(a.centro);
        float2 w = c + ESQUINAS[vid] * (a.radio + 2.0 * v.px);
        SalA o;
        o.pos = a_pantalla(w, v);
        o.color = color_de(a.color, v);
        o.mundo = w;
        o.arco = float4(c, a.radio, a.inicio);
        o.barrido = a.barrido;
        return o;
    }

    fragment float4 ps_arco(SalA i [[stage_in]], constant Vista& v [[buffer(0)]]) {
        float2 d = i.mundo - i.arco.xy;
        float dist = abs(length(d) - i.arco.z) / v.px;
        float a = saturate(1.1 - dist);
        if (a <= 0.003) discard_fragment();
        if (i.barrido < 6.2831) {
            float ang = atan2(d.y, d.x) - i.arco.w;
            ang = ang - floor(ang / 6.2831853) * 6.2831853;
            if (ang > i.barrido) discard_fragment();
        }
        float4 t = tinta(i.color, v);
        return float4(t.rgb, t.a * a);
    }
    """

    /// El modelo 3D: con profundidad, luz, aristas, caja de seccion y sus
    /// tapas, rayas, puntos y vidrios.
    static let modelo = """
    #include <metal_stdlib>
    using namespace metal;

    struct Vista3d {
        float4x4 mvp;      // los 16 numeros por filas de Rust: m * p da p * M
        float4 luz;        // hacia la luz (xyz)
        float4 ojo;        // la camara (xyz)
        float4 corte;      // x: 1 si hay caja de seccion, z: el radio de los puntos en pixeles
        float4 sec_min;    // la caja de seccion (xyz)
        float4 sec_max;
        uint elegido;      // elemento resaltado + 1 (0: ninguno)
        float claro;       // 1 fondo claro
        float2 pantalla;   // ancho y alto en pixeles
        uint aislado;      // el unico elemento que se ve + 1 (0: todos)
        float r0; float r1; float r2;
    };

    struct V3 { packed_float3 p; uint normal; uint color; uint elem; };

    struct S3 {
        float4 pos [[position]];
        float3 n;
        float4 c;
        float3 w;
        uint e [[flat]];
        uint f [[flat]];
    };

    float3 normal_de(uint n) {
        int3 v = int3(as_type<int>(n << 24) >> 24, as_type<int>(n << 16) >> 24, as_type<int>(n << 8) >> 24);
        return float3(v) / 127.0;
    }
    float4 color_de(uint c) {
        return float4(float(c & 255u), float((c >> 8) & 255u), float((c >> 16) & 255u), float((c >> 24) & 255u)) / 255.0;
    }

    S3 hacer(float3 p, uint n, uint c, uint e, constant Vista3d& v) {
        S3 o;
        o.pos = v.mvp * float4(p, 1.0);
        o.n = normal_de(n);
        o.c = color_de(c);
        o.w = p;
        o.e = e;
        o.f = (n >> 29) & 7u;
        return o;
    }

    vertex S3 vs_3d(uint vid [[vertex_id]],
                    const device V3* vs [[buffer(0)]],
                    constant Vista3d& v [[buffer(1)]]) {
        V3 x = vs[vid];
        return hacer(float3(x.p), x.normal, x.color, x.elem, v);
    }

    float3 de_contraste(S3 i, constant Vista3d& v) {
        if ((i.f & 2u) != 0u) return v.claro > 0.5 ? float3(0.0) : float3(1.0);
        return i.c.rgb;
    }

    // La caja de seccion: lo de fuera no se ve. Aislado: solo ese elemento.
    bool fuera(float3 w, uint e, constant Vista3d& v) {
        if (v.corte.x > 0.5 && (any(w < v.sec_min.xyz) || any(w > v.sec_max.xyz))) return true;
        if (v.aislado != 0u && e + 1u != v.aislado) return true;
        return false;
    }

    // Donde el rayo del ojo a `w` entra en la caja y por que cara.
    float entrada(float3 w, thread float3& cara, constant Vista3d& v) {
        float3 d = w - v.ojo.xyz;
        d = select(d, float3(1e-9), abs(d) < 1e-9);
        float3 t1 = (v.sec_min.xyz - v.ojo.xyz) / d;
        float3 t2 = (v.sec_max.xyz - v.ojo.xyz) / d;
        float3 tn = min(t1, t2);
        float t = max(tn.x, max(tn.y, tn.z));
        float3 eje = (tn.x >= t) ? float3(1, 0, 0) : ((tn.y >= t) ? float3(0, 1, 0) : float3(0, 0, 1));
        cara = -eje * sign(d);
        return t;
    }

    // La tapa del corte: la cara de dentro de un solido vista por el hueco
    // que abre una cara de la caja.
    bool es_tapa(S3 i, constant Vista3d& v) {
        if (v.corte.x < 0.5) return false;
        float3 g = cross(dfdx(i.w), dfdy(i.w));
        if (dot(g, v.ojo.xyz - i.w) < 0.0) g = -g;
        float3 cara;
        return dot(i.n, g) < 0.0 && entrada(i.w, cara, v) > 0.0;
    }

    float4 sombrear(S3 i, constant Vista3d& v) {
        float3 azul = float3(0.10, 0.55, 1.0);
        if ((i.f & 4u) != 0u) {
            float3 np = normalize(i.n);
            if (dot(np, normalize(v.ojo.xyz - i.w)) < 0.0) np = -np;
            float t = dot(np, v.luz.xyz) * 0.42;
            float3 cp = t > 0.0 ? mix(i.c.rgb, float3(1.0), t) : i.c.rgb * (1.0 + t);
            if (i.e + 1u == v.elegido) cp = mix(cp, azul, 0.55);
            return float4(cp, i.c.a);
        }
        if (i.f != 0u) {
            float3 plano = de_contraste(i, v);
            if (i.e + 1u == v.elegido) plano = mix(plano, azul, 0.55);
            return float4(plano, i.c.a);
        }
        float3 n = normalize(i.n);
        float3 vv = normalize(v.ojo.xyz - i.w);
        if (dot(n, vv) < 0.0) n = -n;
        float dif = saturate(dot(n, v.luz.xyz));
        float cielo = 0.5 + 0.5 * n.z;
        float3 h = normalize(v.luz.xyz + vv);
        float brillo = pow(saturate(dot(n, h)), 40.0) * 0.12;
        float3 col = i.c.rgb * (0.30 + 0.22 * cielo + 0.55 * dif) + brillo;
        if (i.e + 1u == v.elegido) col = mix(col, azul, 0.55);
        return float4(col, i.c.a);
    }

    fragment float4 ps_3d(S3 i [[stage_in]], constant Vista3d& v [[buffer(0)]]) {
        if (fuera(i.w, i.e, v) || es_tapa(i, v)) discard_fragment();
        return sombrear(i, v);
    }

    fragment float4 ps_vidrio(S3 i [[stage_in]], constant Vista3d& v [[buffer(0)]]) {
        if (fuera(i.w, i.e, v)) discard_fragment();
        return sombrear(i, v);
    }

    fragment float4 ps_tapa(S3 i [[stage_in]], constant Vista3d& v [[buffer(0)]]) {
        if (fuera(i.w, i.e, v) || !es_tapa(i, v)) discard_fragment();
        float3 cara;
        entrada(i.w, cara, v);
        float dif = saturate(dot(cara, v.luz.xyz));
        float3 col = i.c.rgb * (0.40 + 0.12 * (0.5 + 0.5 * cara.z) + 0.55 * dif) * 0.8;
        if (i.e + 1u == v.elegido) col = mix(col, float3(0.10, 0.55, 1.0), 0.55);
        return float4(col, 1.0);
    }

    // Un punto: 4 vertices en el mismo sitio; la esquina (byte alto de la
    // normal) lo abre en pantalla a un tamano fijo.
    vertex S3 vs_punto(uint vid [[vertex_id]],
                       const device V3* vs [[buffer(0)]],
                       constant Vista3d& v [[buffer(1)]]) {
        V3 x = vs[vid];
        S3 o = hacer(float3(x.p), 0u, x.color, x.elem, v);
        uint k = (x.normal >> 24) & 3u;
        float2 d = float2((k & 1u) != 0u ? 1.0 : -1.0, (k & 2u) != 0u ? 1.0 : -1.0);
        o.pos.xy += d * v.corte.z * 2.0 / v.pantalla * o.pos.w;
        o.n = float3(d, 0.0);
        return o;
    }

    fragment float4 ps_punto(S3 i [[stage_in]], constant Vista3d& v [[buffer(0)]]) {
        if (fuera(i.w, i.e, v)) discard_fragment();
        float r = length(i.n.xy);
        if (r > 1.0) discard_fragment();
        float3 col = i.c.rgb;
        if (i.e + 1u == v.elegido) col = float3(0.10, 0.55, 1.0);
        if (r > 0.62) col *= 0.35;
        return float4(col, 1.0);
    }

    fragment float4 ps_linea(S3 i [[stage_in]], constant Vista3d& v [[buffer(0)]]) {
        if (fuera(i.w, i.e, v)) discard_fragment();
        if (i.e + 1u == v.elegido) return float4(0.10, 0.55, 1.0, 1.0);
        return float4(de_contraste(i, v), i.c.a);
    }

    fragment float4 ps_arista(S3 i [[stage_in]], constant Vista3d& v [[buffer(0)]]) {
        if (fuera(i.w, i.e, v)) discard_fragment();
        float3 base = i.c.rgb * 0.28;
        if (i.e + 1u == v.elegido) base = float3(0.0, 0.30, 0.75);
        return float4(base, v.claro > 0.5 ? 0.75 : 0.85);
    }
    """

    /// Los puntos de entrada de cada fuente (lo que comprueban las pruebas).
    static let funcionesPlano = ["vs_simple", "ps_color", "ps_relleno", "vs_trama", "ps_trama", "vs_letra", "vs_arco", "ps_arco"]
    static let funcionesModelo = ["vs_3d", "vs_punto", "ps_3d", "ps_vidrio", "ps_tapa", "ps_punto", "ps_linea", "ps_arista"]
}
