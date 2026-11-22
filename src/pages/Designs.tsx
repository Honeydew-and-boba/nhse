import { Circle, Disc, Download, Droplet, Eraser, FileDown, FileUp, Heart, ImageDown, ImageUp, Move, PaintBucket, PaintRoller, Palette, Pencil, Pipette, RotateCcw, RotateCw, Slash, Square, Star, UserCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, Card, Check, Field, Loading, MenuButton, Modal, Segmented, Tabs, TextInput } from "../components/ui";
import { api, type DesignKind, type DesignPixels } from "../lib/api";
import { F, pickFile, saveFile } from "../lib/dialog";
import { asset } from "../lib/items";
import * as shapes from "../lib/shapes";
import type { Point, Stamp } from "../lib/shapes";
import { useLoad, useStore } from "../lib/store";
import { MOD } from "../lib/util";

const SHEETS = ["Front", "Back", "Right", "Left"];
const CHECKER = "conic-gradient(#cdcdcd 25%, #ebebeb 0 50%, #cdcdcd 0 75%, #ebebeb 0)";
// In-game stamp diameters are 4 / 10 / 36; L is capped at the 32 px sheet.
const STAMP_RADII = [2, 5, 16];
const BRUSHES = [1, 2, 3];
const checker = (i: number): number[] =>
  ((i % 32) >> 2) % 2 === ((i >> 5) >> 2) % 2 ? [235, 235, 235, 255] : [205, 205, 205, 255];

