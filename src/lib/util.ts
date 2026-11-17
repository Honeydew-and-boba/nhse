/** Shortcut modifier shown in hints: ⌘ on macOS, Ctrl on Windows. */
export const MOD = navigator.userAgent.includes("Windows") ? "Ctrl" : "⌘";
export const last = <T,>(a: readonly T[] | undefined | null): T | undefined => (a && a.length ? a[a.length - 1] : undefined);
export const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
