/** The cell an arrow key moves to in a grid of `count` cells, `cols` wide. */
export function move(index: number, key: string, cols: number, count: number): number {
  const targets: Record<string, number> = {
    ArrowLeft: index - 1,
    ArrowRight: index + 1,
    ArrowUp: index - cols,
    ArrowDown: index + cols,
  };
  const target = targets[key] ?? index;
  if (target >= 0 && target < count) return target;
  // Down from the row above a shorter last row lands on the last cell.
  const lastRow = Math.floor((count - 1) / cols);
  return key === "ArrowDown" && Math.floor(index / cols) < lastRow ? count - 1 : index;
}
