import { FRAGMENT, VERTEX } from "./shaders";

const TEX_SIZE = 64;
const FADE_MS = 1400;
const RENDER_SCALE = 0.5;
/** the background drifts slowly: 30 frames a second look the same as 60 and cost half */
const FRAME_MS = 1000 / 30 - 2;

/** the default look before anything plays: deep sea green with a cool blue glow */
function fallbackSource(): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = c.height = TEX_SIZE;
  const g = c.getContext("2d")!;
  const grad = g.createLinearGradient(0, 0, TEX_SIZE, TEX_SIZE);
  grad.addColorStop(0, "#38d9a9");
  grad.addColorStop(0.5, "#1f6a8a");
  grad.addColorStop(1, "#082b28");
  g.fillStyle = grad;
  g.fillRect(0, 0, TEX_SIZE, TEX_SIZE);
  return c;
}

export class BackdropRenderer {
  private gl: WebGLRenderingContext;
  private program: WebGLProgram;
  private loc: Record<string, WebGLUniformLocation | null> = {};
  private from: WebGLTexture;
  private to: WebGLTexture;
  private mixStart = 0;
  private mix = 1;
  private time = 0;
  private last = 0;
  private raf = 0;
  private running = false;
  private currentUrl: string | null = null;
  pulse = 0;
  warp = 0.55;
  speed = 1;
  dim = 1;

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl", { antialias: false, powerPreference: "low-power" });
    if (!gl) throw new Error("WebGL is not available");
    this.gl = gl;
    this.program = this.link();
    const blank = this.texture(fallbackSource());
    this.from = blank;
    this.to = blank;
    document.addEventListener("visibilitychange", this.onVisibility);
  }

  private compile(type: number, src: string): WebGLShader {
    const gl = this.gl;
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
    return s;
  }

  private link(): WebGLProgram {
    const gl = this.gl;
    const p = gl.createProgram()!;
    gl.attachShader(p, this.compile(gl.VERTEX_SHADER, VERTEX));
    gl.attachShader(p, this.compile(gl.FRAGMENT_SHADER, FRAGMENT));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error("backdrop program did not link");
    gl.useProgram(p);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
    const a = gl.getAttribLocation(p, "aPos");
    gl.enableVertexAttribArray(a);
    gl.vertexAttribPointer(a, 2, gl.FLOAT, false, 0, 0);
    for (const name of ["uFrom", "uTo", "uMix", "uTime", "uWarp", "uPulse", "uDim", "uRes"]) {
      this.loc[name] = gl.getUniformLocation(p, name);
    }
    gl.uniform1i(this.loc.uFrom, 0);
    gl.uniform1i(this.loc.uTo, 1);
    return p;
  }

  private texture(source: TexImageSource & { width: number; height: number }): WebGLTexture {
    const small = document.createElement("canvas");
    small.width = small.height = TEX_SIZE;
    const g = small.getContext("2d")!;
    g.filter = "blur(2px)";
    // only the middle of the cover: many covers have a white or black frame, and its edge
    // pixels would otherwise be stretched along the sides of the window as bright lines
    const side = Math.min(source.width, source.height) * 0.7;
    g.drawImage(source as CanvasImageSource, (source.width - side) / 2, (source.height - side) / 2, side, side, 0, 0, TEX_SIZE, TEX_SIZE);
    const gl = this.gl;
    const tex = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, small);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    return tex;
  }

  private swapTo(source: TexImageSource & { width: number; height: number }): void {
    const next = this.texture(source);
    if (this.from !== this.to) this.gl.deleteTexture(this.from);
    this.from = this.to;
    this.to = next;
    this.mix = 0;
    this.mixStart = performance.now();
  }

  /** fade to a new cover; null goes back to the default colours */
  setImage(url: string | null): void {
    if (url === this.currentUrl) return;
    this.currentUrl = url;
    if (!url) {
      this.swapTo(fallbackSource());
      return;
    }
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      if (this.currentUrl === url) this.swapTo(img);
    };
    img.src = url;
  }

  private resize(): void {
    const w = Math.max(2, Math.round(this.canvas.clientWidth * RENDER_SCALE));
    const h = Math.max(2, Math.round(this.canvas.clientHeight * RENDER_SCALE));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
      this.gl.viewport(0, 0, w, h);
    }
  }

  private frame = (now: number): void => {
    if (!this.running) return;
    if (this.last && now - this.last < FRAME_MS) {
      this.raf = requestAnimationFrame(this.frame);
      return;
    }
    const dt = Math.min(100, now - (this.last || now));
    this.last = now;
    this.time += (dt / 1000) * this.speed;
    if (this.mix < 1) this.mix = Math.min(1, (now - this.mixStart) / FADE_MS);
    this.resize();
    const gl = this.gl;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.from);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.to);
    const eased = this.mix * this.mix * (3 - 2 * this.mix);
    gl.uniform1f(this.loc.uMix, eased);
    gl.uniform1f(this.loc.uTime, this.time);
    gl.uniform1f(this.loc.uWarp, this.warp);
    gl.uniform1f(this.loc.uPulse, this.pulse);
    gl.uniform1f(this.loc.uDim, this.dim);
    gl.uniform2f(this.loc.uRes, this.canvas.width, this.canvas.height);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    this.raf = requestAnimationFrame(this.frame);
  };

  start(): void {
    if (this.running || document.hidden) return;
    this.running = true;
    this.last = 0;
    this.raf = requestAnimationFrame(this.frame);
  }

  stop(): void {
    this.running = false;
    cancelAnimationFrame(this.raf);
  }

  /** draw one frame and stay still (animations off, or the cover changed while off) */
  still(): void {
    if (this.running) this.stop();
    this.running = true;
    this.raf = requestAnimationFrame((t) => {
      this.frame(t);
      this.stop();
    });
  }

  private onVisibility = () => {
    if (document.hidden) this.stop();
    else this.start();
  };

  destroy(): void {
    this.stop();
    document.removeEventListener("visibilitychange", this.onVisibility);
    const gl = this.gl;
    gl.deleteTexture(this.from);
    if (this.to !== this.from) gl.deleteTexture(this.to);
    gl.deleteProgram(this.program);
  }
}
