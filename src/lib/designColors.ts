// ACNH design-editor colour space: 30 hues x 15 vividness x 15 brightness.
//
// Measured from the game with a save dump (docs/designs.md). The 30 hue anchors are the
// game's exact values, byte for byte; vividness and brightness are generated from the
// measured ramps and land within 1/255 of every measured sample.
const ANCHORS = (
  "e50f0f e53a0f e5640f e58f0f e5ba0f e5e50f bae50f 8fe50f 64e50f 3ae50f 0fe50f " +
  "0fe53a 0fe564 0fe58f 0fe5ba 0fe5e5 0fbae5 0f8fe5 0f64e5 0f3ae5 0f0fe5 3a0fe5 " +
  "640fe5 8f0fe5 ba0fe5 e50fe5 e50fba e50f8f e50f64 e50f3a"
).split(" ");

export const VIVID_COUNT = 15;
export const BRIGHT_COUNT = 15;

/** The 30 hue swatches at full vividness and brightness, in the game's strip order. */
export const HUE_COLORS = ANCHORS.map((a) => "#" + a);

const rgb = (hex: string) => [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16));
const byte = (x: number) => Math.min(255, Math.max(0, Math.round(x))).toString(16).padStart(2, "0");
// Brightness scales every channel: 229 at full brightness, 20 at the darkest step.
const BRIGHTNESS = Array.from({ length: BRIGHT_COUNT }, (_, b) => (20 + (209 * b) / (BRIGHT_COUNT - 1)) / 229);

// Hue-major, so one index reaches any colour: h * 225 + v * 15 + b.
const COLORS = ANCHORS.flatMap((anchor) => {
  const a = rgb(anchor);
  const hi = Math.max(...a); // vividness lerps every channel up toward the hue's brightest one
  return Array.from({ length: VIVID_COUNT }, (_, v) =>
    Array.from({ length: BRIGHT_COUNT }, (_, b) =>
      "#" + a.map((c) => byte((c + (hi - c) * (1 - v / (VIVID_COUNT - 1))) * BRIGHTNESS[b])).join(""))).flat();
});

// First indices for each colour; the near-grey steps repeat across hues.
const INDEX = new Map<string, [number, number, number]>();
for (let i = 0; i < COLORS.length; i++) {
  const c = COLORS[i];
  if (!INDEX.has(c)) INDEX.set(c, [Math.floor(i / (VIVID_COUNT * BRIGHT_COUNT)), Math.floor(i / BRIGHT_COUNT) % VIVID_COUNT, i % BRIGHT_COUNT]);
}

/** Colour at a game hue / vividness / brightness index, as `#rrggbb`. */
export const gameColor = (h: number, v: number, b: number) => COLORS[h * VIVID_COUNT * BRIGHT_COUNT + v * BRIGHT_COUNT + b];

/** Game indices of a colour, or null when it is not a colour the game can make. */
export const gameIndex = (hex: string) => INDEX.get(hex.toLowerCase()) ?? null;
