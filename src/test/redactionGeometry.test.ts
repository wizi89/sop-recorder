import { describe, it, expect } from "vitest";
import { containRect, toDisplayRect, toImageBox } from "../lib/redactionGeometry";

describe("redaction geometry", () => {
  it("letterboxes a wide image vertically", () => {
    // 1920x1080 in a 400x400 element: scale 400/1920, height 225, top 87.5.
    expect(containRect(400, 400, 1920, 1080)).toEqual({
      left: 0,
      top: 87.5,
      width: 400,
      height: 225,
    });
  });

  it("letterboxes a tall image horizontally", () => {
    const r = containRect(400, 200, 100, 200);
    expect(r).toEqual({ left: 150, top: 0, width: 100, height: 200 });
  });

  it("maps a drag into image pixels through the letterbox", () => {
    const content = containRect(400, 200, 100, 200); // scale 1, left 150
    expect(toImageBox({ x: 160, y: 10 }, { x: 170, y: 30 }, content, 100, 200)).toEqual([
      10, 10, 10, 20,
    ]);
    // Dragged backwards gives the same box.
    expect(toImageBox({ x: 170, y: 30 }, { x: 160, y: 10 }, content, 100, 200)).toEqual([
      10, 10, 10, 20,
    ]);
  });

  it("rounds outward at a fractional scale", () => {
    // 1000 image pixels on 300 display pixels: scale 10/3.
    const content = containRect(300, 300, 1000, 1000);
    const box = toImageBox({ x: 10.1, y: 10.1 }, { x: 20.2, y: 20.2 }, content, 1000, 1000)!;
    // 10.1 * 10/3 = 33.67 -> 33; 20.2 * 10/3 = 67.33 -> 68.
    expect(box).toEqual([33, 33, 35, 35]);
    const shown = toDisplayRect(box, content, 1000, 1000);
    expect(shown.left).toBeLessThanOrEqual(10.1);
    expect(shown.left + shown.width).toBeGreaterThanOrEqual(20.2);
  });

  it("clips to the image and ignores a drag entirely in the letterbox", () => {
    const content = containRect(400, 200, 100, 200);
    expect(toImageBox({ x: 100, y: -10 }, { x: 160, y: 5 }, content, 100, 200)).toEqual([
      0, 0, 10, 5,
    ]);
    expect(toImageBox({ x: 10, y: 10 }, { x: 100, y: 100 }, content, 100, 200)).toBeNull();
  });

  it("round-trips a stored box back to the same place", () => {
    const content = containRect(800, 450, 1920, 1080);
    const r = toDisplayRect([960, 540, 192, 108], content, 1920, 1080);
    expect(r).toEqual({ left: 400, top: 225, width: 80, height: 45 });
  });
});
