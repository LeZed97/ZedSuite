/**
 * WebGL availability check for the decorative effects (animated backgrounds).
 *
 * Some environments hand out a WebGL context that cannot actually render:
 * virtual machines without GPU, remote desktop sessions, browsers with
 * hardware acceleration disabled. Three.js then throws while reading the
 * shader precision ("null is not an object (evaluating
 * getShaderPrecisionFormat(...).precision)") and the whole page unmounts.
 * The effects are optional, so they are skipped when the context is not
 * usable and the plain theme background stays.
 */

let cached: boolean | null = null;

export function isWebGLUsable(): boolean {
  if (cached !== null) return cached;
  if (typeof document === "undefined") return false;
  try {
    const canvas = document.createElement("canvas");
    const gl =
      (canvas.getContext("webgl2") as WebGL2RenderingContext | null) ||
      (canvas.getContext("webgl") as WebGLRenderingContext | null);
    if (!gl) {
      cached = false;
    } else {
      const vertex = gl.getShaderPrecisionFormat(gl.VERTEX_SHADER, gl.HIGH_FLOAT);
      const fragment = gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.HIGH_FLOAT);
      cached = !!vertex && !!fragment && !gl.isContextLost();
      const lose = gl.getExtension("WEBGL_lose_context");
      lose?.loseContext();
    }
  } catch {
    cached = false;
  }
  if (!cached) console.warn("WebGL is not usable here: animated backgrounds disabled");
  return cached;
}
