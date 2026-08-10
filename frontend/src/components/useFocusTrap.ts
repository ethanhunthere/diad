import { useEffect, type RefObject } from "react";

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusable(node: HTMLElement): HTMLElement[] {
  return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (n) => n.getClientRects().length > 0,
  );
}

/**
 * F-16 (a11y spec 2.23) — modal dialog accessibility:
 *  - traps Tab / Shift+Tab inside the dialog so focus never escapes
 *  - routes Escape to `onEscape` (capture phase, so a stacked dialog's
 *    own handlers can't double-fire)
 *  - restores focus to the previously focused element on close
 *
 * `active` gates the effect so callers can keep the hook unconditional
 * while the modal returns null between requests.
 */
export function useFocusTrap(
  ref: RefObject<HTMLElement | null>,
  opts: { active: boolean; onEscape?: () => void },
) {
  useEffect(() => {
    if (!opts.active) return;
    const node = ref.current;
    if (!node) return;
    const prev = document.activeElement as HTMLElement | null;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        opts.onEscape?.();
        return;
      }
      if (e.key !== "Tab") return;
      const els = focusable(node);
      if (els.length === 0) return;
      const first = els[0];
      const last = els[els.length - 1];
      const cur = document.activeElement;
      if (e.shiftKey) {
        if (cur === first || !node.contains(cur)) {
          e.preventDefault();
          last.focus();
        }
      } else if (cur === last || !node.contains(cur)) {
        e.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("keydown", onKeyDown, true);
      prev?.focus?.();
    };
  }, [opts.active, opts.onEscape, ref]);
}
