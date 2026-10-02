type Keys = Pick<KeyboardEvent, "ctrlKey" | "altKey" | "shiftKey" | "metaKey" | "code">;

/**
 * The Tauri accelerator for a key press, such as `Control+Super+KeyB`, or null
 * while only modifiers are down. A global shortcut needs Ctrl, Alt or ⌘, or it
 * would swallow ordinary typing.
 */
export function accelerator(e: Keys): string | null {
  if (/^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/.test(e.code)) return null;
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return null;
  const modifiers = [
    e.ctrlKey && "Control",
    e.altKey && "Alt",
    e.shiftKey && "Shift",
    e.metaKey && "Super",
  ];
  return [...modifiers.filter(Boolean), e.code].join("+");
}
