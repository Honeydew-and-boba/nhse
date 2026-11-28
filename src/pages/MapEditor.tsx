import { ArrowDown, ArrowUp, Brush, Building2, Check as CheckIcon, Eye, ClipboardPaste, Copy, Crosshair, Download, Droplets, Eraser, Grid3x3, Hand, Layers, Map as MapIcon, MousePointer2, Redo2, Replace, RotateCw, Scissors, Sparkles, SquareDashed, Trash2, Undo2, Upload, ZoomIn, ZoomOut } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { PasteListModal } from "../components/ItemGridEditor";
import { ItemEditor } from "../components/ItemEditor";
import { ItemPicker } from "../components/ItemPicker";
import { Badge, Button, Callout, Check, Field, Loading, MenuButton, NumberInput, Segmented, Select, Tabs } from "../components/ui";
import { enumOptions, prettify } from "../components/Widgets";
import { api, type Arrangement, type Building, type ClearKind, type DesignInfo, type DumpKind, type Item, type MapMeta, type Rect } from "../lib/api";
import { F, pickFile, saveFile } from "../lib/dialog";
import { Grid, NONE_ITEM, readItems, writeItems } from "../lib/grid";
import { EXT_ID, NONE_ID, asset, buildingDoors, buildingFootprint, buildingMovable, buildingRects, footprint, hex, itemColor, itemLabel, itemSprite, variantOf } from "../lib/items";
import { useGD, useStore } from "../lib/store";
import { MOD } from "../lib/util";

type Tool = "inspect" | "place" | "erase" | "rect" | "pan";
type Panel = "tile" | "bulk" | "buildings" | "island" | "dumps";
type Pick = { kind: "building"; index: number } | null;
type GroundBrush = { kind: "path"; family: string } | { kind: "design"; index: number } | { kind: "erase"; layer: "both" | "path" | "design" };
type TerrainBrush = { kind: "water" } | { kind: "water-erase" } | { kind: "cliff"; delta: number } | { kind: "shape"; model: number; angle: number };
type Snapshot = { layer: number; tiles: Item[] } | { ground: { terrain: Uint8Array; designs: number[] } };
interface View { s: number; x: number; y: number }

/** Default path families, with the colours the terrain renderer uses. */
const PATHS = [
  { id: "soil", label: "Dirt", color: "#c49662" },
  { id: "stone", label: "Stone", color: "#9e9ea0" },
  { id: "brick", label: "Brick", color: "#b2463c" },
  { id: "darksoil", label: "Dark dirt", color: "#785234" },
  { id: "pattern", label: "Arched tile", color: "#ece6d2" },
  { id: "sand", label: "Sand", color: "#e2c48c" },
  { id: "tile", label: "Tile", color: "#5e84b2" },
  { id: "wood", label: "Wood", color: "#aa784a" },
];
const MAP_W = 9 * 32, MAP_H = 8 * 32; // whole map in item tiles
const PLAZA_W = 24, PLAZA_H = 20; // 12×10 terrain tiles
const MIN_S = 1, MAX_S = 40;
const CLEAR_KINDS: [ClearKind, string][] = [
  ["weeds", "Weeds"], ["branches", "Tree branches"], ["stones", "Stones & rocks"], ["shells", "Shells"], ["flowers", "Flowers"], ["bushes", "Bushes"],
  ["trees", "Trees"], ["plants", "All plants"], ["fences", "Fences"], ["holes", "Holes"], ["field_objects", "Field objects"], ["placed_items", "Placed items"], ["everything", "Everything"],
];

const rgbCache = new Map<number, [number, number, number]>();

/** Four-way "drag me" badge: dark disc, white arrows. */
const drawMoveBadge = (ctx: CanvasRenderingContext2D, cx: number, cy: number, r: number) => {
  ctx.beginPath(); ctx.arc(cx, cy, r, 0, Math.PI * 2);
  ctx.fillStyle = "rgba(27,27,27,0.92)"; ctx.fill();
  ctx.strokeStyle = "#fff"; ctx.lineWidth = Math.max(1, r * 0.14); ctx.stroke();
  const a = r * 0.6, h = Math.max(1.5, r * 0.3);
  ctx.strokeStyle = "#fff"; ctx.lineWidth = Math.max(1, r * 0.2); ctx.lineCap = "round";
  ctx.beginPath();
  ctx.moveTo(cx - a, cy); ctx.lineTo(cx + a, cy);
  ctx.moveTo(cx, cy - a); ctx.lineTo(cx, cy + a);
  ctx.stroke();
  ctx.lineCap = "butt";
  ctx.fillStyle = "#fff";
  for (const [sx, sy] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
    ctx.beginPath();
    ctx.moveTo(cx + sx * a, cy + sy * a);
    ctx.lineTo(cx + sx * a - sx * h - sy * h * 0.8, cy + sy * a - sy * h + sx * h * 0.8);
    ctx.lineTo(cx + sx * a - sx * h + sy * h * 0.8, cy + sy * a - sy * h - sx * h * 0.8);
    ctx.closePath(); ctx.fill();
  }
};

/**
 * A measured entrance: a translucent plate marks the door zone (and the drag hit area), and a
 * one-terrain-tile door glyph with a drag badge sits in its middle.
 */
const drawDoorZone = (ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, s: number, hot: boolean, glyph = true) => {
  ctx.fillStyle = hot ? "rgba(255,214,64,0.30)" : "rgba(255,214,64,0.16)";
  ctx.fillRect(x, y, w, h);
  ctx.strokeStyle = hot ? "rgba(255,214,64,0.95)" : "rgba(255,214,64,0.55)";
  ctx.lineWidth = 1; ctx.setLineDash([4, 3]);
  ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
  ctx.setLineDash([]);
  if (!glyph) return;
  const d = Math.min(2 * s, w * 0.92, h * 0.92); // one terrain tile is 2 item tiles
  const cx = x + w / 2, cy = y + h / 2;
  if (d >= 20) {
    const gw = d * 0.6, gh = d * 0.86;
    ctx.fillStyle = "rgba(255,246,214,0.95)";
    ctx.fillRect(cx - gw / 2, cy - gh / 2, gw, gh);
    ctx.strokeStyle = "rgba(20,20,20,0.8)"; ctx.lineWidth = 1.2;
    ctx.strokeRect(cx - gw / 2 + 0.5, cy - gh / 2 + 0.5, gw - 1, gh - 1);
    drawMoveBadge(ctx, cx, cy, Math.max(4, d * 0.17));
  } else if (d >= 8) {
    drawMoveBadge(ctx, cx, cy, Math.max(3, d * 0.28));
  }
};

/** Fill colour per building category: houses, villagers, shops, civic, campsite, transport, special. */
const buildingColor = (kind: number): [number, number, number] => {
  if (kind <= 8) return [91, 140, 255];
  if (kind <= 18) return [43, 182, 163];
  if (kind === 19 || kind === 24) return [211, 61, 61];
  if (kind === 20 || kind === 21) return [156, 107, 255];
  if (kind === 25) return [255, 159, 67];
  if (kind === 22 || kind === 23 || kind === 26 || kind === 27) return [136, 153, 170];
  return [226, 112, 138];
};

