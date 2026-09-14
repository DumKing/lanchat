// Change only the SVG camera, preserving tile coordinates and hit detection.
export function setupBoardCamera(svg) {
  const min = 0.6, max = 2.2;
  let zoom = 1, cx = 540, cy = 380, drag = null, suppressClick = false;
  const out = document.getElementById('zoom-out');
  const into = document.getElementById('zoom-in');
  const value = document.getElementById('zoom-value');
  function paint() {
    const width = 1080 / zoom, height = 760 / zoom;
    cx = Math.max(0, Math.min(1080, cx));
    cy = Math.max(0, Math.min(760, cy));
    svg.setAttribute('viewBox', `${cx - width / 2} ${cy - height / 2} ${width} ${height}`);
    value.textContent = `${Math.round(zoom * 100)}%`;
    out.disabled = zoom <= min;
    into.disabled = zoom >= max;
  }
  function setZoom(next, anchor = { x: cx, y: cy }) {
    next = Math.max(min, Math.min(max, next));
    const ratio = zoom / next;
    cx = anchor.x + (cx - anchor.x) * ratio;
    cy = anchor.y + (cy - anchor.y) * ratio;
    zoom = next;
    paint();
  }
  function reset() { zoom = 1; cx = 540; cy = 380; paint(); }
  out.addEventListener('click', () => setZoom(Math.round((zoom - 0.1) * 100) / 100));
  into.addEventListener('click', () => setZoom(Math.round((zoom + 0.1) * 100) / 100));
  document.getElementById('zoom-reset').addEventListener('click', reset);
  svg.addEventListener('wheel', event => {
    // Leave browser page zoom shortcuts intact.
    if (event.ctrlKey || event.metaKey || event.deltaY === 0) return;
    const matrix = svg.getScreenCTM();
    if (!matrix) return;
    event.preventDefault();
    const anchor = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
    const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? svg.clientHeight : 1);
    setZoom(zoom * Math.exp(-Math.max(-150, Math.min(150, delta)) * 0.002), anchor);
  }, { passive: false });
  svg.addEventListener('pointerdown', event => {
    if (event.button !== 0 || !event.isPrimary) return;
    suppressClick = false;
    drag = { id: event.pointerId, x: event.clientX, y: event.clientY, cx, cy, matrix: svg.getScreenCTM()?.inverse(), moved: false };
  });
  svg.addEventListener('pointermove', event => {
    if (!drag || event.pointerId !== drag.id || !drag.matrix) return;
    if (!drag.moved && Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < 5) return;
    if (!drag.moved) {
      drag.moved = true;
      svg.setPointerCapture(event.pointerId);
      svg.classList.add('dragging');
    }
    const start = new DOMPoint(drag.x, drag.y).matrixTransform(drag.matrix);
    const end = new DOMPoint(event.clientX, event.clientY).matrixTransform(drag.matrix);
    cx = drag.cx + start.x - end.x;
    cy = drag.cy + start.y - end.y;
    paint();
  });
  function endDrag(event) {
    if (!drag || event.pointerId !== drag.id) return;
    suppressClick = drag.moved;
    drag = null;
    svg.classList.remove('dragging');
    if (svg.hasPointerCapture(event.pointerId)) svg.releasePointerCapture(event.pointerId);
  }
  svg.addEventListener('pointerup', endDrag);
  svg.addEventListener('pointercancel', endDrag);
  svg.addEventListener('lostpointercapture', endDrag);
  svg.addEventListener('pointerleave', event => { if (drag && !drag.moved) endDrag(event); });
  svg.addEventListener('click', event => {
    if (suppressClick) { suppressClick = false; event.preventDefault(); event.stopImmediatePropagation(); }
  }, true);
  paint();
  return { reset };
}
