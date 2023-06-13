import { cncAxios } from "./cncAxios";
import { ReloadablePromiseResult, useGet } from "./generic";

export type Vec3 = [number, number, number]

export interface SavedPosition {
  label: string,
  position: Vec3
}
export interface Offsets {
  tools: Record<string, Vec3>,
  workpieces: Record<string, Vec3>,
}

export function usePositions(): ReloadablePromiseResult<SavedPosition[]> {
  return useGet("/coords/positions");
}
export function useCoordinates(): ReloadablePromiseResult<Offsets> {
  return useGet("/coords/offsets")
}
export function recordPosition(position: SavedPosition) {
  return cncAxios.post("/coords/positions", position);
}

export type OffsetKind = "Tool" | "Workpiece";

export function setCoordinateOffset(data: {
  name: string,
  offset_kind: OffsetKind,
  offset: Vec3
}) {
  return cncAxios.put("/coords/offsets", data);
}
export function deleteCoordinateOffset(data: {
  name: string,
  offset_kind: OffsetKind
}) {
  return cncAxios.delete("/coords/offsets", {
    data, headers: {
      'Content-Type': 'application/json',
    }
  });
}