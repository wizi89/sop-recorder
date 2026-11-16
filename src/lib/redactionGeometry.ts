/**
 * Mapping between the review screen and saved-image pixels (review-redaction
 * D4). Pure, so it is tested without layout. Boxes are stored in the saved
 * image's own pixels, never in display pixels, so nothing about a box depends
 * on window size.
 */

/** `[x, y, width, height]` in saved-image pixels. */
export type PixelBox = [number, number, number, number];

/** Where the image's content sits inside its element, in element pixels. */
export interface ContentRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * The rectangle `object-fit: contain` draws the image into: the element box
 * minus the letterbox on the long side.
 */
export function containRect(
  elementWidth: number,
  elementHeight: number,
  naturalWidth: number,
  naturalHeight: number,
): ContentRect {
  if (naturalWidth <= 0 || naturalHeight <= 0 || elementWidth <= 0 || elementHeight <= 0) {
    return { left: 0, top: 0, width: 0, height: 0 };
  }
  const scale = Math.min(elementWidth / naturalWidth, elementHeight / naturalHeight);
  const width = naturalWidth * scale;
  const height = naturalHeight * scale;
  return {
    left: (elementWidth - width) / 2,
    top: (elementHeight - height) / 2,
    width,
    height,
  };
}

/**
 * A box dragged between two points (element pixels) as saved-image pixels,
 * clipped to the image. Rounded outward -- floor on the left and top edge,
 * ceil on the right and bottom -- so the stored box covers at least every
 * pixel the drawn one did. `null` when nothing of the image is covered.
 */
export function toImageBox(
  a: { x: number; y: number },
  b: { x: number; y: number },
  content: ContentRect,
  naturalWidth: number,
  naturalHeight: number,
): PixelBox | null {
  if (content.width <= 0 || content.height <= 0) return null;
  const sx = naturalWidth / content.width;
  const sy = naturalHeight / content.height;
  const toX = (px: number) => (px - content.left) * sx;
  const toY = (py: number) => (py - content.top) * sy;
  const x0 = Math.max(0, Math.floor(toX(Math.min(a.x, b.x))));
  const y0 = Math.max(0, Math.floor(toY(Math.min(a.y, b.y))));
  const x1 = Math.min(naturalWidth, Math.ceil(toX(Math.max(a.x, b.x))));
  const y1 = Math.min(naturalHeight, Math.ceil(toY(Math.max(a.y, b.y))));
  if (x1 <= x0 || y1 <= y0) return null;
  return [x0, y0, x1 - x0, y1 - y0];
}

/** A stored box back in element pixels, for drawing it over the image. */
export function toDisplayRect(
  box: PixelBox,
  content: ContentRect,
  naturalWidth: number,
  naturalHeight: number,
): ContentRect {
  const sx = naturalWidth > 0 ? content.width / naturalWidth : 0;
  const sy = naturalHeight > 0 ? content.height / naturalHeight : 0;
  return {
    left: content.left + box[0] * sx,
    top: content.top + box[1] * sy,
    width: box[2] * sx,
    height: box[3] * sy,
  };
}

/** Too small to have been meant: a click rather than a drag. */
export function isClick(a: { x: number; y: number }, b: { x: number; y: number }): boolean {
  return Math.abs(a.x - b.x) < 3 && Math.abs(a.y - b.y) < 3;
}