export default function MapEditor() {
  const gd = useGD();
  const { save, touch, toast, confirm, settings, rev } = useStore();
  const L = save!.field;
  const [meta, setMeta] = useState<MapMeta | null>(null);
  const [buildings, setBuildings] = useState<Building[] | null>(null);
  const [terrain, setTerrain] = useState<HTMLImageElement | null>(null);
  const layers = useRef<Item[][]>([[], []]);
  const [, setVer] = useState(0); // bumps whenever layer contents change (forces a redraw)
  const [layer, setLayer] = useState(0);
  const [tool, setTool] = useState<Tool>("inspect");
  const [panel, setPanel] = useState<Panel>("tile");
  const [view, setView] = useState<View>({ s: 3, x: 20, y: 20 });
  const [hover, setHover] = useState<[number, number] | null>(null); // layer coords
  const [selTile, setSelTile] = useState<[number, number] | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [brush, setBrush] = useState<Item>({ ...NONE_ITEM });
  const [ground, setGround] = useState<GroundBrush | null>(null);
  const [tbrush, setTbrush] = useState<TerrainBrush | null>(null);
  const [terrainSize, setTerrainSize] = useState(1);
  const [sharp, setSharp] = useState(false);
  // ground paths keep their own corner style: the game stores the same bend as rounded or sharp
  const [roadSharp, setRoadSharp] = useState(false);
  const [patterns, setPatterns] = useState<DesignInfo[] | null>(null);
  const [stroke, setStroke] = useState<[number, number][]>([]);
  const strokeTiles = useRef(new Set<string>());
  const terrainTargets = useRef<[number, number][]>([]);
  const terrainSeen = useRef(new Set<string>());
  const groundBefore = useRef<{ designs: number[]; terrain: Promise<Uint8Array> } | null>(null);
  const [show, setShow] = useState({ other: true, grid: true, buildings: true, plaza: true, sprites: true, designs: true, contours: false });
  const [pick, setPick] = useState<Pick>(null);
  const [selBuilding, setSelBuilding] = useState<number | null>(null);
  const [floating, setFloating] = useState<[number, number][]>([]);
  const undo = useRef<Snapshot[]>([]);
  const redo = useRef<Snapshot[]>([]);
  const warnedHigh = useRef(false);
  const clip = useRef<{ w: number; h: number; tiles: Item[] } | null>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const layerCanvas = useRef<(HTMLCanvasElement | null)[]>([null, null]);
  const sprites = useRef(new Map<string, HTMLImageElement | null>());
  /** Thumbnails for placed ground designs, keyed by normal design index. */
  const designs = useRef(new Map<number, HTMLImageElement | null>());
  /** Cache-busts design URLs once per mount so an edited design is reloaded. */
  const designEpoch = useRef(Date.now()).current;
  const syncChain = useRef<Promise<unknown>>(Promise.resolve());
  const [loaded, setLoaded] = useState(false);
  // side panel state lives here so it survives the frequent re-renders caused by the canvas
  const [bs, setBs] = useState({
    kind: "weeds" as ClearKind, from: NONE_ID, to: NONE_ID, idOnly: true, src: "item" as "item" | "diy" | "list", spawnItem: { ...NONE_ITEM } as Item, count: 10,
    diy: { min: 0, max: 1000, alphabetical: true, count: 1 }, list: [] as Item[], dropped: true, arrange: "square" as Arrangement, overwrite: false, pasting: false,
  });
  const [showEmpty, setShowEmpty] = useState(false);
  const [acreSel, setAcreSel] = useState<[number, number]>([0, 0]);

  // ------------------------------------------------------------- loading
  const loadTerrain = useCallback(async () => {
    const png = await api.terrainPng(8);
    const url = URL.createObjectURL(new Blob([png as BlobPart], { type: "image/png" }));
    const img = new Image();
    img.onload = () => setTerrain(img);
    img.src = url;
  }, []);
  const loadLayer = useCallback(async (l: number) => { layers.current[l] = readItems(await api.fieldLayer(l)); layerCanvas.current[l] = null; }, []);
  useEffect(() => {
    (async () => {
      try {
        const [m, b] = await Promise.all([api.mapMeta(), api.buildings(), loadLayer(0), loadLayer(1), loadTerrain()]);
        setMeta(m); setBuildings(b); setVer((v) => v + 1); setLoaded(true);
        undo.current = []; redo.current = [];
        if (m.max_elevation > 3 && !warnedHigh.current) {
          warnedHigh.current = true;
          toast("warn", "Terrain above the game limit",
            `This save has tiles at elevation ${m.max_elevation}. The game supports 0–3; they are left unchanged and painting is limited to 0–3.`);
        }
      } catch (e) { toast("error", "Could not load the map", String(e)); }
    })();
  }, [rev]);

  // fit the map to the viewport on first load
  useEffect(() => {
    if (!loaded || !wrap.current) return;
    const r = wrap.current.getBoundingClientRect();
    const s = Math.max(MIN_S, Math.min((r.width - 40) / MAP_W, (r.height - 40) / MAP_H));
    setView({ s, x: (r.width - MAP_W * s) / 2, y: (r.height - MAP_H * s) / 2 });
  }, [loaded]);

  // non-empty normal designs for the ground palette; PRO designs cannot be placed on the ground
  // in game, so they are never offered
  useEffect(() => {
    if (!loaded) return;
    api.designs("normal").then((d) => setPatterns(d.filter((p) => !p.empty))).catch(() => setPatterns([]));
  }, [loaded, rev]);

  const grid = (l = layer) => new Grid(L.width, L.height, layers.current[l]);
  const rgb = (id: number): [number, number, number] => {
    let c = rgbCache.get(id);
    if (!c) { const h = itemColor(gd, id); const n = parseInt(h.slice(1), 16); c = [(n >> 16) & 255, (n >> 8) & 255, n & 255]; rgbCache.set(id, c); }
    return c;
  };
  const layerImage = (l: number) => {
    let c = layerCanvas.current[l];
    if (c) return c;
    c = document.createElement("canvas");
    c.width = L.width; c.height = L.height;
    const ctx = c.getContext("2d")!;
    const img = ctx.createImageData(L.width, L.height);
    const tiles = layers.current[l];
    for (let x = 0; x < L.width; x++) for (let y = 0; y < L.height; y++) {
      const it = tiles[x * L.height + y];
      if (!it || it.id === NONE_ID) continue;
      const [r, g, b] = rgb(it.id === EXT_ID ? it.free & 0xffff : it.id);
      const o = (y * L.width + x) * 4;
      img.data[o] = r; img.data[o + 1] = g; img.data[o + 2] = b; img.data[o + 3] = 235;
    }
    ctx.putImageData(img, 0, 0);
    layerCanvas.current[l] = c;
    return c;
  };
  const changed = (l: number) => { layerCanvas.current[l] = null; setVer((v) => v + 1); };
  /** Loads a normal design thumbnail once; null while it loads (or if it fails). */
  const designImage = (index: number) => {
    let img = designs.current.get(index);
    if (img === undefined) {
      const im = new Image();
      designs.current.set(index, null);
      im.onload = () => { designs.current.set(index, im); setVer((v) => v + 1); };
      im.src = asset(`design/normal/${index}?r=${designEpoch}`);
      img = null;
    }
    return img;
  };
  const sync = (l: number) => {
    const bytes = writeItems(layers.current[l]);
    syncChain.current = syncChain.current.then(() => api.setFieldLayer(l, bytes)).then(touch).catch((e) => toast("error", "Could not save map changes", String(e)));
    return syncChain.current;
  };
  const snapshot = (l = layer) => { undo.current.push({ layer: l, tiles: layers.current[l].slice() }); if (undo.current.length > 100) undo.current.shift(); redo.current = []; };
  const doUndo = async (from: typeof undo, to: typeof redo) => {
    const s = from.current.pop();
    if (!s) return;
    if ("ground" in s) {
      to.current.push({ ground: { terrain: await api.terrain(), designs: meta?.design_tiles.slice() ?? [] } });
      try {
        await api.setTerrain(s.ground.terrain);
        await api.setDesignTiles(s.ground.designs);
        setMeta(await api.mapMeta());
        await loadTerrain();
        touch();
      } catch (e) { toast("error", "Could not undo the ground change", String(e)); }
      return;
    }
    to.current.push({ layer: s.layer, tiles: layers.current[s.layer].slice() });
    layers.current[s.layer] = s.tiles;
    changed(s.layer); sync(s.layer);
  };
  /** Runs a backend bulk operation and reloads the layer. */
  const bulk = async (label: string, l: number, op: () => Promise<number>) => {
    await syncChain.current;
    snapshot(l);
    try {
      const n = await op();
      await loadLayer(l); changed(l); touch();
      toast(n ? "ok" : "info", n ? `${label}: ${n} tile${n === 1 ? "" : "s"} changed` : `${label}: nothing to change`);
    } catch (e) { undo.current.pop(); toast("error", label + " failed", String(e)); }
  };

  // ------------------------------------------------------------- coordinates
  const toTile = (cx: number, cy: number): [number, number] => {
    const mx = Math.floor((cx - view.x) / view.s), my = Math.floor((cy - view.y) / view.s);
    return [mx - L.offset_x, my - L.offset_y];
  };
  const inLayer = (x: number, y: number) => x >= 0 && y >= 0 && x < L.width && y < L.height;
  /**
   * Terrain tile that contains a layer tile, or null outside the terrain layer. Terrain is
   * rendered over the whole map but its data covers the middle 7×6 acres (16 tiles in).
   */
  const terrainTile = (x: number, y: number): [number, number] | null => {
    if (!meta) return null;
    const tx = Math.floor((x + L.offset_x) / 2) - 16;
    const ty = Math.floor((y + L.offset_y) / 2) - 16;
    const [tw, th] = meta.terrain_dims;
    return tx >= 0 && ty >= 0 && tx < tw && ty < th ? [tx, ty] : null;
  };
  /** Top-left layer tile of a terrain tile (the inverse of `terrainTile`). */
  const terrainLayer = ([tx, ty]: [number, number]): [number, number] =>
    [(tx + 16) * 2 - L.offset_x, (ty + 16) * 2 - L.offset_y];
  /**
   * A path or terrain brush writes only where the acre template owns the tile (collision type 7);
   * beach, sea, rocks and river mouths are fixed and the game ignores terrain edits there.
   */
  const terrainEditable = ([tx, ty]: [number, number]) =>
    !!meta && meta.editable[tx * meta.terrain_dims[1] + ty];

  // ------------------------------------------------------------- drawing
  useEffect(() => {
    const c = canvas.current, w = wrap.current;
    if (!c || !w || !loaded) return;
    let frame = 0;
    const draw = () => {
      const dpr = window.devicePixelRatio || 1;
      const { width: cw, height: ch } = w.getBoundingClientRect();
      if (c.width !== Math.round(cw * dpr) || c.height !== Math.round(ch * dpr)) { c.width = Math.round(cw * dpr); c.height = Math.round(ch * dpr); c.style.width = cw + "px"; c.style.height = ch + "px"; }
      const ctx = c.getContext("2d")!;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.imageSmoothingEnabled = false;
      ctx.fillStyle = getComputedStyle(document.body).getPropertyValue("--bg") || "#111";
      ctx.fillRect(0, 0, cw, ch);
      const { s, x: px, y: py } = view;
      const ox = px + L.offset_x * s, oy = py + L.offset_y * s;
      if (terrain) ctx.drawImage(terrain, 0, 0, terrain.width, terrain.height, px, py, MAP_W * s, MAP_H * s);
      // Height contours: a line wherever two terrain tiles differ in elevation. Terrain data
      // starts one acre (16 terrain tiles, 32 item tiles) into the map image.
      if (show.contours && meta) {
        const [tw, th] = meta.terrain_dims;
        const d2 = 2 * s;
        if (d2 >= 5) {
          const bx = px + 16 * d2, by = py + 16 * d2;
          ctx.strokeStyle = "rgba(20,38,20,0.6)";
          ctx.lineWidth = Math.max(1, s * 0.35);
          ctx.beginPath();
          for (let tx = 0; tx < tw; tx++) for (let ty = 0; ty < th; ty++) {
            const gx = bx + tx * d2, gy = by + ty * d2;
            if (gx + d2 < 0 || gx > cw || gy + d2 < 0 || gy > ch) continue;
            const e = meta.elevations[tx * th + ty];
            if (tx + 1 < tw && meta.elevations[(tx + 1) * th + ty] !== e) {
              ctx.moveTo(gx + d2 + 0.5, gy); ctx.lineTo(gx + d2 + 0.5, gy + d2);
            }
            if (ty + 1 < th && meta.elevations[tx * th + ty + 1] !== e) {
              ctx.moveTo(gx, gy + d2 + 0.5); ctx.lineTo(gx + d2, gy + d2 + 0.5);
            }
          }
          ctx.stroke();
        }
      }
      if (show.plaza && meta) {
        ctx.fillStyle = "rgba(150,155,165,0.35)";
        ctx.fillRect(px + meta.plaza.x * s, py + meta.plaza.z * s, PLAZA_W * s, PLAZA_H * s);
      }
      // placed custom designs sit on the ground under every item; each design tile is one
      // terrain tile (2×2 items) and is aligned to the field layer, so layer tile / 2.
      if (show.designs && meta) {
        const [dw, dh] = meta.design_dims;
        const d2 = 2 * s;
        const gx0 = Math.max(0, Math.floor(-ox / d2)), gy0 = Math.max(0, Math.floor(-oy / d2));
        const gx1 = Math.min(dw, Math.ceil((cw - ox) / d2)), gy1 = Math.min(dh, Math.ceil((ch - oy) / d2));
        for (let x = gx0; x < gx1; x++) for (let y = gy0; y < gy1; y++) {
          const v = meta.design_tiles[x * dh + y];
          if (v === 0xf800) continue;
          const dx = ox + x * d2, dy = oy + y * d2;
          // Normal slots are proven (0..pattern_count-1); the game never puts a PRO design on the
          // ground, so anything else is an unknown value and keeps a plain marker. See docs/map-editor.md.
          const known = v < save!.pattern_count;
          const img = known ? designImage(v) : null;
          if (img) { ctx.drawImage(img, dx, dy, d2, d2); continue; }
          ctx.fillStyle = known ? "rgba(255,61,161,0.12)" : "rgba(255,61,161,0.28)";
          ctx.fillRect(dx, dy, d2, d2);
          ctx.strokeStyle = "rgba(255,61,161,0.75)"; ctx.lineWidth = 1;
          ctx.strokeRect(dx + 0.5, dy + 0.5, d2 - 1, d2 - 1);
        }
      }
      // ground-painting preview: paths and designs sit under the item layers
      if (stroke.length) {
        const d2 = 2 * s;
        for (const [lx, ly] of stroke) {
          if (tbrush) {
            ctx.globalAlpha = 0.55;
            ctx.fillStyle = tbrush.kind === "water" ? "#46bedc" : tbrush.kind === "water-erase" ? "#ff4d6d"
              : tbrush.kind === "cliff" ? "#b48c5a" : "#ffd23f";
            ctx.fillRect(ox + lx * s, oy + ly * s, d2, d2);
            ctx.globalAlpha = 1;
          } else if (ground?.kind === "design") {
            const dx = ox + (lx >> 1) * d2, dy = oy + (ly >> 1) * d2;
            const img = designImage(ground.index);
            if (img) { ctx.globalAlpha = 0.85; ctx.drawImage(img, dx, dy, d2, d2); ctx.globalAlpha = 1; }
            else { ctx.fillStyle = "rgba(255,61,161,0.3)"; ctx.fillRect(dx, dy, d2, d2); }
          } else {
            const t = terrainTile(lx, ly);
            if (!t) continue;
            ctx.globalAlpha = 0.85;
            ctx.fillStyle = ground?.kind === "path" ? PATHS.find((p) => p.id === ground.family)?.color ?? "#fff" : "rgba(255,77,109,0.55)";
            // terrain data tile 0 is drawn at image tile 16 (one acre of sea border)
            ctx.fillRect(px + (t[0] + 16) * d2, py + (t[1] + 16) * d2, d2, d2);
            ctx.globalAlpha = 1;
          }
        }
      }
      if (show.other) { ctx.globalAlpha = 0.3; ctx.drawImage(layerImage(1 - layer), ox, oy, L.width * s, L.height * s); }
      ctx.globalAlpha = 0.92;
      ctx.drawImage(layerImage(layer), ox, oy, L.width * s, L.height * s);
      ctx.globalAlpha = 1;

      // visible range in layer coords
      const x0 = Math.max(0, Math.floor(-ox / s)), y0 = Math.max(0, Math.floor(-oy / s));
      const x1 = Math.min(L.width, Math.ceil((cw - ox) / s)), y1 = Math.min(L.height, Math.ceil((ch - oy) / s));
      const tiles = layers.current[layer];
      if (show.sprites && s >= 12 && (x1 - x0) * (y1 - y0) < 12000) {
        for (let x = Math.max(0, x0 - 4); x < x1; x++) for (let y = Math.max(0, y0 - 4); y < y1; y++) {
          const it = tiles[x * L.height + y];
          if (!it || it.id === NONE_ID || it.id === EXT_ID) continue;
          const [fw, fh] = footprint(gd, it);
          const key = `${it.id}:${variantOf(it)}`;
          let img = sprites.current.get(key);
          if (img === undefined) {
            const im = new Image();
            sprites.current.set(key, null);
            im.onload = () => { sprites.current.set(key, im); setVer((v) => v + 1); };
            im.src = itemSprite(it.id, variantOf(it));
            img = null;
          }
          ctx.strokeStyle = "rgba(0,0,0,0.35)"; ctx.lineWidth = 1;
          ctx.strokeRect(ox + x * s + 0.5, oy + y * s + 0.5, fw * s - 1, fh * s - 1);
          if (img) {
            const d = Math.min(fw, fh) * s * 0.9;
            ctx.imageSmoothingEnabled = true;
            ctx.drawImage(img, ox + x * s + (fw * s - d) / 2, oy + y * s + (fh * s - d) / 2, d, d);
            ctx.imageSmoothingEnabled = false;
          }
        }
      }
      // grid lines
      if (show.grid) {
        ctx.lineWidth = 1;
        if (s >= 8) {
          ctx.strokeStyle = "rgba(255,255,255,0.10)";
          ctx.beginPath();
          for (let x = x0; x <= x1; x++) { ctx.moveTo(Math.round(ox + x * s) + 0.5, oy + y0 * s); ctx.lineTo(Math.round(ox + x * s) + 0.5, oy + y1 * s); }
          for (let y = y0; y <= y1; y++) { ctx.moveTo(ox + x0 * s, Math.round(oy + y * s) + 0.5); ctx.lineTo(ox + x1 * s, Math.round(oy + y * s) + 0.5); }
          ctx.stroke();
        }
        ctx.strokeStyle = "rgba(255,255,255,0.45)";
        ctx.setLineDash([4, 4]);
        ctx.beginPath();
        for (let a = 0; a <= 9; a++) { const X = Math.round(px + a * 32 * s) + 0.5; ctx.moveTo(X, py); ctx.lineTo(X, py + MAP_H * s); }
        for (let a = 0; a <= 8; a++) { const Y = Math.round(py + a * 32 * s) + 0.5; ctx.moveTo(px, Y); ctx.lineTo(px + MAP_W * s, Y); }
        ctx.stroke();
        ctx.setLineDash([]);
        if (s >= 2.5) {
          ctx.fillStyle = "rgba(255,255,255,0.75)";
          ctx.font = `600 ${Math.min(14, 4 + s * 1.5)}px Inter, system-ui`;
          for (let ay = 0; ay < 8; ay++) for (let ax = 0; ax < 9; ax++) ctx.fillText(String.fromCharCode(65 + ax) + (ay + 1), px + ax * 32 * s + 4, py + ay * 32 * s + Math.min(14, 4 + s * 1.5) + 2);
        }
        ctx.strokeStyle = "rgba(255,255,255,0.9)"; ctx.lineWidth = 1.5;
        ctx.strokeRect(ox, oy, L.width * s, L.height * s);
      }
      // floating items
      if (floating.length && layer === 1) {
        ctx.strokeStyle = "#ff4d6d"; ctx.lineWidth = 2;
        for (const [x, y] of floating) ctx.strokeRect(ox + x * s, oy + y * s, s, s);
      }
      // buildings: x/y is the centre of a plot measured in terrain tiles (16 per acre),
      // so the footprint spans x ± width and y ± height in item tiles (32 per acre).
      if (show.buildings && buildings) {
        ctx.font = `600 ${Math.max(10, Math.min(13, s * 2))}px Inter, system-ui`;
        ctx.textAlign = "center";
        const hoverIdx = hover ? doorAt(hover[0] + L.offset_x, hover[1] + L.offset_y) : -1;
        buildings.forEach((b, i) => {
          if (!b.kind) return;
          const sel = i === selBuilding;
          const [r, g, bl] = buildingColor(b.kind);
          const edge = sel ? "#fff" : `rgba(${r},${g},${bl},0.9)`;
          const rects = buildingRects(gd, b.kind);
          for (const [dx, dy, dw, dh] of rects) {
            const x = px + (b.x + dx) * s, y = py + (b.y + dy) * s;
            const w = dw * s, h = dh * s;
            ctx.fillStyle = `rgba(${r},${g},${bl},${sel ? 0.5 : 0.26})`;
            ctx.fillRect(x, y, w, h);
            ctx.strokeStyle = edge; ctx.lineWidth = 1.5;
            ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
            // corner ticks make the edges readable at a glance
            if (Math.min(w, h) >= 16) {
              const c = Math.min(10, w / 4, h / 4);
              ctx.strokeStyle = sel ? "#fff" : "rgba(255,255,255,0.9)"; ctx.lineWidth = 2;
              ctx.beginPath();
              ctx.moveTo(x, y + c); ctx.lineTo(x, y); ctx.lineTo(x + c, y);
              ctx.moveTo(x + w - c, y); ctx.lineTo(x + w, y); ctx.lineTo(x + w, y + c);
              ctx.moveTo(x + w, y + h - c); ctx.lineTo(x + w, y + h); ctx.lineTo(x + w - c, y + h);
              ctx.moveTo(x + c, y + h); ctx.lineTo(x, y + h); ctx.lineTo(x, y + h - c);
              ctx.stroke();
            }
          }
          // entrances: a translucent plate marks the door zone, the glyph is the drag handle
          const doors = buildingDoors(gd, b.kind);
          doors.forEach(([dx, dy, dw, dh], di) => {
            drawDoorZone(ctx, px + (b.x + dx) * s, py + (b.y + dy) * s, dw * s, dh * s, s, sel || hoverIdx === i, di === 0);
          });
          if (!doors.length) {
            // no measured door: a neutral centre handle for movable kinds (fixed ones select by body)
            if (buildingMovable(gd, b.kind)) drawMoveBadge(ctx, px + b.x * s, py + b.y * s, Math.max(3, Math.min(6, s * 0.7)));
          }
          if (s >= 2) {
            const [bdx, , bdw] = rects[0];
            const bottom = Math.max(...rects.map((d) => d[1] + d[3]), ...doors.map((d) => d[1] + d[3]));
            const label = buildingName(b.kind);
            ctx.fillStyle = "rgba(0,0,0,0.55)";
            const tw = ctx.measureText(label).width + 8;
            const lx = px + (b.x + bdx + bdw / 2) * s, ly = py + (b.y + bottom) * s;
            ctx.fillRect(lx - tw / 2, ly + 2, tw, 16);
            ctx.fillStyle = "#fff"; ctx.fillText(label, lx, ly + 14);
          }
        });
        ctx.textAlign = "start";
      }
      // selection rectangle
      if (rect) {
        ctx.fillStyle = "rgba(80,160,255,0.15)"; ctx.strokeStyle = "#4da3ff"; ctx.lineWidth = 1.5; ctx.setLineDash([6, 3]);
        ctx.fillRect(ox + rect.x * s, oy + rect.y * s, rect.w * s, rect.h * s);
        ctx.strokeRect(ox + rect.x * s + 0.5, oy + rect.y * s + 0.5, rect.w * s, rect.h * s);
        ctx.setLineDash([]);
      }
      // selected tile
      if (selTile) {
        const it = tiles[selTile[0] * L.height + selTile[1]];
        const [fw, fh] = it && it.id !== NONE_ID ? footprint(gd, it) : [1, 1];
        ctx.strokeStyle = "#ffd23f"; ctx.lineWidth = 2;
        ctx.strokeRect(ox + selTile[0] * s - 1, oy + selTile[1] * s - 1, fw * s + 2, fh * s + 2);
      }
      // hover
      if (hover && inLayer(hover[0], hover[1])) {
        let [fw, fh] = [1, 1];
        let [hx, hy] = hover;
        let ok = true;
        if (tool === "place" && tbrush) {
          const n = tbrush.kind === "shape" ? 1 : terrainSize;
          const r = (n - 1) >> 1;
          [fw, fh] = [2 * n, 2 * n];
          hx = (hover[0] & ~1) - r * 2; hy = (hover[1] & ~1) - r * 2;
          if (!terrainTile(hx, hy) || !terrainTile(hx + 2 * (n - 1), hy + 2 * (n - 1))) ok = false;
        } else if (tool === "place" && ground) {
          // Ground brushes work in one-terrain-tile (2×2 item tile) blocks. Their writes skip
          // water and fixed template ground, so the hover does not mark those tiles red; only the
          // terrain brush keeps the old red for tiles outside the terrain data.
          [fw, fh] = [2, 2];
          hx = hover[0] & ~1; hy = hover[1] & ~1;
        } else if (tool === "place" && brush.id !== NONE_ID) {
          [fw, fh] = footprint(gd, brush);
          ok = !settings?.map_prevent_overlap || grid().check(gd, brush, hover[0], hover[1]) === "ok";
          if (hover[0] + fw > L.width || hover[1] + fh > L.height) ok = false;
        } else if (tool === "rect" && clip.current && pasteMode.current) {
          [fw, fh] = [clip.current.w, clip.current.h];
        }
        ctx.strokeStyle = ok ? "rgba(255,255,255,0.95)" : "#ff4d6d"; ctx.lineWidth = 1.5;
        ctx.fillStyle = ok ? "rgba(255,255,255,0.18)" : "rgba(255,77,109,0.25)";
        ctx.fillRect(ox + hx * s, oy + hy * s, fw * s, fh * s);
        ctx.strokeRect(ox + hx * s + 0.5, oy + hy * s + 0.5, fw * s - 1, fh * s - 1);
      }
    };
    frame = requestAnimationFrame(draw);
    const ro = new ResizeObserver(() => { cancelAnimationFrame(frame); frame = requestAnimationFrame(draw); });
    ro.observe(w);
    return () => { cancelAnimationFrame(frame); ro.disconnect(); };
  });

  const buildingName = (k: number) => prettify((gd.enums.BuildingType ?? []).find(([, v]) => v === k)?.[0] ?? `Type ${k}`);

  // ------------------------------------------------------------- interaction
  const drag = useRef<{ mode: "pan" | "paint" | "erase" | "rect" | "ground" | "terrain"; sx: number; sy: number; vx: number; vy: number; tx: number; ty: number; dirty: boolean; last: string } | null>(null);
  const bdrag = useRef<{ index: number; startX: number; startY: number; mx: number; my: number; next: Building[] | null } | null>(null);
  const pasteMode = useRef(false);
  const space = useRef(false);

  /** Index of the building whose door or move handle is at this absolute item position, or -1. */
  const doorAt = (mx: number, my: number) =>
    buildings?.findIndex((b) => {
      if (!b.kind) return false;
      const rects = buildingDoors(gd, b.kind);
      if (!rects.length) return mx >= b.x - 1 && mx <= b.x && my >= b.y - 1 && my <= b.y;
      return rects.some(([dx, dy, dw, dh]) =>
        mx >= b.x + dx && mx < b.x + dx + dw && my >= b.y + dy && my < b.y + dy + dh);
    }) ?? -1;

  const paintAt = (x: number, y: number, single: boolean) => {
    const g = grid();
    if (brush.id === NONE_ID) { if (single) toast("info", "Choose an item to place first", "Pick it in the side panel, or select a tile and use “Use as brush”."); return false; }
    const [fw, fh] = footprint(gd, brush);
    if (!inLayer(x, y) || x + fw > L.width || y + fh > L.height) return false;
    if (settings?.map_prevent_overlap) {
      if (g.check(gd, brush, x, y) !== "ok") { if (single) toast("warn", "That spot is occupied", "Turn off “prevent overlapping items” in Settings to overwrite."); return false; }
    } else {
      for (let ix = 0; ix < fw; ix++) for (let iy = 0; iy < fh; iy++) g.removeAt(gd, x + ix, y + iy);
    }
    g.place(gd, { ...brush }, x, y, settings?.map_auto_extensions ?? true);
    return true;
  };
  const eraseAt = (x: number, y: number) => inLayer(x, y) && !!grid().removeAt(gd, x, y);

  const addGroundTile = (x: number, y: number) => {
    // MyDesignMap is 144×96 terrain tiles, the same 9×6 block as the field layer, so every layer
    // tile has a design slot; keep the bounds check in case a revision disagrees.
    if (ground && meta) {
      const [dw, dh] = meta.design_dims;
      const wantsDesign = ground.kind === "design" || (ground.kind === "erase" && ground.layer !== "path");
      if (wantsDesign && ((x >> 1) >= dw || (y >> 1) >= dh)) return false;
    }
    const key = `${x},${y}`;
    if (strokeTiles.current.has(key)) return false;
    strokeTiles.current.add(key);
    setStroke((s) => [...s, [x, y]]);
    return true;
  };

  const addTerrainTarget = (tx: number, ty: number) => {
    if (!meta) return false;
    const [tw, th] = meta.terrain_dims;
    const r = (terrainSize - 1) >> 1;
    let added = false;
    for (let dx = 0; dx < terrainSize; dx++) for (let dy = 0; dy < terrainSize; dy++) {
      const x = tx - r + dx, y = ty - r + dy;
      if (x < 0 || y < 0 || x >= tw || y >= th) continue;
      // the game owns beach, sea, rocks and river mouths; skip them in the stroke and preview
      if (!meta.editable[x * th + y]) continue;
      const key = `${x},${y}`;
      if (terrainSeen.current.has(key)) continue;
      terrainSeen.current.add(key);
      terrainTargets.current.push([x, y]);
      const [lx, ly] = terrainLayer([x, y]);
      setStroke((s) => [...s, [lx, ly]]);
      added = true;
    }
    return added;
  };

  /** Commits a ground stroke: one command for paths, then the design grid. */
  const commitGround = async (tiles: [number, number][], brush: GroundBrush) => {
    const before = groundBefore.current;
    groundBefore.current = null;
    if (!before || !tiles.length || !meta) return;
    const uniq = (a: [number, number][]) => [...new Map(a.map((p) => [p.join(","), p] as [string, [number, number]])).values()];
    const [dw, dh] = meta.design_dims;
    const designs = uniq(tiles.map(([x, y]) => [x >> 1, y >> 1] as [number, number]))
      .filter(([x, y]) => x < dw && y < dh)
      .filter(([x, y]) => brush.kind !== "design" || !meta.design_fixed[x * dh + y]);
    const terrain = uniq(
      tiles
        .map(([x, y]) => terrainTile(x, y))
        .filter(
          (p): p is [number, number] =>
            !!p && (brush.kind === "path" || brush.kind === "erase" ? terrainEditable(p) : true),
        ),
    );
    // a tile holds a default path and a design in separate save fields, so painting one keeps
    // the other; only the erase brushes remove a layer. See docs/map-editor.md.
    const layer = brush.kind === "erase" ? brush.layer : brush.kind;
    const wantPath = layer === "path" || layer === "both";
    const wantDesign = layer === "design" || layer === "both";
    const next = meta.design_tiles.slice();
    if (wantDesign) {
      for (const [x, y] of designs) next[x * dh + y] = brush.kind === "design" ? brush.index : 0xf800;
    }
    const designsChanged = wantDesign && designs.some(([x, y]) => next[x * dh + y] !== meta.design_tiles[x * dh + y]);
    if ((!wantPath || !terrain.length) && !designsChanged) return;
    try {
      const prevTerrain = await before.terrain;
      if (wantPath && terrain.length) await api.paintRoad(terrain, brush.kind === "path" ? brush.family : null, roadSharp);
      if (designsChanged) { await api.setDesignTiles(next); setMeta({ ...meta, design_tiles: next }); }
      await loadTerrain();
      undo.current.push({ ground: { terrain: prevTerrain, designs: before.designs } });
      if (undo.current.length > 100) undo.current.shift();
      redo.current = [];
      touch();
    } catch (e) { toast("error", "Could not paint the ground", String(e)); }
  };

  /**
   * Commits one terrain stroke. Water also clears ground designs on the painted tiles.
   * The backend refuses a two-level step and warns about trees/buildings within one terrain tile.
   */
  const commitTerrain = async (targets: [number, number][], brush: TerrainBrush) => {
    const before = groundBefore.current;
    groundBefore.current = null;
    if (!before || !targets.length || !meta) return;
    const [dw, dh] = meta.design_dims;
    const designs = brush.kind === "water" ? meta.design_tiles.slice() : null;
    let designsDirty = false;
    if (designs) {
      for (const t of targets) {
        const [lx, ly] = terrainLayer(t);
        const dx = lx >> 1, dy = ly >> 1;
        if (dx >= 0 && dy >= 0 && dx < dw && dy < dh && designs[dx * dh + dy] !== 0xf800) {
          designs[dx * dh + dy] = 0xf800;
          designsDirty = true;
        }
      }
    }
    try {
      const prevTerrain = await before.terrain;
      const res = brush.kind === "cliff"
        ? await api.paintHeight(targets, brush.delta, sharp)
        : await api.paintWater(targets, brush.kind === "water", sharp);
      if (!res.changed && !designsDirty) { toast("info", "Nothing changed"); return; }
      undo.current.push({ ground: { terrain: prevTerrain, designs: before.designs } });
      if (undo.current.length > 100) undo.current.shift();
      redo.current = [];
      if (designsDirty && designs) {
        await api.setDesignTiles(designs);
      }
      await loadTerrain();
      // refresh elevations for the contours and hover height, plus the design grid
      setMeta(await api.mapMeta());
      touch();
      const near = [
        res.trees && `${res.trees} tree${res.trees === 1 ? "" : "s"}`,
        res.buildings && `${res.buildings} building${res.buildings === 1 ? "" : "s"}`,
      ].filter(Boolean).join(" and ");
      const notes = [
        near && `${near} within one terrain tile. The game normally keeps these away from cliffs and rivers.`,
        res.warnings && `${res.warnings} water edge${res.warnings === 1 ? "" : "s"} at a corner, where the game has no waterfall model, so ${res.warnings === 1 ? "it" : "they"} stayed a river.`,
      ].filter(Boolean).join(" ");
      const title = res.changed
        ? `${res.changed} terrain tile${res.changed === 1 ? "" : "s"} changed${res.falls ? `, ${res.falls} waterfall tile${res.falls === 1 ? "" : "s"}` : ""}`
        : "The terrain did not change";
      toast(notes ? "warn" : res.changed ? "ok" : "info", title, notes || undefined);
    } catch (e) { toast("error", "Could not paint the terrain", String(e)); }
  };

  /** Manual one-tile model stamp for fixing an edge the autotiler cannot build. */
  const commitShape = async (t: [number, number]) => {
    if (!tbrush || tbrush.kind !== "shape" || !meta) return;
    try {
      const prev = await api.terrain();
      const bytes = prev.slice();
      const o = (t[0] * meta.terrain_dims[1] + t[1]) * 14;
      const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      dv.setUint16(o, tbrush.model, true);
      dv.setUint16(o + 2, 0, true); // variation
      dv.setUint16(o + 4, tbrush.angle, true);
      await api.setTerrain(bytes);
      await loadTerrain();
      undo.current.push({ ground: { terrain: prev, designs: meta.design_tiles.slice() } });
      if (undo.current.length > 100) undo.current.shift();
      redo.current = [];
      touch();
      toast("ok", "Terrain shape changed");
    } catch (e) { toast("error", "Could not change the terrain shape", String(e)); }
  };

  /** Choosing another top-bar tool drops the ground and terrain brushes, like upstream's painter. */
  const chooseTool = (t: Tool) => {
    setTool(t);
    pasteMode.current = false;
    if (t !== "place") { setGround(null); setTbrush(null); }
  };

  const onDown = (e: React.PointerEvent) => {
    const r = canvas.current!.getBoundingClientRect();
    const cx = e.clientX - r.left, cy = e.clientY - r.top;
    const [tx, ty] = toTile(cx, cy);
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    if (e.button === 1 || tool === "pan" || space.current) { drag.current = { mode: "pan", sx: cx, sy: cy, vx: view.x, vy: view.y, tx, ty, dirty: false, last: "" }; return; }
    if (e.button !== 0) return;
    if (pick) {
      const mx = tx + L.offset_x, my = ty + L.offset_y;
      if (pick.kind === "building" && buildings) { const next = buildings.slice(); next[pick.index] = { ...next[pick.index], x: Math.max(0, mx), y: Math.max(0, my) }; setBuildings(next); api.setBuildings(next).then(touch); }
      setPick(null);
      return;
    }
    if (tool === "place" && tbrush) {
      const t = terrainTile(tx, ty);
      if (!t) { toast("info", "Terrain painting works inside the editable map", "The outer sea border is acre data, not terrain."); return; }
      if (tbrush.kind === "shape") { commitShape(t); return; }
      terrainSeen.current = new Set();
      terrainTargets.current = [];
      setStroke([]);
      groundBefore.current = { designs: meta!.design_tiles.slice(), terrain: api.terrain() };
      drag.current = { mode: "terrain", sx: cx, sy: cy, vx: 0, vy: 0, tx, ty, dirty: false, last: "" };
      addTerrainTarget(t[0], t[1]);
      return;
    }
    if (tool === "place" && ground) {
      strokeTiles.current = new Set();
      setStroke([]);
      groundBefore.current = { designs: meta!.design_tiles.slice(), terrain: api.terrain() };
      drag.current = { mode: "ground", sx: cx, sy: cy, vx: 0, vy: 0, tx, ty, dirty: false, last: "" };
      if (inLayer(tx, ty)) addGroundTile(tx, ty);
      return;
    }
    if (tool === "inspect" && show.buildings && buildings) {
      const mx = tx + L.offset_x, my = ty + L.offset_y;
      const hit = doorAt(mx, my);
      if (hit >= 0) {
        setSelBuilding(hit); setPanel("buildings");
        // a fixed structure still selects from its door, but only movable kinds drag
        if (buildingMovable(gd, buildings[hit].kind)) bdrag.current = { index: hit, startX: buildings[hit].x, startY: buildings[hit].y, mx, my, next: null };
        return;
      }
    }
    if (tool === "inspect") {
      if (show.buildings && buildings) {
        const mx = tx + L.offset_x, my = ty + L.offset_y;
        const hit = buildings.findIndex((b) => {
          if (!b.kind) return false;
          return buildingRects(gd, b.kind).some(([dx, dy, dw, dh]) =>
            mx >= b.x + dx && mx < b.x + dx + dw && my >= b.y + dy && my < b.y + dy + dh);
        });
        if (hit >= 0) { setSelBuilding(hit); setPanel("buildings"); return; }
      }
      if (inLayer(tx, ty)) { setSelTile(grid().rootOf(tx, ty) ?? [tx, ty]); setPanel("tile"); }
      return;
    }
    if (tool === "rect") {
      if (pasteMode.current && clip.current) { pasteClip(tx, ty); return; }
      drag.current = { mode: "rect", sx: cx, sy: cy, vx: 0, vy: 0, tx, ty, dirty: false, last: "" };
      setRect(inLayer(tx, ty) ? { x: tx, y: ty, w: 1, h: 1 } : null);
      return;
    }
    snapshot();
    const d = { mode: tool === "place" ? "paint" as const : "erase" as const, sx: cx, sy: cy, vx: 0, vy: 0, tx, ty, dirty: false, last: `${tx},${ty}` };
    d.dirty = tool === "place" ? paintAt(tx, ty, true) : eraseAt(tx, ty);
    drag.current = d;
    if (d.dirty) changed(layer);
  };

  const onMove = (e: React.PointerEvent) => {
    const r = canvas.current!.getBoundingClientRect();
    const cx = e.clientX - r.left, cy = e.clientY - r.top;
    const [tx, ty] = toTile(cx, cy);
    if (!hover || hover[0] !== tx || hover[1] !== ty) setHover([tx, ty]);
    const bd = bdrag.current;
    if (bd && buildings) {
      // one terrain tile per step keeps every plot aligned to the terrain grid
      const [fw, fh] = buildingFootprint(gd, buildings[bd.index].kind);
      const nx = Math.max(L.offset_x + fw, Math.min(L.offset_x + L.width - fw, bd.startX + Math.round((tx + L.offset_x - bd.mx) / 2) * 2));
      const ny = Math.max(L.offset_y + fh, Math.min(L.offset_y + L.height - fh, bd.startY + Math.round((ty + L.offset_y - bd.my) / 2) * 2));
      if (buildings[bd.index].x !== nx || buildings[bd.index].y !== ny) {
        const next = buildings.slice();
        next[bd.index] = { ...next[bd.index], x: nx, y: ny };
        setBuildings(next); bd.next = next;
      }
      return;
    }
    const d = drag.current;
    if (!d) return;
    if (d.mode === "pan") { setView({ ...view, x: d.vx + cx - d.sx, y: d.vy + cy - d.sy }); return; }
    if (d.mode === "rect") {
      const x0 = Math.max(0, Math.min(d.tx, tx)), y0 = Math.max(0, Math.min(d.ty, ty));
      const x1 = Math.min(L.width - 1, Math.max(d.tx, tx)), y1 = Math.min(L.height - 1, Math.max(d.ty, ty));
      if (x1 >= x0 && y1 >= y0) setRect({ x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 });
      return;
    }
    if (d.mode === "ground") { if (inLayer(tx, ty)) addGroundTile(tx, ty); return; }
    if (d.mode === "terrain") { const t = terrainTile(tx, ty); if (t) addTerrainTarget(t[0], t[1]); return; }
    const key = `${tx},${ty}`;
    if (key === d.last) return;
    d.last = key;
    if (d.mode === "paint") {
      const [fw, fh] = footprint(gd, brush);
      if ((((tx - d.tx) % fw) + fw) % fw !== 0 || (((ty - d.ty) % fh) + fh) % fh !== 0) return; // stamp on the brush grid
      if (paintAt(tx, ty, false)) { d.dirty = true; changed(layer); }
    } else if (eraseAt(tx, ty)) { d.dirty = true; changed(layer); }
  };

  const onUp = () => {
    const bd = bdrag.current;
    bdrag.current = null;
    if (bd?.next) api.setBuildings(bd.next).then(touch).catch((e) => toast("error", "Could not save buildings", String(e)));
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    if (d.mode === "ground") {
      const tiles = [...strokeTiles.current].map((k) => k.split(",").map(Number) as [number, number]);
      strokeTiles.current = new Set();
      setStroke([]);
      if (ground) commitGround(tiles, ground);
      return;
    }
    if (d.mode === "terrain") {
      const targets = terrainTargets.current;
      terrainTargets.current = [];
      terrainSeen.current = new Set();
      setStroke([]);
      if (tbrush) commitTerrain(targets, tbrush);
      return;
    }
    if (d.mode === "paint" || d.mode === "erase") {
      if (d.dirty) sync(layer); else undo.current.pop();
    }
  };

  /** A cancelled pointer restores the drag preview and writes nothing. */
  const onCancel = () => {
    const bd = bdrag.current;
    bdrag.current = null;
    if (bd?.next) {
      const next = bd.next.slice();
      next[bd.index] = { ...next[bd.index], x: bd.startX, y: bd.startY };
      setBuildings(next);
    }
    onUp();
  };

  // wheel: pinch / ⌘-scroll zooms, plain scroll pans (trackpad friendly)
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = c.getBoundingClientRect();
      const cx = e.clientX - r.left, cy = e.clientY - r.top;
      setView((v) => {
        if (e.ctrlKey || e.metaKey) {
          const s = Math.min(MAX_S, Math.max(MIN_S, v.s * Math.exp(-e.deltaY * (e.ctrlKey ? 0.012 : 0.004))));
          return { s, x: cx - ((cx - v.x) / v.s) * s, y: cy - ((cy - v.y) / v.s) * s };
        }
        return { ...v, x: v.x - e.deltaX, y: v.y - e.deltaY };
      });
    };
    c.addEventListener("wheel", wheel, { passive: false });
    return () => c.removeEventListener("wheel", wheel);
  }, [loaded]);

  const zoomBy = (f: number) => {
    const r = wrap.current!.getBoundingClientRect();
    const cx = r.width / 2, cy = r.height / 2;
    setView((v) => { const s = Math.min(MAX_S, Math.max(MIN_S, v.s * f)); return { s, x: cx - ((cx - v.x) / v.s) * s, y: cy - ((cy - v.y) / v.s) * s }; });
  };

  const copyRect = (cut = false) => {
    if (!rect) return;
    clip.current = { w: rect.w, h: rect.h, tiles: grid().extract(rect.x, rect.y, rect.w, rect.h) };
    if (cut) { snapshot(); const g = grid(); for (let x = 0; x < rect.w; x++) for (let y = 0; y < rect.h; y++) g.set(rect.x + x, rect.y + y, NONE_ITEM); changed(layer); sync(layer); }
    toast("info", `${cut ? "Cut" : "Copied"} ${rect.w}×${rect.h} tiles`, `Press ${MOD}V, then click where the top-left corner should go.`);
  };
  const pasteClip = (x: number, y: number) => {
    const c = clip.current;
    pasteMode.current = false;
    if (!c || !inLayer(x, y)) return;
    snapshot();
    const n = grid().paste(x, y, c.w, c.h, c.tiles, true);
    setRect({ x, y, w: Math.min(c.w, L.width - x), h: Math.min(c.h, L.height - y) });
    changed(layer); sync(layer);
    toast("ok", `Pasted ${n} tiles`);
  };
  const deleteRect = () => {
    if (!rect) return;
    snapshot();
    const g = grid();
    for (let x = rect.x; x < rect.x + rect.w; x++) for (let y = rect.y; y < rect.y + rect.h; y++) g.removeAt(gd, x, y);
    changed(layer); sync(layer);
  };

  // keyboard shortcuts
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable) return;
      const mod = e.metaKey || e.ctrlKey;
      if (e.code === "Space") { space.current = true; e.preventDefault(); return; }
      if (mod && e.key.toLowerCase() === "z") { e.preventDefault(); e.shiftKey ? doUndo(redo, undo) : doUndo(undo, redo); return; }
      if (mod && e.key.toLowerCase() === "y") { e.preventDefault(); doUndo(redo, undo); return; }
      if (mod && e.key.toLowerCase() === "c") { if (rect) { e.preventDefault(); copyRect(); } return; }
      if (mod && e.key.toLowerCase() === "x") { if (rect) { e.preventDefault(); copyRect(true); } return; }
      if (mod && e.key.toLowerCase() === "v") { if (clip.current) { e.preventDefault(); setTool("rect"); pasteMode.current = true; toast("info", "Click on the map to paste"); } return; }
      if (mod) return;
      const k = e.key.toLowerCase();
      if (k === "v") chooseTool("inspect"); else if (k === "b") chooseTool("place"); else if (k === "e") chooseTool("erase"); else if (k === "m") chooseTool("rect"); else if (k === "h") chooseTool("pan");
      else if (k === "r" && tool === "place") setBrush((b) => ({ ...b, system: (b.system & ~3) | (((b.system & 3) + 1) & 3) }));
      else if (k === "escape") { setRect(null); setPick(null); pasteMode.current = false; }
      else if ((k === "delete" || k === "backspace") && rect) deleteRect();
      else if (k === "1") setLayer(0); else if (k === "2") setLayer(1);
      else if (k === "=" || k === "+") zoomBy(1.25); else if (k === "-") zoomBy(0.8);
    };
    const up = (e: KeyboardEvent) => { if (e.code === "Space") space.current = false; };
    window.addEventListener("keydown", down); window.addEventListener("keyup", up);
    return () => { window.removeEventListener("keydown", down); window.removeEventListener("keyup", up); };
  });

  if (!loaded || !meta || !buildings) return <Loading />;
  const hovered = hover && inLayer(hover[0], hover[1]) ? grid().get(hover[0], hover[1]) : null;
  const hoverDesign = hover && inLayer(hover[0], hover[1]) && meta
    && (hover[0] >> 1) < meta.design_dims[0] && (hover[1] >> 1) < meta.design_dims[1]
    ? meta.design_tiles[(hover[0] >> 1) * meta.design_dims[1] + (hover[1] >> 1)]
    : 0xf800;
  const hoverTerrain = hover ? terrainTile(hover[0], hover[1]) : null;
  const acreLabel = (x: number, y: number) => { const mx = x + L.offset_x, my = y + L.offset_y; return String.fromCharCode(65 + Math.floor(mx / 32)) + (Math.floor(my / 32) + 1); };
  const hoverBuilding = tool === "inspect" && show.buildings && hover ? doorAt(hover[0] + L.offset_x, hover[1] + L.offset_y) : -1;
  const hoverDoor = hoverBuilding >= 0 && buildings ? buildingMovable(gd, buildings[hoverBuilding].kind) : false;
  const cursor = pick ? "crosshair" : bdrag.current ? "grabbing" : tool === "pan" ? "grab" : tool === "inspect" ? (hoverDoor ? "grab" : "default") : "crosshair";

  return (
    <div className="content flush" style={{ flexDirection: "row" }}>
      <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <div className="row wrap" style={{ padding: "10px 14px", borderBottom: "1px solid var(--border)", background: "var(--card)", gap: 10 }}>
          <Segmented value={tool} onChange={chooseTool} options={[
            { value: "inspect", label: "Select", icon: <MousePointer2 /> }, { value: "place", label: "Place", icon: <Brush /> },
            { value: "erase", label: "Erase", icon: <Eraser /> }, { value: "rect", label: "Area", icon: <SquareDashed /> }, { value: "pan", label: "Pan", icon: <Hand /> }]} />
          <Segmented value={layer} onChange={(l) => { setLayer(l); setSelTile(null); }} options={[{ value: 0, label: "Layer 1", icon: <Layers /> }, { value: 1, label: "Layer 2" }]} />
          <div className="row" style={{ gap: 4 }}>
            <Button size="sm" iconOnly icon={<Undo2 />} title={`Undo (${MOD}Z)`} onClick={() => doUndo(undo, redo)} disabled={!undo.current.length} />
            <Button size="sm" iconOnly icon={<Redo2 />} title={`Redo (⇧${MOD}Z)`} onClick={() => doUndo(redo, undo)} disabled={!redo.current.length} />
          </div>
          {tool === "rect" && rect && (
            <div className="row" style={{ gap: 4 }}>
              <Button size="sm" iconOnly icon={<Copy />} title={`Copy (${MOD}C)`} onClick={() => copyRect()} />
              <Button size="sm" iconOnly icon={<Scissors />} title={`Cut (${MOD}X)`} onClick={() => copyRect(true)} />
              <Button size="sm" iconOnly icon={<Trash2 />} title="Delete (⌫)" onClick={deleteRect} />
            </div>
          )}
          {tool === "rect" && clip.current && <Button size="sm" icon={<ClipboardPaste />} onClick={() => { pasteMode.current = true; toast("info", "Click on the map to paste"); }}>Paste</Button>}
          <div style={{ flex: 1 }} />
          <MenuButton label="View" icon={<Eye />} size="sm" align="right" items={([
            ["other", "Other layer (dimmed)"], ["grid", "Grid & acre names"], ["buildings", "Buildings"], ["plaza", "Plaza"], ["sprites", "Item pictures when zoomed in"], ["designs", "Ground designs"], ["contours", "Height contours"],
          ] as [keyof typeof show, string][]).map(([k, label]) => ({ label, icon: show[k] ? <CheckIcon /> : <span style={{ width: 14 }} />, onClick: () => setShow({ ...show, [k]: !show[k] }) }))} />
          <div className="row" style={{ gap: 4 }}>
            <Button size="sm" iconOnly icon={<ZoomOut />} onClick={() => zoomBy(0.8)} title="Zoom out (−)" />
            <span className="small faint mono" style={{ width: 44, textAlign: "center" }}>{Math.round(view.s * 100 / 4)}%</span>
            <Button size="sm" iconOnly icon={<ZoomIn />} onClick={() => zoomBy(1.25)} title="Zoom in (+)" />
          </div>
        </div>
        <div ref={wrap} style={{ flex: 1, position: "relative", overflow: "hidden" }}>
          <canvas ref={canvas} style={{ position: "absolute", inset: 0, cursor, touchAction: "none" }}
            onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={onCancel} onPointerLeave={() => setHover(null)} onContextMenu={(e) => e.preventDefault()} />
          {pick && <div style={{ position: "absolute", top: 12, left: "50%", transform: "translateX(-50%)" }}><Badge kind="accent">Click on the map to place the building · Esc to cancel</Badge></div>}
          <div className="small" style={{ position: "absolute", left: 12, bottom: 10, padding: "4px 10px", borderRadius: 8, background: "color-mix(in srgb, var(--card) 88%, transparent)", border: "1px solid var(--border)", pointerEvents: "none" }}>
            {hover && inLayer(hover[0], hover[1]) ? <>Acre <b>{acreLabel(hover[0], hover[1])}</b> · X {hover[0]} Y {hover[1]} · {itemLabel(gd, hovered!)}{hovered && hovered.id !== NONE_ID && hovered.id !== EXT_ID && <span className="faint"> · 0x{hex(hovered.id)}</span>}{hoverDesign !== 0xf800 && <span className="faint"> · {hoverDesign < save!.pattern_count ? `Ground design #${hoverDesign}` : `Unknown ground value 0x${hoverDesign.toString(16).toUpperCase()}`}</span>}{hoverTerrain && <span className="faint"> · Height {meta.elevations[hoverTerrain[0] * meta.terrain_dims[1] + hoverTerrain[1]]}</span>}</> : <span className="faint">{`Scroll to pan · pinch or ${MOD}-scroll to zoom · Space-drag to pan`}</span>}
          </div>
        </div>
      </div>

      <aside style={{ width: 360, flex: "none", borderLeft: "1px solid var(--border)", background: "var(--card)", display: "flex", flexDirection: "column", minHeight: 0 }}>
        <div style={{ padding: "6px 6px 0" }}>
          <Tabs value={panel} onChange={setPanel} tabs={[
            { value: "tile", label: "Item", icon: <Crosshair /> }, { value: "bulk", label: "Bulk", icon: <Sparkles /> },
            { value: "buildings", label: "Buildings", icon: <Building2 /> }, { value: "island", label: "Island", icon: <MapIcon /> }, { value: "dumps", label: "Files", icon: <Download /> }]} />
        </div>
        <div style={{ flex: 1, overflowY: "auto", padding: 16 }} className="col">
          {panel === "tile" && renderTile()}
          {panel === "bulk" && renderBulk()}
          {panel === "buildings" && renderBuildings()}
          {panel === "island" && renderIsland()}
          {panel === "dumps" && renderDumps()}
        </div>
      </aside>
    </div>
  );

  // ------------------------------------------------------------- panels (closures over editor state)
  function renderTile() {
    const selItem = selTile ? grid().get(selTile[0], selTile[1]) : null;
    return (
      <div className="col" style={{ gap: 16 }}>
        <div className="col" style={{ gap: 8 }}>
          <div className="row"><div className="label" style={{ flex: 1 }}>Brush</div><span className="tiny faint"><span className="kbd">B</span> place · <span className="kbd">R</span> rotate</span></div>
          <ItemEditor item={brush} onChange={(it) => { setGround(null); setTbrush(null); setBrush(it); }} />
          <div className="row">
            <Button size="sm" icon={<RotateCw />} onClick={() => setBrush({ ...brush, system: (brush.system & ~3) | (((brush.system & 3) + 1) & 3) })}>Rotate</Button>
            <Button size="sm" icon={<Brush />} variant={tool === "place" && !ground ? "primary" : "default"} onClick={() => { setGround(null); setTbrush(null); setTool("place"); }}>Paint with it</Button>
          </div>
        </div>
        <div className="divider" />
        {selTile && selItem ? (
          <div className="col" style={{ gap: 8 }}>
            <div className="label">Selected tile · {acreLabel(selTile[0], selTile[1])} · X {selTile[0]} Y {selTile[1]}</div>
            {selItem.id === NONE_ID ? <div className="faint small">Nothing here.</div> : selItem.id === EXT_ID ? (
              <Callout kind="warn" title="Orphaned extension tile">This tile belongs to a multi-tile item that no longer exists. <Button size="sm" onClick={() => { snapshot(); grid().set(selTile[0], selTile[1], NONE_ITEM); changed(layer); sync(layer); }}>Remove it</Button></Callout>
            ) : (
              <>
                <ItemEditor item={selItem} onChange={(it) => {
                  const g = grid().clone();
                  g.removeAt(gd, selTile[0], selTile[1]);
                  if (it.id !== NONE_ID) {
                    const fit = g.check(gd, it, selTile[0], selTile[1]);
                    if (fit === "bounds" || (fit === "collision" && settings?.map_prevent_overlap)) { toast("warn", "The changed item doesn't fit here"); return; }
                    g.place(gd, it, selTile[0], selTile[1], settings?.map_auto_extensions ?? true);
                  }
                  snapshot();
                  layers.current[layer] = g.tiles;
                  changed(layer); sync(layer);
                }} />
                <div className="row">
                  <Button size="sm" icon={<Brush />} onClick={() => { setGround(null); setTbrush(null); setBrush(selItem); setTool("place"); }}>Use as brush</Button>
                  <Button size="sm" variant="danger" icon={<Trash2 />} onClick={() => { snapshot(); grid().removeAt(gd, selTile[0], selTile[1]); changed(layer); sync(layer); setSelTile(null); }}>Remove</Button>
                </div>
              </>
            )}
          </div>
        ) : <div className="faint small">Use <b>Select</b> (<span className="kbd">V</span>) and click a tile to inspect and edit the item on it.</div>}
        <div className="divider" />
        <div className="col" style={{ gap: 8 }}>
          <div className="row"><div className="label" style={{ flex: 1 }}>Ground</div><span className="tiny faint">paths and designs</span></div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 6 }}>
            {PATHS.map((p) => (
              <button key={p.id} title={`Paint ${p.label}`} onClick={() => { setTbrush(null); setGround({ kind: "path", family: p.id }); setTool("place"); }}
                style={{ padding: "0 0 3px", borderRadius: 8, overflow: "hidden", cursor: "pointer", background: "var(--card-2)",
                  border: ground?.kind === "path" && ground.family === p.id ? "2px solid var(--accent)" : "1px solid var(--border)" }}>
                <span style={{ display: "block", height: 14, background: p.color }} />
                <span className="tiny" style={{ display: "block", paddingTop: 2 }}>{p.label}</span>
              </button>
            ))}
          </div>
          <div className="row">
            <span className="tiny faint" style={{ minWidth: 40 }}>Corners</span>
            <Select value={roadSharp ? 1 : 0} onChange={(v) => setRoadSharp(v === 1)}
              options={[{ value: 0, label: "Rounded" }, { value: 1, label: "Sharp" }]} style={{ width: 110 }} />
            <span className="tiny faint">path bends</span>
          </div>
          <div className="row wrap">
            <Button size="sm" variant={ground?.kind === "erase" && ground.layer === "both" ? "primary" : "default"} icon={<Eraser />} onClick={() => { setTbrush(null); setGround({ kind: "erase", layer: "both" }); setTool("place"); }}>Erase ground</Button>
            <Button size="sm" variant={ground?.kind === "erase" && ground.layer === "path" ? "primary" : "default"} icon={<Eraser />} onClick={() => { setTbrush(null); setGround({ kind: "erase", layer: "path" }); setTool("place"); }}>Erase path</Button>
            <Button size="sm" variant={ground?.kind === "erase" && ground.layer === "design" ? "primary" : "default"} icon={<Eraser />} onClick={() => { setTbrush(null); setGround({ kind: "erase", layer: "design" }); setTool("place"); }}>Erase design</Button>
            {ground && <Button size="sm" variant="ghost" onClick={() => setGround(null)}>Stop</Button>}
          </div>
          <div className="label" style={{ marginTop: 4 }}>My designs</div>
          {patterns === null ? <div className="tiny faint">Loading…</div> : patterns.length === 0 ? (
            <div className="tiny faint">No saved designs. Draw one on the Designs page first.</div>
          ) : (
            <div style={{ flex: "none", maxHeight: 170, overflowY: "auto" }}>
              {/* the panel is a flex column; without this wrapper the grid rows get squeezed instead of scrolling */}
              <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 6 }}>
                {patterns.map((p) => (
                  <button key={p.index} title={p.name || `Design ${p.index + 1}`} onClick={() => { setTbrush(null); setGround({ kind: "design", index: p.index }); setTool("place"); }}
                    style={{ padding: 0, borderRadius: 8, overflow: "hidden", cursor: "pointer", background: "var(--card-2)",
                      border: ground?.kind === "design" && ground.index === p.index ? "2px solid var(--accent)" : "1px solid var(--border)" }}>
                    <img src={asset(`design/normal/${p.index}?r=${designEpoch}`)} alt="" style={{ display: "block", width: "100%", aspectRatio: "1", imageRendering: "pixelated" }} />
                  </button>
                ))}
              </div>
            </div>
          )}
          <div className="tiny faint">
            {ground
              ? <>Painting <b>{ground.kind === "path" ? PATHS.find((p) => p.id === ground.family)?.label : ground.kind === "design" ? "a custom design" : ground.layer === "both" ? "paths and designs away" : `only the ${ground.layer === "path" ? "path" : "design"} away`}</b>. Drag on the map; pick an item above to place items again.</>
              : <>Click a path or design, then drag on the map. Default paths join up automatically, and a path and a design can share a tile. The Corners choice styles new path bends; PRO designs can't be placed on the ground in game, so they aren't offered.</>}
          </div>
        </div>
        <div className="divider" />
        <div className="col" style={{ gap: 8 }}>
          <div className="row"><div className="label" style={{ flex: 1 }}>Terrain</div><span className="tiny faint">rivers, cliffs, falls</span></div>
          {meta && meta.max_elevation > 3 && (
            <Callout kind="warn" title="Terrain above the game limit">
              This save has tiles at elevation {meta.max_elevation}. The game supports 0–3; existing tiles are
              left unchanged, and painting is limited to 0–3.
            </Callout>
          )}
          <div className="row wrap">
            <Button size="sm" icon={<Droplets />} variant={tbrush?.kind === "water" ? "primary" : "default"}
              onClick={() => { setGround(null); setTbrush({ kind: "water" }); setTool("place"); }}>Water</Button>
            <Button size="sm" icon={<Eraser />} variant={tbrush?.kind === "water-erase" ? "primary" : "default"}
              onClick={() => { setGround(null); setTbrush({ kind: "water-erase" }); setTool("place"); }}>Erase water</Button>
            <Button size="sm" icon={<ArrowUp />} variant={tbrush?.kind === "cliff" && tbrush.delta > 0 ? "primary" : "default"}
              onClick={() => { setGround(null); setTbrush({ kind: "cliff", delta: 1 }); setTool("place"); }}>Raise</Button>
            <Button size="sm" icon={<ArrowDown />} variant={tbrush?.kind === "cliff" && tbrush.delta < 0 ? "primary" : "default"}
              onClick={() => { setGround(null); setTbrush({ kind: "cliff", delta: -1 }); setTool("place"); }}>Lower</Button>
            {tbrush && <Button size="sm" variant="ghost" onClick={() => setTbrush(null)}>Stop</Button>}
          </div>
          <div className="row">
            <span className="tiny faint" style={{ minWidth: 40 }}>Brush</span>
            <Select value={terrainSize} onChange={setTerrainSize}
              options={[1, 2, 3, 4, 5].map((n) => ({ value: n, label: `${n} × ${n}` }))} style={{ width: 110 }} />
          </div>
          <div className="row">
            <span className="tiny faint" style={{ minWidth: 40 }}>Corners</span>
            <Select value={sharp ? 1 : 0} onChange={(v) => setSharp(v === 1)}
              options={[{ value: 0, label: "Rounded" }, { value: 1, label: "Sharp" }]} style={{ width: 110 }} />
          </div>
          <div className="tiny faint">
            {tbrush?.kind === "water" ? <>Drag to paint rivers. Water keeps the tile height and drops into a waterfall where it sits one level above anything lower. No river behind the fall is needed: the game draws a recessed fall into the cliff face. Water clears ground designs.</>
              : tbrush?.kind === "water-erase" ? <>Drag to erase rivers. The tile becomes grass at the same height; cliff faces next to it are restored.</>
              : tbrush?.kind === "cliff" ? <>Drag to {tbrush.delta > 0 ? "raise" : "lower"} land. Faces are generated automatically for single tiles, strips, ridges and whole plateaus; increase the brush for larger areas. Water has to be erased first. New corners use the <b>{sharp ? "sharp" : "rounded"}</b> style; corners that already exist keep theirs.</>
              : <>Pick a brush and drag on the map. Rivers drop into a waterfall where the land steps down one level. Paths and designs are painted separately under Ground.</>}
          </div>
        </div>
        <div className="divider" />
        <div className="tiny faint" style={{ lineHeight: 1.7 }}>
          <span className="kbd">V</span> select · <span className="kbd">B</span> place · <span className="kbd">E</span> erase · <span className="kbd">M</span> area · <span className="kbd">H</span> pan · <span className="kbd">1</span>/<span className="kbd">2</span> layer · <span className="kbd">{MOD}Z</span> undo · <span className="kbd">{MOD}C</span>/<span className="kbd">{MOD}V</span> copy/paste area
        </div>
      </div>
    );
  }

  function renderBulk() {
    const { kind, from, to, idOnly, src, spawnItem, count, diy, list, dropped, arrange, overwrite, pasting } = bs;
    const up = <K extends keyof typeof bs>(k: K) => (v: (typeof bs)[K]) => setBs((p) => ({ ...p, [k]: v }));
    const [setKind, setFrom, setTo, setIdOnly, setSrc, setSpawnItem, setCount, setDiy, setList, setDropped, setArrange, setOverwrite, setPasting] =
      [up("kind"), up("from"), up("to"), up("idOnly"), up("src"), up("spawnItem"), up("count"), up("diy"), up("list"), up("dropped"), up("arrange"), up("overwrite"), up("pasting")];
    const target = rect ?? undefined;
    const where = rect ? `selected area (${rect.w}×${rect.h})` : "whole layer";
    const origin = rect ? [rect.x, rect.y] : selTile;
    return (
      <div className="col" style={{ gap: 18 }}>
        <Callout kind="info">Applies to the <b>{where}</b> of layer {layer + 1}. Use the <b>Area</b> tool (<span className="kbd">M</span>) to limit it. Every action can be undone.</Callout>
        <section className="col">
          <div className="label">Remove</div>
          <div className="row">
            <Select value={kind} onChange={setKind} options={CLEAR_KINDS.map(([v, label]) => ({ value: v, label }))} style={{ flex: 1 }} />
            <Button icon={<Trash2 />} onClick={async () => {
              if (kind === "everything" && !(await confirm({ title: `Remove everything in the ${where}?`, danger: true, confirm: "Remove" }))) return;
              bulk("Remove " + CLEAR_KINDS.find(([k]) => k === kind)![1].toLowerCase(), layer, () => api.fieldClear(layer, kind, target));
            }}>Remove</Button>
          </div>
        </section>
        <section className="col">
          <div className="label">Flowers</div>
          <div className="row">
            <Button icon={<Droplets />} onClick={() => bulk("Water flowers", layer, () => api.fieldWater(layer, false, target))}>Water</Button>
            <Button icon={<Sparkles />} onClick={() => bulk("Gold-water flowers", layer, () => api.fieldWater(layer, true, target))} title="Watered by every visitor (best breeding odds)">Water by all visitors</Button>
          </div>
        </section>
        <section className="col">
          <div className="label">Replace</div>
          <Field label="Find"><ItemPicker value={from} onChange={setFrom} /></Field>
          <Field label="Replace with"><ItemPicker value={to} onChange={setTo} /></Field>
          <Check label="Match by item only (ignore variant, count, flags)" checked={idOnly} onChange={setIdOnly} />
          <Button icon={<Replace />} disabled={from === NONE_ID || to === NONE_ID} onClick={() => bulk("Replace", layer, () => api.fieldReplace(layer, { ...NONE_ITEM, id: from }, { ...NONE_ITEM, id: to }, idOnly, target))}>Replace all</Button>
        </section>
        <section className="col">
          <div className="label">Spawn items</div>
          <Segmented value={src} onChange={setSrc} options={[{ value: "item", label: "One item" }, { value: "diy", label: "DIY cards" }, { value: "list", label: "From a list" }]} />
          {src === "item" && <><ItemEditor item={spawnItem} compact onChange={setSpawnItem} /><Field label="How many"><NumberInput value={count} min={1} max={10000} onChange={setCount} /></Field></>}
          {src === "diy" && (
            <div className="grid cols-2" style={{ gap: 10 }}>
              <Field label="From recipe #"><NumberInput value={diy.min} max={0xffff} onChange={(min) => setDiy({ ...diy, min })} /></Field>
              <Field label="To recipe #"><NumberInput value={diy.max} max={0xffff} onChange={(max) => setDiy({ ...diy, max })} /></Field>
              <Field label="Copies of each"><NumberInput value={diy.count} min={1} max={20} onChange={(c) => setDiy({ ...diy, count: c })} /></Field>
              <Check label="Sort alphabetically" checked={diy.alphabetical} onChange={(alphabetical) => setDiy({ ...diy, alphabetical })} />
            </div>
          )}
          {src === "list" && (
            <div className="row">
              <Button size="sm" onClick={() => setPasting(true)}>Paste a list…</Button>
              <span className="small faint">{list.length} item{list.length === 1 ? "" : "s"}</span>
              <Check label="Dropped" checked={dropped} onChange={setDropped} />
            </div>
          )}
          <div className="row">
            <Select value={arrange} onChange={setArrange} options={[{ value: "square", label: "Square" }, { value: "horizontal", label: "Rows" }, { value: "vertical", label: "Columns" }]} />
            <Check label="Overwrite occupied tiles" checked={overwrite} onChange={setOverwrite} />
          </div>
          <Button variant="primary" icon={<Sparkles />} disabled={!origin} onClick={() => {
            if (!origin) return;
            if (src === "item" && spawnItem.id === NONE_ID) { toast("info", "Choose an item to spawn"); return; }
            if (src === "list" && !list.length) { toast("info", "Paste a list of items first"); return; }
            const source = src === "item" ? { type: "item" as const, item: spawnItem, count } : src === "diy" ? { type: "diy" as const, ...diy } : { type: "list" as const, items: list, dropped };
            bulk("Spawn", layer, () => api.fieldSpawn(layer, origin[0], origin[1], arrange, source, overwrite));
          }}>Spawn {origin ? `at X ${origin[0]} Y ${origin[1]}` : ""}</Button>
          {!origin && <div className="tiny faint">Select a tile or an area first: items are laid out from its top-left corner.</div>}
          {pasting && <PasteListModal onClose={() => setPasting(false)} onAdd={(l) => { setList(l); setPasting(false); }} />}
        </section>
        <section className="col">
          <div className="label">Clean up</div>
          <Button icon={<Grid3x3 />} onClick={() => bulk("Remove orphaned extension tiles", layer, () => api.fieldCleanExtensions(layer))}>Remove orphaned extension tiles</Button>
          <Button onClick={async () => {
            const f = await api.fieldUnsupported();
            setFloating(f); setLayer(1);
            toast(f.length ? "warn" : "ok", f.length ? `${f.length} floating tile${f.length === 1 ? "" : "s"} on layer 2` : "No floating items on layer 2", f.length ? "They are outlined in red. Items on layer 2 need something under them on layer 1." : undefined);
          }}>Find floating items on layer 2</Button>
          {floating.length > 0 && <Button size="sm" variant="ghost" onClick={() => setFloating([])}>Hide highlights</Button>}
        </section>
      </div>
    );
  }

  function renderBuildings() {
    const list = buildings!.map((b, i) => ({ b, i })).filter(({ b }) => showEmpty || b.kind);
    const b = selBuilding !== null ? buildings![selBuilding] : null;
    const upd = (patch: Partial<Building>) => {
      if (selBuilding === null) return;
      const cur = buildings![selBuilding], next = { ...cur, ...patch };
      // mirrors Building::angle_ok: only a bridge (26) or incline (27) may change its angle
      if (next.angle !== cur.angle && next.kind === cur.kind && cur.kind !== 26 && cur.kind !== 27) {
        toast("error", "Buildings can't be rotated", "The game ignores an angle change on a placed structure. Only bridges and inclines carry a real angle.");
        return;
      }
      const list = buildings!.slice(); list[selBuilding] = next;
      setBuildings(list); api.setBuildings(list).then(touch).catch((e) => toast("error", "Could not save buildings", String(e)));
    };
    return (
      <div className="col" style={{ gap: 14 }}>
        {b && selBuilding !== null && (
          <div className="col" style={{ gap: 10, padding: 12, border: "1px solid var(--border)", borderRadius: 12, background: "var(--card-2)" }}>
            <div className="row"><h3 style={{ flex: 1 }}>{buildingName(b.kind)}</h3><span className="tiny faint">#{selBuilding}</span></div>
            <Field label="Type"><Select value={b.kind} onChange={(kind) => upd({ kind })} options={enumOptions(gd, "BuildingType")} /></Field>
            <div className="grid cols-2" style={{ gap: 10 }}>
              <Field label="X"><NumberInput value={b.x} max={0xffff} onChange={(x) => upd({ x })} /></Field>
              <Field label="Y"><NumberInput value={b.y} max={0xffff} onChange={(y) => upd({ y })} /></Field>
            </div>
            <Button size="sm" icon={<Crosshair />} onClick={() => setPick({ kind: "building", index: selBuilding })}>Pick position on the map</Button>
            {settings?.advanced && (
              <div className="grid cols-2" style={{ gap: 10 }}>
                <Field label="Angle"><NumberInput value={b.angle} max={255} onChange={(angle) => upd({ angle })} /></Field>
                <Field label="Bit"><NumberInput value={b.bit} max={255} onChange={(bit) => upd({ bit })} /></Field>
                <Field label="Type value"><NumberInput value={b.type_} max={0xffff} onChange={(type_) => upd({ type_ })} /></Field>
                <Field label="Type argument"><NumberInput value={b.type_arg} max={0xffff} onChange={(type_arg) => upd({ type_arg })} /></Field>
                <Field label="Unique id"><NumberInput value={b.unique_id} max={0xffff} onChange={(unique_id) => upd({ unique_id })} /></Field>
              </div>
            )}
            {buildingMovable(gd, b.kind) ? (
              <Callout kind="warn">NHSE only changes the coordinates. Terrain, rivers, cliffs, trees, furniture and other buildings stay where they are, so check the new plot and its surroundings before saving.</Callout>
            ) : (
              <Callout kind="danger">The game doesn't let you move this structure. Bridges, inclines, the airport, tent, Resident Services, Redd's trawler, the studio and the hotel should stay fixed; moving them can break paths, layouts or the island.</Callout>
            )}
          </div>
        )}
        <div className="row"><div className="label" style={{ flex: 1 }}>All buildings</div><Check label="Show empty slots" checked={showEmpty} onChange={setShowEmpty} /></div>
        <div className="list">
          {list.map(({ b, i }) => (
            <div key={i} className={"list-item " + (i === selBuilding ? "sel" : "")} onClick={() => {
              setSelBuilding(i);
              if (b.kind && wrap.current) { const r = wrap.current.getBoundingClientRect(); setView((v) => ({ ...v, x: r.width / 2 - b.x * v.s, y: r.height / 2 - b.y * v.s })); }
            }}>
              <Building2 size={15} style={{ color: "var(--text-3)" }} />
              <div style={{ flex: 1 }} className="ellipsis">{b.kind ? buildingName(b.kind) : <span className="faint">Empty slot</span>}</div>
              <span className="tiny faint mono">{b.x}, {b.y}</span>
            </div>
          ))}
        </div>
      </div>
    );
  }

  function renderIsland() {
    const m = meta!;
    const setAcre = (i: number, v: number) => { const acres = m.acres.slice(); acres[i] = v; setMeta({ ...m, acres }); api.setAcres(acres).then(() => { touch(); loadTerrain(); }); };
    return (
      <div className="col" style={{ gap: 16 }}>
        <section className="col">
          <div className="label">Resident Services plaza</div>
          <div className="small faint">
            X {m.plaza.x}, Y {m.plaza.z - L.offset_y} · 12×10 terrain tiles. The game doesn't let you move the plaza; it is shown for reference only.
          </div>
        </section>
        <section className="col">
          <div className="label">Acre templates</div>
          <div className="small faint">The template of each acre (beaches, rocks, river mouths, the airport…). Only change these if you know what you're doing.</div>
          {settings?.advanced ? (
            <div style={{ display: "grid", gridTemplateColumns: "repeat(9, 1fr)", gap: 3 }}>
              {m.acres.map((a, i) => (
                <input key={`${i}-${a}`} className="input mono" style={{ padding: "0 2px", height: 26, fontSize: 10, textAlign: "center" }} title={`${String.fromCharCode(65 + (i % 9))}${Math.floor(i / 9) + 1}: ${(gd.enums.OutsideAcre ?? []).find(([, v]) => v === a)?.[0] ?? a}`}
                  defaultValue={hex(a, 3)} onBlur={(e) => { const v = parseInt(e.target.value, 16); if (!Number.isNaN(v) && v !== a && v >= 0 && v <= 0xffff) setAcre(i, v); else e.target.value = hex(a, 3); }} />
              ))}
            </div>
          ) : <Callout kind="info">Turn on <b>Advanced mode</b> in Settings to edit acre templates.</Callout>}
        </section>
        <section className="col">
          <div className="label">Ground designs</div>
          <div className="small faint">{m.design_tiles.filter((t) => t !== 0xf800).length} tiles have a custom design painted on the ground ({m.design_dims[0]}×{m.design_dims[1]} grid). Import or export them from the Files tab.</div>
          <Button size="sm" variant="danger" icon={<Trash2 />} onClick={async () => {
            if (!(await confirm({ title: "Remove every ground design?", message: "Custom designs painted on the ground are erased (the designs themselves stay).", danger: true, confirm: "Remove" }))) return;
            const tiles = m.design_tiles.map(() => 0xf800); setMeta({ ...m, design_tiles: tiles }); await api.setDesignTiles(tiles); touch(); toast("ok", "Ground designs removed");
          }}>Remove all ground designs</Button>
        </section>
      </div>
    );
  }

  function renderDumps() {
    const [ax, ay] = acreSel;
    const setAx = (x: number) => setAcreSel([x, ay]), setAy = (y: number) => setAcreSel([ax, y]);
    const reloadAll = async () => {
      const [m, b] = await Promise.all([api.mapMeta(), api.buildings(), loadLayer(0), loadLayer(1), loadTerrain()]);
      setMeta(m); setBuildings(b); changed(0); changed(1); undo.current = []; redo.current = [];
      if (m.max_elevation > 3) {
        toast("warn", "Terrain above the game limit",
          `The imported terrain reaches elevation ${m.max_elevation}. The game supports 0–3; painting is limited to 0–3.`);
      }
    };
    const rows: { kind: DumpKind; label: string; desc: string; filter: { name: string; extensions: string[] }; name: string; acre?: "field" | "terrain"; perLayer?: boolean }[] = [
      { kind: "field_layer", label: "Field items", desc: `Every item on layer ${layer + 1}`, filter: F.nhl, name: `layer${layer + 1}.nhl`, perLayer: true },
      { kind: "field_acre", label: "Field items (one acre)", desc: "32×32 tiles of the chosen acre", filter: F.nhl, name: `acre-${ax}-${ay}.nhl`, acre: "field", perLayer: true },
      { kind: "field_flags", label: "Item activity flags", desc: `Layer ${layer + 1}`, filter: F.nhlf, name: `layer${layer + 1}.nhlf`, perLayer: true },
      { kind: "terrain", label: "Terrain", desc: "Cliffs, rivers, paths", filter: F.nht, name: "terrain.nht" },
      { kind: "terrain_acre", label: "Terrain (one acre)", desc: "16×16 terrain tiles", filter: F.nht, name: `terrain-${ax}-${ay}.nht`, acre: "terrain" },
      { kind: "buildings", label: "Buildings", desc: "Positions of every building", filter: F.nhb, name: "buildings.nhb" },
      { kind: "acres", label: "Acre templates", desc: "Beaches and outer acres", filter: F.nha, name: "acres.nha" },
      { kind: "designs", label: "Ground designs", desc: "Custom designs on the ground", filter: F.nhmd, name: "designs.nhmd" },
    ];
    return (
      <div className="col" style={{ gap: 12 }}>
        <div className="small faint">Share or back up parts of the island. Files are compatible with the original NHSE.</div>
        <div className="row">
          <Field label="Acre column"><NumberInput value={ax} max={8} onChange={setAx} width={70} /></Field>
          <Field label="Acre row"><NumberInput value={ay} max={6} onChange={setAy} width={70} /></Field>
          <div className="tiny faint" style={{ flex: 1 }}>Used by the one-acre files (counted from the top-left of each layer).</div>
        </div>
        {rows.map((r) => (
          <div key={r.kind} className="row" style={{ padding: "8px 10px", border: "1px solid var(--border)", borderRadius: 10 }}>
            <div style={{ flex: 1, minWidth: 0 }}><div className="bold">{r.label}</div><div className="tiny faint">{r.desc}</div></div>
            <Button size="sm" iconOnly icon={<Download />} title="Export" onClick={async () => {
              const p = await saveFile(`Export ${r.label.toLowerCase()}`, r.name, [r.filter]);
              if (!p) return;
              await syncChain.current;
              try { await api.mapExport(r.kind, p, layer, r.acre ? ax : undefined, r.acre ? ay : undefined); toast("ok", `${r.label} exported`); } catch (e) { toast("error", "Export failed", String(e)); }
            }} />
            <Button size="sm" iconOnly icon={<Upload />} title="Import" onClick={async () => {
              const p = await pickFile(`Import ${r.label.toLowerCase()}`, [r.filter, F.all]);
              if (!p) return;
              await syncChain.current;
              try { await api.mapImport(r.kind, p, layer, r.acre ? ax : undefined, r.acre ? ay : undefined); await reloadAll(); touch(); toast("ok", `${r.label} imported`); } catch (e) { toast("error", "Import failed", String(e)); }
            }} />
          </div>
        ))}
      </div>
    );
  }
}