export default function Designs() {
  const { touch, toast, rev, player, save, confirm } = useStore();
  const [kind, setKind] = useState<DesignKind>("normal");
  const [list, setList, reload] = useLoad(() => api.designs(kind), [kind]);
  const [ver, setVer] = useState(0);
  const [edit, setEdit] = useState<number | null>(null);
  const bump = () => { setVer((v) => v + 1); reload(); touch(); };
  const ext = kind === "normal" || kind === "flag" ? "nhd" : "nhpd";
  return (
    <div className="content"><div className="page wide">
      <Card bodyClass="" actions={null}>
        <div className="row" style={{ padding: "8px 12px 0" }}>
          <Tabs value={kind} onChange={(k) => { setKind(k); setList(null); }} tabs={[
            { value: "normal", label: "My Designs" }, { value: "pro", label: "PRO Designs" }, { value: "tailor", label: "Able Sisters" }, { value: "flag", label: "Island flag" }]} />
          <span className="spacer" />
          <Button size="sm" icon={<UserCheck />} onClick={async () => {
            if (!(await confirm({ title: `Make ${save?.players[player]?.name} the author of every design?`, message: "Lets this resident edit designs that were imported from other islands." }))) return;
            const n = await api.setDesignAuthorAll(kind, player); bump(); toast("ok", `Updated ${n} designs`);
          }}>Claim all</Button>
        </div>
        <div style={{ padding: 16 }}>
          {!list ? <Loading /> : (
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(118px, 1fr))", gap: 14 }}>
              {list.map((d) => (
                <div key={d.index} className="col" style={{ gap: 6, cursor: "default" }} onClick={() => setEdit(d.index)}>
                  <img className="design-thumb" src={asset(`design/${kind}/${d.index}?r=${rev}.${ver}`)} alt="" style={{ opacity: d.empty ? 0.35 : 1 }} />
                  <div className="tiny" style={{ lineHeight: 1.25 }}>
                    <div className="bold ellipsis">{d.name || <span className="faint">(empty)</span>}</div>
                    <div className="faint ellipsis">{d.player_name ? `${d.player_name} · ${d.town_name}` : `#${d.index + 1}`}</div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </Card>
      {edit !== null && <DesignEditor kind={kind} index={edit} ext={ext} onClose={() => setEdit(null)} onSaved={bump} />}
    </div></div>
  );
}

type Tool = "pencil" | "fill" | "fill-all" | "pick" | "erase" | "line" | "rect" | "ellipse" | Stamp | "move";
const isStamp = (t: Tool): t is Stamp => t === "circle" || t === "star" || t === "heart";
const isShape = (t: Tool) => t === "line" || t === "rect" || t === "ellipse" || isStamp(t);
const clampCell = ([x, y]: Point): Point => [Math.min(31, Math.max(0, x)), Math.min(31, Math.max(0, y))];
// The mirror axis runs between columns 15 and 16, as in the game.
const mirrorCol = (k: number) => k - (k % 32) + 31 - (k % 32);

function DesignEditor({ kind, index, ext, onClose, onSaved }: { kind: DesignKind; index: number; ext: string; onClose: () => void; onSaved: () => void }) {
  const { toast } = useStore();
  const [d, setD] = useState<DesignPixels | null>(null);
  const [sheet, setSheet] = useState(0);
  const [color, setColor] = useState(0);
  const [tool, setTool] = useState<Tool>("pencil");
  const [stampSize, setStampSize] = useState(1);
  const [brushSize, setBrushSize] = useState(0);
  const [dirty, setDirty] = useState(false);
  const [grid, setGrid] = useState(true);
  const [mirror, setMirror] = useState(false);
  const [preview, setPreview] = useState<number[] | null>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const painting = useRef(false);
  const stroke = useRef(false);
  const drag = useRef<{ a: Point; b: Point } | null>(null);
  // Whole-array snapshots; `d.pixels` is never mutated in place.
  const hist = useRef<{ past: number[][]; future: number[][] }>({ past: [], future: [] });
  // Move tool: the sheet as it was when the drag started, plus the last applied
  // offset, so every move recomputes from that snapshot instead of compounding.
  const pan = useRef<{ start: Point; base: number[]; dx: number; dy: number } | null>(null);
  // The design as loaded or last saved. "Dirty" means "differs from this", so a
  // no-op edit (undoing every stroke, rotating four times, dragging back) leaves
  // Save disabled instead of offering to rewrite an unchanged design.
  const clean = useRef<DesignPixels | null>(null);
  const load = () => api.designPixels(kind, index).then((x) => {
    setD(x);
    clean.current = x;
    setDirty(false);
    hist.current = { past: [], future: [] };
  });
  useEffect(() => { load(); }, [kind, index]);
  const sheets = d ? d.pixels.length / 1024 : 1;

  useEffect(() => {
    const c = canvas.current;
    if (!c || !d) return;
    const ctx = c.getContext("2d")!;
    const img = ctx.createImageData(32, 32);
    for (let i = 0; i < 1024; i++) {
      const p = d.pixels[sheet * 1024 + i];
      if (p >= 15) img.data.set(checker(i), i * 4);
      else img.data.set([...d.palette[p], 255], i * 4);
    }
    if (preview) for (const i of preview) img.data.set(color < 15 ? [...d.palette[color], 255] : checker(i), i * 4);
    ctx.putImageData(img, 0, 0);
  }, [d, sheet, preview, color]);

  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
      const key = e.key.toLowerCase();
      if (key === "z") { e.preventDefault(); if (e.shiftKey) redo(); else undo(); }
      else if (key === "y") { e.preventDefault(); redo(); }
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [d]);

  if (!d) return null;
  const commit = (next: DesignPixels) => {
    const c = clean.current!;
    setD(next);
    setDirty(next.info.name !== c.info.name || JSON.stringify(next.palette) !== JSON.stringify(c.palette)
      || next.pixels.length !== c.pixels.length || next.pixels.some((p, i) => p !== c.pixels[i]));
  };
  const pushHist = (pixels: number[]) => {
    hist.current.past.push(pixels);
    if (hist.current.past.length > 64) hist.current.past.shift();
    hist.current.future = [];
  };
  const remember = () => {
    if (stroke.current) return;
    stroke.current = true;
    pushHist(d.pixels);
  };
  const undo = () => {
    const prev = hist.current.past.pop();
    if (!prev) return;
    hist.current.future.unshift(d.pixels);
    commit({ ...d, pixels: prev });
  };
  const redo = () => {
    const next = hist.current.future.shift();
    if (!next) return;
    hist.current.past.push(d.pixels);
    commit({ ...d, pixels: next });
  };
  const pos = (e: React.PointerEvent<HTMLCanvasElement>): Point => {
    const r = e.currentTarget.getBoundingClientRect();
    return [Math.floor(((e.clientX - r.left) / r.width) * 32), Math.floor(((e.clientY - r.top) / r.height) * 32)];
  };
  const shapePixels = (a: Point, b: Point): number[] =>
    tool === "line" ? shapes.line(a, b)
      : tool === "rect" ? shapes.rect(a, b)
        : tool === "ellipse" ? shapes.ellipse(a, b)
          : isStamp(tool) ? shapes.stamp(tool, b, STAMP_RADII[stampSize])
            : [];
  const withMirror = (ids: number[]): number[] => mirror ? [...new Set(ids.flatMap((k) => [k, mirrorCol(k)]))] : ids;
  const apply = (x: number, y: number) => {
    if (x < 0 || y < 0 || x > 31 || y > 31) return;
    const base = sheet * 1024;
    const px = d.pixels.slice();
    const i = base + y * 32 + x;
    if (tool === "pick") { setColor(px[i]); setTool("pencil"); return; }
    const v = tool === "erase" ? 15 : color;
    let changed = false;
    // With Mirror on, every painted cell also paints its mirror column.
    const paint = (k: number) => {
      for (const m of mirror ? [k, mirrorCol(k)] : [k]) {
        if (px[base + m] === v) continue;
        px[base + m] = v;
        changed = true;
      }
    };
    if (tool === "fill-all") {
      if (px.slice(base, base + 1024).every((p) => p === v)) return;
      px.fill(v, base, base + 1024);
      changed = true;
    } else if (tool === "fill") {
      const flood = (sx: number, sy: number, target: number) => {
        const stack = [[sx, sy]];
        while (stack.length) {
          const [cx, cy] = stack.pop()!;
          if (cx < 0 || cy < 0 || cx > 31 || cy > 31) continue;
          const k = base + cy * 32 + cx;
          if (px[k] !== target) continue;
          px[k] = v;
          changed = true;
          stack.push([cx + 1, cy], [cx - 1, cy], [cx, cy + 1], [cx, cy - 1]);
        }
      };
      if (px[i] !== v) flood(x, y, px[i]);
      // The mirrored seed fills on its own, so two separate areas both fill.
      if (mirror) {
        const mx = 31 - x, target = px[base + y * 32 + mx];
        if (target !== v) flood(mx, y, target);
      }
    } else {
      // Draw uses a square brush; even sizes grow right and down from the cursor.
      const n = tool === "pencil" ? BRUSHES[brushSize] : 1;
      const o = (n - 1) >> 1;
      for (let dy = 0; dy < n; dy++) {
        for (let dx = 0; dx < n; dx++) {
          const bx = x + dx - o, by = y + dy - o;
          if (bx < 0 || by < 0 || bx > 31 || by > 31) continue;
          paint(by * 32 + bx);
        }
      }
    }
    if (!changed) return;
    remember();
    commit({ ...d, pixels: px });
  };
  // Whole-sheet edits touch only the visible sheet, as in game, and each gesture
  // is one undo step. `snapshot` is the pixel array from before the gesture.
  const writeSheet = (next: number[], snapshot: number[]) => {
    const px = d.pixels.slice();
    px.splice(sheet * 1024, 1024, ...next);
    if (!stroke.current) { stroke.current = true; pushHist(snapshot); }
    commit({ ...d, pixels: px });
  };
  const turn = (cw: boolean) => {
    stroke.current = false;
    writeSheet(shapes.rotate(d.pixels.slice(sheet * 1024, sheet * 1024 + 1024), cw), d.pixels);
  };
  const panTo = (p: Point) => {
    const g = pan.current;
    if (!g) return;
    const dx = p[0] - g.start[0], dy = p[1] - g.start[1];
    if (dx === g.dx && dy === g.dy) return;
    g.dx = dx; g.dy = dy;
    writeSheet(shapes.shift(g.base.slice(sheet * 1024, sheet * 1024 + 1024), dx, dy), g.base);
  };
  // A shape is previewed while dragging, then written as one undo step on release.
  const finishShape = () => {
    const g = drag.current;
    drag.current = null;
    setPreview(null);
    if (!g) return;
    const base = sheet * 1024;
    const px = d.pixels.slice();
    let changed = false;
    for (const i of withMirror(shapePixels(g.a, g.b))) {
      if (px[base + i] !== color) { px[base + i] = color; changed = true; }
    }
    if (!changed) return;
    remember();
    commit({ ...d, pixels: px });
  };
  const down = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    stroke.current = false;
    const p = pos(e);
    if (tool === "move") {
      pan.current = { start: p, base: d.pixels, dx: 0, dy: 0 };
      return;
    }
    if (isShape(tool)) {
      const at = clampCell(p);
      drag.current = { a: at, b: at };
      setPreview(withMirror(shapePixels(at, at)));
      return;
    }
    painting.current = true;
    apply(p[0], p[1]);
  };
  const move = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (pan.current) panTo(pos(e));
    else if (drag.current) {
      drag.current.b = clampCell(pos(e));
      setPreview(withMirror(shapePixels(drag.current.a, drag.current.b)));
    } else if (painting.current && (tool === "pencil" || tool === "erase")) apply(...pos(e));
  };
  const up = () => {
    if (drag.current) finishShape();
    pan.current = null;
    painting.current = false;
  };
  const cancel = () => {
    drag.current = null;
    pan.current = null;
    setPreview(null);
    painting.current = false;
  };
  const save = async () => {
    await api.setDesignPixels(kind, index, d.info.name, d.palette, d.pixels);
    clean.current = d;
    setDirty(false); onSaved(); toast("ok", "Design saved");
  };
  const hexc = (c: number[]) => "#" + c.map((x) => x.toString(16).padStart(2, "0")).join("");
  return (
    <Modal title={d.info.name || `Design ${index + 1}`} size="wide" onClose={onClose} icon={<Palette size={18} />} footer={<>
      <MenuButton label="Import / export" items={[
        { label: "Import PNG…", icon: <ImageUp />, onClick: async () => { const p = await pickFile("Import image", [F.png]); if (p) { try { await api.importDesignPng(kind, index, p); await load(); onSaved(); toast("ok", "Image imported", "Colors were reduced to a 15-color palette."); } catch (e) { toast("error", "Import failed", String(e)); } } } },
        { label: "Export PNG…", icon: <ImageDown />, onClick: async () => { const p = await saveFile("Export image", `${d.info.name || "design"}.png`, [F.png]); if (p) { await api.designPng(kind, index, p, 16); toast("ok", "Exported"); } } },
        "sep",
        { label: `Import .${ext}…`, icon: <FileUp />, onClick: async () => { const p = await pickFile("Import design", [ext === "nhd" ? F.nhd : F.nhpd, F.all]); if (p) { try { await api.importDesign(kind, index, p, true); await load(); onSaved(); } catch (e) { toast("error", "Import failed", String(e)); } } } },
        { label: `Export .${ext}…`, icon: <FileDown />, onClick: async () => { const p = await saveFile("Export design", `${d.info.name || "design"}.${ext}`, [ext === "nhd" ? F.nhd : F.nhpd]); if (p) { await api.exportDesign(kind, index, p); toast("ok", "Exported"); } } },
      ]} icon={<Download />} />
      <span className="spacer" />
      <Button onClick={onClose}>Close</Button>
      <Button variant="primary" disabled={!dirty} onClick={save}>Save design</Button>
    </>}>
      <div className="grid" style={{ gridTemplateColumns: "auto 1fr", gap: 20, alignItems: "start" }}>
        <div className="col" style={{ gap: 10 }}>
          {sheets > 1 && <Segmented value={sheet} onChange={setSheet} options={SHEETS.map((l, i) => ({ value: i, label: l }))} />}
          <div style={{ position: "relative", width: 448, height: 448, borderRadius: 12, border: "1px solid var(--border)", boxSizing: "content-box" }}>
            <canvas ref={canvas} width={32} height={32} className="pixel" style={{ width: "100%", height: "100%", display: "block", borderRadius: 12, cursor: tool === "move" ? "grab" : "crosshair" }}
              onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={cancel} />
            {grid && <div aria-hidden style={{
              position: "absolute", inset: 0, borderRadius: 12, pointerEvents: "none",
              backgroundImage: "linear-gradient(to right, rgba(0,0,0,0.22) 1px, transparent 1px), linear-gradient(to bottom, rgba(0,0,0,0.22) 1px, transparent 1px)",
              backgroundSize: "3.125% 3.125%",
            }} />}
          </div>
          <div className="row" style={{ gap: 14 }}>
            <Check checked={grid} onChange={setGrid} label="Grid" title="Show the boundary of every pixel" />
            <Check checked={mirror} onChange={setMirror} label="Mirror" title="Also paint the mirrored column" />
            <span className="spacer" />
            <Button size="sm" icon={<RotateCcw />} title="Rotate this sheet 90° counter-clockwise" onClick={() => turn(false)}>Rotate</Button>
            <Button size="sm" icon={<RotateCw />} title="Rotate this sheet 90° clockwise" onClick={() => turn(true)} />
          </div>
        </div>
        <div className="col" style={{ gap: 14 }}>
          <Field label="Name" hint="Up to 20 characters"><TextInput value={d.info.name} maxLength={20} onChange={(v) => { commit({ ...d, info: { ...d.info, name: v } }); }} /></Field>
          <Field label="Tool" hint={`${MOD}+Z undo · ${MOD}+Shift+Z redo`}>
            <div className="col" style={{ gap: 8 }}>
              <Segmented value={tool} onChange={(t) => { setTool(t); setPreview(null); }} options={[
                { value: "pencil", label: "Draw", icon: <Pencil /> }, { value: "fill", label: "Fill", icon: <PaintBucket /> },
                { value: "fill-all", label: "Fill all", icon: <PaintRoller /> }, { value: "pick", label: "Pick", icon: <Pipette /> },
                { value: "erase", label: "Erase", icon: <Eraser /> }]} />
              <Segmented value={tool} onChange={(t) => { setTool(t); setPreview(null); }} options={[
                { value: "line", label: "Line", icon: <Slash /> }, { value: "rect", label: "Rect", icon: <Square /> },
                { value: "ellipse", label: "Ellipse", icon: <Circle /> }, { value: "move", label: "Move", icon: <Move /> }]} />
              <Segmented value={tool} onChange={(t) => { setTool(t); setPreview(null); }} options={[
                { value: "circle", label: "Circle", icon: <Disc /> }, { value: "star", label: "Star", icon: <Star /> },
                { value: "heart", label: "Heart", icon: <Heart /> }]} />
            </div>
          </Field>
          {tool === "pencil" && (
            <Field label="Brush size" hint="1×1 · 2×2 · 3×3">
              <Segmented value={brushSize} onChange={setBrushSize} options={[
                { value: 0, label: "S" }, { value: 1, label: "M" }, { value: 2, label: "L" }]} />
            </Field>
          )}
          {isStamp(tool) && (
            <Field label="Stamp size" hint="Filled with the selected color; drag to place">
              <Segmented value={stampSize} onChange={setStampSize} options={[
                { value: 0, label: "S" }, { value: 1, label: "M" }, { value: 2, label: "L" }]} />
            </Field>
          )}
          <Field label={<><Droplet size={13} /> Palette</>} hint="Click to select · double-click the swatch below to change its color">
            <div className="row" style={{ gap: 6, alignItems: "center" }}>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 36px)", gap: 6 }}>
                {d.palette.map((c, i) => (
                  <button key={i} onClick={() => { setColor(i); if (tool === "erase" || tool === "pick") setTool("pencil"); }}
                    style={{ width: 36, height: 36, borderRadius: 9, background: hexc(c), border: i === color ? "3px solid var(--accent)" : "1px solid var(--border-strong)", boxShadow: i === color ? "0 0 0 3px var(--accent-soft)" : undefined }} />
                ))}
              </div>
              <button title="Transparent" aria-label="Transparent" onClick={() => { setColor(15); if (tool === "erase" || tool === "pick") setTool("pencil"); }}
                style={{ width: 36, height: 36, borderRadius: 9, backgroundImage: CHECKER, backgroundSize: "12px 12px", border: color === 15 ? "3px solid var(--accent)" : "1px solid var(--border-strong)", boxShadow: color === 15 ? "0 0 0 3px var(--accent-soft)" : undefined }} />
            </div>
          </Field>
          <div className="row">
            {color < 15 ? (
              <input type="color" value={hexc(d.palette[color])} style={{ width: 48, height: 32, border: 0, background: "none" }}
                onChange={(e) => {
                  const v = e.target.value; const rgb: [number, number, number] = [1, 3, 5].map((k) => parseInt(v.slice(k, k + 2), 16)) as [number, number, number];
                  const pal = d.palette.slice(); pal[color] = rgb; commit({ ...d, palette: pal });
                }} />
            ) : (
              <span style={{ width: 48, height: 32, borderRadius: 6, backgroundImage: CHECKER, backgroundSize: "12px 12px", border: "1px solid var(--border-strong)" }} />
            )}
            <span className="small muted">{color < 15 ? `Color ${color + 1} · ${hexc(d.palette[color]).toUpperCase()}` : "Transparent · erases pixels"}</span>
          </div>
          <div className="tiny faint">Author: {d.info.player_name || "—"} ({d.info.town_name || "—"})</div>
        </div>
      </div>
    </Modal>
  );
}
