from typing import NamedTuple
from matplotlib.path import Path
from matplotlib.patches import PathPatch
from shapely.geometry.polygon import orient
from shapely.geometry import Polygon, Point, MultiPoint, LineString, MultiPolygon, MultiLineString, GeometryCollection
import matplotlib.pyplot as plt

###
# Display for debugging...
###

def plot_list_of_points(ax, list_of_points, **kwargs):
    ax.plot([
        pt[0] for pt in list_of_points
    ], [
        pt[1] for pt in list_of_points
    ], **kwargs)

def plot_polygon(ax, poly, **kwargs):
    path = Path.make_compound_path(
        Path(np.asarray(poly.exterior.coords)[:, :2]),
        *[Path(np.asarray(ring.coords)[:, :2]) for ring in poly.interiors])

    patch = PathPatch(path, **kwargs)
    collection = PatchCollection([patch], **kwargs)
    
    ax.add_collection(collection, autolim=True)
    ax.autoscale_view()
    return collection


###
# General utilities
###

def as_geo_list(poly):
    if hasattr(poly, 'geoms'):
        return list(poly.geoms)
    elif not poly.is_empty:
        return [poly]
    else:
        return []

def as_line_string_list(poly):
    return [
        geo
        for geo in as_geo_list(poly)
        if geo.geom_type == "LineString" or geo.geom_type == "LinearRing"
    ]

def as_poly_list(poly):
    return [
        geo
        for geo in as_geo_list(poly)
        if geo.geom_type == "Polygon"
    ]

###
# Utilities for converting a shape to a series of polygons within it.
# Basically cuts up shapes like onions...
###

def offset_polygon_levels_for(shape, offset_amount, min_shape=None) -> "list[Polygon]":
    # Repeatedly compute offsets of shape by offset_amount until the resulting region is 
    # either empty or, if specified, a subset of min_shape.
    levels = []
    offset_total = 0
    while True:
        next_shape = shape.buffer(-offset_total).simplify(0.1)
        next_level = [
            orient(candidate)
            for candidate in as_poly_list(next_shape)
        ]
        if len(next_level) > 0 and (min_shape is None or not min_shape.contains(next_shape)):
            levels.append(next_level)
            offset_total += offset_amount
        else:
            return list(reversed(levels))  # Reverse so it goes smallest to biggest.

# Given...
#   region: The total region around which to move the tool; the boundary will definitely be traversed.
#   offset_amount: how frequently to include inner paths
#   exclusion_region: a region not to move the tool in (e.g. to represen already-cut portions)
#
# Returns a possible sequence of cuts to make. Not very smart - always repositions between successive cuts.
def path_for_shape(shape, offset_amount, exclusion_region=None):
    levels = offset_polygon_levels_for(shape, offset_amount, exclusion_region)
    paths = []
    for level in levels:
        paths += [
            list(line_string.coords)
            for polygon in level
            for line_string in as_line_string_list(
                polygon.boundary.difference(exclusion_region)
                if exclusion_region is not None
                else polygon.boundary
            )
        ]
    return paths

def get_depth_list(z_max, z_min, z_step):
    depth = z_max - z_step
    depth_list = []
    while depth > z_min:
        depth_list.append(depth)
        depth -= z_step
    depth_list.append(z_min)
    return depth_list

def stroke_paths_to_depth(paths, *, z_max, z_min, z_step):
    # Given a list of paths in 2D, create a list of paths in 3D.
    return [
        [(x, y, z) for x, y in path]
        for z in get_depth_list(z_max, z_min, z_step)
        for path in paths
    ]


def plot_paths(list_of_list):
    fig, ax = plt.subplots()
    for line_string in list_of_list:
        plot_list_of_points(ax, line_string)
    fig.show()


def approx_equal(x, y):
    dif = x - y
    return -0.001 < dif and dif < 0.001

def approx_equal_tuples(x, y):
    return all(approx_equal(a, b) for a, b in zip(x, y))

def paths_to_gcode(paths, feedrate, safe_height):
    last_position = (-1434634, -12342345) # Random numbers, hopefully not equal to anything
    code = ""
    for path in paths:
        if len(path) == 0:
            continue
        first_point = path[0]
        if not approx_equal_tuples(last_position, first_point[0:2]):
            code += f"G0 Z{safe_height}\nG0 X{path[0][0]:.2f} Y{path[0][1]:.2f}\n"
        code += "\n".join([f"G1 X{x:.2f} Y{y:.2f} Z{z:.2f} F{feedrate:.2f}" for x,y,z in path])
        code += "\n"
        last_position = path[-1][0:2]
    code += f"G0 Z{safe_height}"
    return code

def shape_to_gcode(*, shape, inset, stepover, z_max, z_min, z_step, safe_height, feedrate, exclusion_region=None):
    paths = path_for_shape(shape.buffer(-inset), stepover, exclusion_region)
    plot_paths(paths)
    return paths_to_gcode(
        paths=stroke_paths_to_depth(paths, z_max=z_max, z_min=z_min, z_step=z_step),
        feedrate=feedrate,
        safe_height=safe_height,
    )


shape = Polygon([(0, 0), (16, 0), (16, 16), (0, 16)])
inner_shape = Polygon([(0.4, 0.4), (0.5, 0.5), (0.6, 0.4)])