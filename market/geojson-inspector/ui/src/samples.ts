/** 覆盖验收场景的内置样例：七种几何、空值、高程、反序、未闭合环、跨反经线 bbox、旧式 CRS */
export const SAMPLES: Record<string, string> = {
  valid: `{
  "type": "FeatureCollection",
  "bbox": [100.0, 0.0, 103.0, 1.0],
  "features": [
    {"type": "Feature", "properties": {"name": "a"}, "geometry": {"type": "Point", "coordinates": [102.0, 0.5]}},
    {"type": "Feature", "properties": null, "geometry": null}
  ]
}`,
  seven: `{
  "type": "GeometryCollection",
  "geometries": [
    {"type": "Point", "coordinates": [1.0, 2.0]},
    {"type": "MultiPoint", "coordinates": [[1.0, 2.0], [3.0, 4.0]]},
    {"type": "LineString", "coordinates": [[1.0, 2.0], [3.0, 4.0]]},
    {"type": "MultiLineString", "coordinates": [[[1.0, 2.0], [3.0, 4.0]]]},
    {"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]},
    {"type": "MultiPolygon", "coordinates": [[[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]]},
    {"type": "GeometryCollection", "geometries": [{"type": "Point", "coordinates": [1.0, 2.0]}]}
  ]
}`,
  elevation: `{
  "type": "FeatureCollection",
  "features": [
    {"type": "Feature", "properties": null, "geometry": {"type": "Point", "coordinates": [102.0, 0.5, 30.5]}},
    {"type": "Feature", "properties": null, "geometry": {"type": "LineString", "coordinates": [[102.0, 0.0, 10.0], [103.0, 1.0, 12.5]]}}
  ]
}`,
  swapped: `{
  "type": "FeatureCollection",
  "features": [
    {"type": "Feature", "properties": {"city": "Shanghai"}, "geometry": {"type": "Point", "coordinates": [31.2304, 121.4737]}}
  ]
}`,
  unclosed: `{
  "type": "FeatureCollection",
  "features": [
    {"type": "Feature", "properties": null, "geometry": {"type": "Polygon", "coordinates": [
      [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 1.0]],
      [[0.1, 0.1], [0.1, 0.2]]
    ]}}
  ]
}`,
  antimeridian: `{
  "type": "FeatureCollection",
  "bbox": [175.0, 0.0, -175.0, 2.0],
  "features": [
    {"type": "Feature", "properties": null, "geometry": {"type": "Point", "coordinates": [179.5, 1.0]}}
  ]
}`,
  legacyCrs: `{
  "type": "Feature",
  "crs": {"type": "name", "properties": {"name": "EPSG:3857"}},
  "properties": null,
  "geometry": {"type": "Point", "coordinates": [1.0, 2.0]}
}`,
  emptyCollection: `{"type": "FeatureCollection", "features": []}`,
}
