// The live background (src/ui/backdrop/shaders.ts, the same shader in HLSL): value noise and
// fbm drive a slow domain warp of the cover, which comes in as a tiny blurred texture; two
// warped samples are blended for depth, then the colour is enriched, vignetted and given a
// still dither against banding. Drawn at half the window's size into a texture of its own,
// which the interface then shows stretched (ui/gpu.rs).
//
// After a change, make the compiled shaders again with the Windows SDK's compiler:
//   fxc /nologo /T vs_4_0 /E vs_full /O3 /Fo backdrop_vs.cso backdrop.hlsl
//   fxc /nologo /T ps_4_0 /E ps_main /O3 /Fo backdrop_ps.cso backdrop.hlsl

cbuffer Params : register(b0) {
    float uMix;
    float uTime;
    float uWarp;
    float uPulse;
    float uDim;
    float2 uRes;
    float pad;
};

Texture2D uFrom : register(t0);
Texture2D uTo : register(t1);
SamplerState smp : register(s0);

struct VsOut {
    float4 pos : SV_POSITION;
    // the page's vUv: 0..1 with y going up
    float2 uv : TEXCOORD;
};

// one triangle that covers the whole target
VsOut vs_full(uint id : SV_VertexID) {
    VsOut o;
    float2 p = float2((id << 1) & 2, id & 2);
    o.pos = float4(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = p;
    return o;
}

float hash(float2 p) { return frac(sin(dot(p, float2(41.3, 289.1))) * 45758.5453); }
float noise(float2 p) {
    float2 i = floor(p), f = frac(p);
    float2 u = f * f * (3.0 - 2.0 * f);
    return lerp(lerp(hash(i), hash(i + float2(1.0, 0.0)), u.x),
                lerp(hash(i + float2(0.0, 1.0)), hash(i + float2(1.0, 1.0)), u.x), u.y);
}
float fbm(float2 p) {
    float v = 0.0, a = 0.5;
    [unroll] for (int i = 0; i < 4; i++) { v += a * noise(p); p = p * 2.03 + float2(1.7, 9.2); a *= 0.5; }
    return v;
}
// GLSL's mat2(c, -s, s, c) * p, column-major
float2 rotate(float2 p, float a) { float c = cos(a), s = sin(a); return float2(c * p.x + s * p.y, -s * p.x + c * p.y); }

float3 cover(Texture2D tex, float2 uv, float t) {
    float2 q = float2(fbm(uv * 1.6 + t * 0.07), fbm(uv * 1.6 + float2(5.2, 1.3) - t * 0.06));
    float2 r = float2(fbm(uv * 1.2 + q * 2.0 + t * 0.05), fbm(uv * 1.2 + q * 2.0 + float2(8.3, 2.8) + t * 0.04));
    float amount = uWarp * (0.55 + uPulse * 0.35);
    float2 a = rotate(uv - 0.5, t * 0.035) * (0.82 - uPulse * 0.04) + 0.5 + (r - 0.5) * amount;
    float2 b = rotate(uv - 0.5, -t * 0.028 + 1.9) * 0.62 + 0.5 + (q - 0.5) * amount * 1.3;
    // WebGL reads the texture's first row at v = 0 too, so the coordinates go in as they are
    float3 ca = tex.SampleLevel(smp, saturate(a), 0).rgb;
    float3 cb = tex.SampleLevel(smp, saturate(b), 0).rgb;
    return lerp(ca, cb, 0.45 + 0.25 * sin(t * 0.21 + r.x * 3.0));
}

float4 ps_main(VsOut i) : SV_TARGET {
    float2 vUv = i.uv;
    float2 uv = vUv;
    float aspect = uRes.x / uRes.y;
    uv.x = (uv.x - 0.5) * min(aspect, 1.8) / 1.8 + 0.5;
    float3 col = lerp(cover(uFrom, uv, uTime), cover(uTo, uv, uTime), uMix);
    float l = dot(col, float3(0.299, 0.587, 0.114));
    col = lerp(float3(l, l, l), col, 1.28);
    col *= (0.78 + uPulse * 0.06) * uDim;
    float vig = smoothstep(1.15, 0.25, length(vUv - float2(0.5, 0.45)));
    col *= lerp(0.55, 1.0, vig);
    // a still, one-step dither against banding: invisible, unlike moving film grain
    col += (hash(i.pos.xy) - 0.5) / 255.0;
    return float4(saturate(col), 1.0);
}
