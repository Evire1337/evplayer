export const VERTEX = `
attribute vec2 aPos;
varying vec2 vUv;
void main() { vUv = aPos * 0.5 + 0.5; gl_Position = vec4(aPos, 0.0, 1.0); }
`;

// Value noise and fbm drive a slow domain warp. The cover comes in as a tiny texture, so
// it is already soft; two warped samples are blended for depth, then colour is enriched,
// vignetted and given a still dither against banding.
export const FRAGMENT = `
precision highp float;
varying vec2 vUv;
uniform sampler2D uFrom;
uniform sampler2D uTo;
uniform float uMix;
uniform float uTime;
uniform float uWarp;
uniform float uPulse;
uniform float uDim;
uniform vec2 uRes;

float hash(vec2 p) { return fract(sin(dot(p, vec2(41.3, 289.1))) * 45758.5453); }
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
             mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) {
  float v = 0.0, a = 0.5;
  for (int i = 0; i < 4; i++) { v += a * noise(p); p = p * 2.03 + vec2(1.7, 9.2); a *= 0.5; }
  return v;
}
vec2 rotate(vec2 p, float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c) * p; }

vec3 cover(sampler2D tex, vec2 uv, float t) {
  vec2 q = vec2(fbm(uv * 1.6 + t * 0.07), fbm(uv * 1.6 + vec2(5.2, 1.3) - t * 0.06));
  vec2 r = vec2(fbm(uv * 1.2 + q * 2.0 + t * 0.05), fbm(uv * 1.2 + q * 2.0 + vec2(8.3, 2.8) + t * 0.04));
  float amount = uWarp * (0.55 + uPulse * 0.35);
  vec2 a = rotate(uv - 0.5, t * 0.035) * (0.82 - uPulse * 0.04) + 0.5 + (r - 0.5) * amount;
  vec2 b = rotate(uv - 0.5, -t * 0.028 + 1.9) * 0.62 + 0.5 + (q - 0.5) * amount * 1.3;
  vec3 ca = texture2D(tex, clamp(a, 0.0, 1.0)).rgb;
  vec3 cb = texture2D(tex, clamp(b, 0.0, 1.0)).rgb;
  return mix(ca, cb, 0.45 + 0.25 * sin(t * 0.21 + r.x * 3.0));
}

void main() {
  vec2 uv = vUv;
  float aspect = uRes.x / uRes.y;
  uv.x = (uv.x - 0.5) * min(aspect, 1.8) / 1.8 + 0.5;
  vec3 col = mix(cover(uFrom, uv, uTime), cover(uTo, uv, uTime), uMix);
  float l = dot(col, vec3(0.299, 0.587, 0.114));
  col = mix(vec3(l), col, 1.28);
  col *= (0.78 + uPulse * 0.06) * uDim;
  float vig = smoothstep(1.15, 0.25, length(vUv - vec2(0.5, 0.45)));
  col *= mix(0.55, 1.0, vig);
  // a still, one-step dither against banding: invisible, unlike moving film grain
  col += (hash(gl_FragCoord.xy) - 0.5) / 255.0;
  gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
`;
