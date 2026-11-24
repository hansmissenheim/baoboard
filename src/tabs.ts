export type Sticker = { id: string; path: string };
export type Tab = { id: string; name: string; emoji: string; image: string | null; stickers: string[] };

/** The stickers a tab shows, in its order, or all of them for the All tab. */
export function visible(all: Sticker[], tab: Tab | undefined): Sticker[] {
  if (!tab) return all;
  const byId = new Map(all.map((s) => [s.id, s]));
  // A tab can still list stickers deleted since.
  return tab.stickers.flatMap((id) => byId.get(id) ?? []);
}

/** The tab `step` places from `current` among All (null) and `tabs`, wrapping around. */
export function cycle(current: string | null, tabs: Tab[], step: number): string | null {
  const ids = [null, ...tabs.map((t) => t.id)];
  const i = Math.max(ids.indexOf(current), 0);
  return ids[(i + step + ids.length) % ids.length];
}
