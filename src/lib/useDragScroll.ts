import { useEffect, useRef } from "react";

/**
 * Enables drag-to-scroll on an element.
 * Only activates when the drag starts on a non-interactive area
 * (not buttons, links, inputs, switches, etc.).
 */
export function useDragScroll<T extends HTMLElement>() {
  const ref = useRef<T>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    let isDown = false;
    let startX = 0;
    let startY = 0;
    let scrollLeft = 0;
    let scrollTop = 0;
    let hasMoved = false;

    const isInteractive = (target: EventTarget | null): boolean => {
      if (!target || !(target instanceof HTMLElement)) return false;
      return target.closest(
        'button, a, input, select, textarea, [role="switch"], [role="checkbox"], [role="menuitem"], label, .touch-none'
      ) !== null;
    };

    const onMouseDown = (e: MouseEvent) => {
      if (isInteractive(e.target)) return;
      isDown = true;
      hasMoved = false;
      el.style.cursor = "grabbing";
      el.style.userSelect = "none";
      startX = e.pageX - el.offsetLeft;
      startY = e.pageY - el.offsetTop;
      scrollLeft = el.scrollLeft;
      scrollTop = el.scrollTop;
    };

    const onMouseMove = (e: MouseEvent) => {
      if (!isDown) return;
      e.preventDefault();
      const x = e.pageX - el.offsetLeft;
      const y = e.pageY - el.offsetTop;
      const walkX = x - startX;
      const walkY = y - startY;
      if (Math.abs(walkX) > 3 || Math.abs(walkY) > 3) {
        hasMoved = true;
      }
      el.scrollLeft = scrollLeft - walkX;
      el.scrollTop = scrollTop - walkY;
    };

    const onMouseUp = () => {
      if (!isDown) return;
      isDown = false;
      el.style.cursor = "";
      el.style.userSelect = "";
      if (hasMoved) {
        // Suppress the next click if we were dragging
        const suppressClick = (e: MouseEvent) => {
          e.preventDefault();
          e.stopPropagation();
          el.removeEventListener("click", suppressClick, true);
        };
        el.addEventListener("click", suppressClick, true);
      }
    };

    const onMouseLeave = () => {
      if (!isDown) return;
      isDown = false;
      el.style.cursor = "";
      el.style.userSelect = "";
    };

    el.addEventListener("mousedown", onMouseDown);
    el.addEventListener("mousemove", onMouseMove);
    el.addEventListener("mouseup", onMouseUp);
    el.addEventListener("mouseleave", onMouseLeave);

    return () => {
      el.removeEventListener("mousedown", onMouseDown);
      el.removeEventListener("mousemove", onMouseMove);
      el.removeEventListener("mouseup", onMouseUp);
      el.removeEventListener("mouseleave", onMouseLeave);
    };
  }, []);

  return ref;
}
