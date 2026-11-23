// egui's triangles on the video card (ui/gpu.rs). Positions come in points, colours as
// premultiplied sRGB bytes; they are blended as they are (in gamma space), the way egui's
// own renderers and ui/soft.rs blend.
//
// The compiled shaders next to this file are what the program loads; after a change here
// make them again with the Windows SDK's compiler:
//   fxc /nologo /T vs_4_0 /E vs_main /O3 /Fo egui_vs.cso egui.hlsl
//   fxc /nologo /T ps_4_0 /E ps_main /O3 /Fo egui_ps.cso egui.hlsl

cbuffer Screen : register(b0) {
    float2 screen_points;
    float2 pad;
};

struct VsIn {
    float2 pos : POSITION;
    float2 uv : TEXCOORD;
    float4 color : COLOR;
};

struct VsOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD;
    float4 color : COLOR;
};

VsOut vs_main(VsIn i) {
    VsOut o;
    o.pos = float4(2.0 * i.pos.x / screen_points.x - 1.0, 1.0 - 2.0 * i.pos.y / screen_points.y, 0.0, 1.0);
    o.uv = i.uv;
    o.color = i.color;
    return o;
}

Texture2D tex : register(t0);
SamplerState smp : register(s0);

float4 ps_main(VsOut i) : SV_TARGET {
    return tex.Sample(smp, i.uv) * i.color;
}
