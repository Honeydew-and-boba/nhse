/** Pixel rasterizers for the design editor's shape tools.
 *
 *  Coordinates are 32×32 design pixels; every result is a list of sheet-local
 *  indices (`y * 32 + x`) with out-of-canvas pixels dropped, so callers can
 *  paint them straight into a sheet. */

export type Point = [number, number];
export type Stamp = "circle" | "star" | "heart";

const W = 32;
const index = (x: number, y: number) => y * W + x;
const inBounds = (x: number, y: number) => x >= 0 && y >= 0 && x < W && y < W;

/** One-pixel line between two points (Bresenham). */
export function line(a: Point, b: Point): number[] {
  const out: number[] = [];
  let [x, y] = a;
  const dx = Math.abs(b[0] - x);
  const dy = -Math.abs(b[1] - y);
  const sx = x < b[0] ? 1 : -1;
  const sy = y < b[1] ? 1 : -1;
  let err = dx + dy;
  for (;;) {
    if (inBounds(x, y)) out.push(index(x, y));
    if (x === b[0] && y === b[1]) break;
    const e2 = 2 * err;
    if (e2 >= dy) { err += dy; x += sx; }
    if (e2 <= dx) { err += dx; y += sy; }
  }
  return out;
}

/** One-pixel rectangle outline with `a` and `b` as opposite corners. */
export function rect(a: Point, b: Point): number[] {
  const out = new Set<number>();
  const xa = Math.min(a[0], b[0]), xb = Math.max(a[0], b[0]);
  const ya = Math.min(a[1], b[1]), yb = Math.max(a[1], b[1]);
  for (let x = xa; x <= xb; x++) {
    for (const y of [ya, yb]) if (inBounds(x, y)) out.add(index(x, y));
  }
  for (let y = ya + 1; y < yb; y++) {
    for (const x of [xa, xb]) if (inBounds(x, y)) out.add(index(x, y));
  }
  return [...out];
}

/** One-pixel ellipse inscribed in the box with `a` and `b` as opposite corners;
 *  a zero-width or zero-height box degenerates to a line. */
export function ellipse(a: Point, b: Point): number[] {
  const xa = Math.min(a[0], b[0]), xb = Math.max(a[0], b[0]);
  const ya = Math.min(a[1], b[1]), yb = Math.max(a[1], b[1]);
  const cx = (xa + xb) / 2, cy = (ya + yb) / 2;
  const rx = (xb - xa) / 2, ry = (yb - ya) / 2;
  if (rx === 0 || ry === 0) return line(a, b);
  const out = new Set<number>();
  // Stepping both axes and taking square roots keeps steep and flat ovals closed.
  for (let x = xa; x <= xb; x++) {
    const t = 1 - ((x - cx) / rx) ** 2;
    if (t < 0) continue;
    const d = ry * Math.sqrt(t);
    for (const y of [Math.round(cy - d), Math.round(cy + d)]) if (inBounds(x, y)) out.add(index(x, y));
  }
  for (let y = ya; y <= yb; y++) {
    const t = 1 - ((y - cy) / ry) ** 2;
    if (t < 0) continue;
    const d = rx * Math.sqrt(t);
    for (const x of [Math.round(cx - d), Math.round(cx + d)]) if (inBounds(x, y)) out.add(index(x, y));
  }
  return [...out];
}

/** Rotate a 32×32 sheet by 90°; `cw` turns it clockwise on screen. */
export function rotate(sheet: number[], cw: boolean): number[] {
  const out: number[] = new Array(W * W);
  for (let y = 0; y < W; y++) {
    for (let x = 0; x < W; x++) {
      // Clockwise sends (x, y) to (31 − y, x).
      out[cw ? index(W - 1 - y, x) : index(y, W - 1 - x)] = sheet[index(x, y)];
    }
  }
  return out;
}

/** Shift a 32×32 sheet by whole pixels, wrapping whatever leaves an edge to the opposite one. */
export function shift(sheet: number[], dx: number, dy: number): number[] {
  const out: number[] = new Array(W * W);
  // Double modulo: JS keeps the sign of the dividend, so a large negative drag
  // would otherwise index before the array.
  const wrap = (v: number) => ((v % W) + W) % W;
  for (let y = 0; y < W; y++) {
    for (let x = 0; x < W; x++) {
      out[index(wrap(x + dx), wrap(y + dy))] = sheet[index(x, y)];
    }
  }
  return out;
}

/** Filled stamp of the given radius centred on `centre`. */
export function stamp(kind: Stamp, centre: Point, radius: number): number[] {
  if (kind === "circle") return disc(centre, radius);
  // Polygons are sampled at pixel centres, so their vertices are shifted half a
  // pixel to land on the same pixel the disc is centred on.
  const c: Point = [centre[0] + 0.5, centre[1] + 0.5];
  if (kind === "star") {
    const pts: Point[] = [];
    for (let i = 0; i < 10; i++) {
      const angle = -Math.PI / 2 + (i * Math.PI) / 5;
      const r = i % 2 ? radius * 0.45 : radius;
      pts.push([c[0] + Math.cos(angle) * r, c[1] + Math.sin(angle) * r]);
    }
    return fillPolygon(pts, centre, radius);
  }
  const pts: Point[] = [];
  const scale = radius * 1.15;
  for (let i = 0; i <= 40; i++) {
    const t = (i / 40) * Math.PI * 2;
    const x = 16 * Math.sin(t) ** 3;
    const y = 13 * Math.cos(t) - 5 * Math.cos(2 * t) - 2 * Math.cos(3 * t) - Math.cos(4 * t);
    pts.push([c[0] + (x / 17) * scale, c[1] - (y / 17) * scale]);
  }
  return fillPolygon(pts, centre, radius);
}

function disc([x0, y0]: Point, radius: number): number[] {
  const out: number[] = [];
  for (let y = y0 - radius; y <= y0 + radius; y++) {
    for (let x = x0 - radius; x <= x0 + radius; x++) {
      if (inBounds(x, y) && (x - x0) ** 2 + (y - y0) ** 2 <= radius * radius) out.push(index(x, y));
    }
  }
  return out;
}

/** Even-odd fill of a polygon, sampling pixel centres. */
function fillPolygon(pts: Point[], centre: Point, radius: number): number[] {
  const out: number[] = [];
  const [x0, y0] = centre;
  for (let y = Math.max(0, Math.floor(y0 - radius)); y <= Math.min(W - 1, Math.ceil(y0 + radius)); y++) {
    for (let x = Math.max(0, Math.floor(x0 - radius)); x <= Math.min(W - 1, Math.ceil(x0 + radius)); x++) {
      const px = x + 0.5, py = y + 0.5;
      let inside = false;
      for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
        const [xi, yi] = pts[i], [xj, yj] = pts[j];
        if ((yi > py) !== (yj > py) && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) inside = !inside;
      }
      if (inside) out.push(index(x, y));
    }
  }
  return out;
}
