import { create } from 'zustand';
import type { Viewport } from '@/types';

/**
 * The camera viewport lives in its own store, separate from the main editor
 * store, so that panning/zooming does NOT re-render the entire panel/menu tree.
 *
 * During a pan the offset changes ~60×/sec; if viewport were part of the main
 * store, every component subscribed to it (which, with whole-store subscriptions,
 * was all of them) would re-render each frame and tank the framerate. The canvas
 * render loop reads this store imperatively via getState() and subscribes to mark
 * itself dirty, so panning touches zero React reconciliation.
 */
interface ViewportState {
  viewport: Viewport;
  setViewport: (viewport: Partial<Viewport>) => void;
  pan: (dx: number, dy: number) => void;
  zoom: (factor: number, centerX: number, centerY: number) => void;
}

export const useViewportStore = create<ViewportState>((set) => ({
  viewport: {
    offsetX: 400,
    offsetY: 200,
    zoom: 1,
  },

  setViewport: (viewport) =>
    set((state) => ({ viewport: { ...state.viewport, ...viewport } })),

  pan: (dx, dy) =>
    set((state) => ({
      viewport: {
        ...state.viewport,
        offsetX: state.viewport.offsetX + dx,
        offsetY: state.viewport.offsetY + dy,
      },
    })),

  zoom: (factor, centerX, centerY) =>
    set((state) => {
      const newZoom = Math.max(0.25, Math.min(4, state.viewport.zoom * factor));
      const zoomRatio = newZoom / state.viewport.zoom;
      // Adjust offset to zoom toward the center point.
      const newOffsetX = centerX - (centerX - state.viewport.offsetX) * zoomRatio;
      const newOffsetY = centerY - (centerY - state.viewport.offsetY) * zoomRatio;
      return {
        viewport: { offsetX: newOffsetX, offsetY: newOffsetY, zoom: newZoom },
      };
    }),
}));
