from typing import NamedTuple
from matplotlib.path import Path
from matplotlib.patches import PathPatch
from shapely.geometry.polygon import orient
from shapely.geometry import Polygon, Point, MultiPoint, LineString, MultiPolygon, MultiLineString

###
# Display for debugging...
###

def plot_line_string(ax, line_string, **kwargs):
    ax.plot(*line_string, **kwargs)

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
    elif len(poly.exterior.coords) > 0:
        return [poly]
    else:
        return []

def as_line_string_list(poly):
    return [
        geo
        for geo in as_geo_list(poly)
        if geo.geom_type == "LineString"
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

class OffsetLevels(NamedTuple):
    # Returns levels ordered from smallest to biggest
    levels: "list[list[Polygon]]"

class OffsetGraphNode(NamedTuple):
    line_string: "LineString"
    is_hole: "bool"
    requirements: "list[OffsetGraphNode]"

class OffsetGraph(NamedTuple):
    nodes: "list[OffsetGraphNode]"

def offset_polygon_levels_for(poly, offset_amount):
    # poly can be a polygon or compound
    levels = []
    offset_total = 0
    while True:
        next_level = [
            orient(candidate)
            for candidate as_poly_list(poly.buffer(-offset_total))
        ]
        if len(next_level) > 0:
            ret.append(next_level)
            offset_total += offset_amount
        else:
            return OffsetLevels(levels=reversed(levels))  # Reverse so it goes smallest to biggest.

def offset_graph_for(poly, offset_amount):
    offset_levels = offset_polygon_levels_for(poly, offset_amount)
    total_nodes = []
    prior_nodes = []
    for level in offset_levels.levels:
        next_nodes = []
        for polygon in level:
            next_nodes.append(OffsetGraphNode(
                line_string=polygon.exterior,
                is_hole=False,
                requirements=[
                    node
                    for node in prior_nodes
                    if polygon.contains(Point(*node.line_string.coords[0]))
                ],
            ))
            for hole in polygon.interiors:
                point_in_hole = Point(*hole.coords[0])
                next_nodes.append(OffsetGraphNode(
                    line_string=hole,
                    is_hole=True,
                    requirements=[
                        node
                        for node in prior_nodes
                        if node.is_hole and Polygon(node.line_string.coords).contains(point_in_hole)
                    ],
                ))
        # Sanity check:
        required_prior_nodes = set()
        for node in next_nodes:
            for requirement in node.requirements:
                required_prior_nodes.add(requirement)
        if required_prior_nodes != set(prior_nodes):
            raise RuntimeError("Not all prior nodes were required!")
        # Prepare for next level
        total_nodes += next_nodes
        prior_nodes = total_nodes
    return OffsetGraph(nodes=total_nodes)

# Really, there are two relations: which are safe paths from a given place + which should be done earlier


###
# Utilites for converting offset trees to paths...
###

def cut_line_string(line, distance):
    # Cut a linestring into two line strings at the specified distance from the start.
    # Returns an array of 1 or 2 lines, starting with the one up to the cut if there are 2.
    if distance <= 0.0 or distance >= line.length:
        return [LineString(line)]
    coords = list(line.coords)
    for i, p in enumerate(coords):
        pd = line.project(Point(p))
        if pd == distance:
            return [
                LineString(coords[:i+1]),
                LineString(coords[i:])]
        if pd > distance:
            cp = line.interpolate(distance)
            return [
                LineString(coords[:i] + [(cp.x, cp.y)]),
                LineString([(cp.x, cp.y)] + coords[i:])]
    return [LineString(line)]

def almost_equal(c1, c2):
    return (c1[0] - c2[0])** 2 + (c1[1] - c2[1]) ** 2 < 0.0000000000001

def reposition_line_cycle(line_cycle, distance):
    # Move the start of a line cycle to start instead at a given distance from the current start.
    if not almost_equal(line_cycle.coords[-1], line_cycle.coords[0]):
        raise RuntimeError("Line cycle is not cycle!")
    cut = cut_line_string(line_cycle, distance)
    if len(cut) == 2:
        cut = list(cut[1].coords) + list(cut[0].coords[1:])
    else:
        cut = list(cut[0].coords)
    return cut

def get_index_with_distance_multiline(line_string_list, distance):
    total = 0
    for index, line in enumerate(line_string_list):
        previous_total = total
        total += line.length
        if total >= distance:
            return index, distance - previous_total
    raise RuntimeError("Distance was too great!")

def offset_tree_to_paths(tree):
    final_paths = []
    for child in tree.children:
        final_paths += offset_tree_to_paths(child)
    exteriors = as_geo_list(tree.outer.boundary)
    if len(final_paths) > 0:
        # Find closest point in the most recent path and continue from there
        closest_pt_distance = tree.outer.boundary.project(Point(*final_paths[-1][-1]))
        index, distance = get_index_with_distance_multiline(exteriors, closest_pt_distance)
        split_path = exteriors[index]
        del exteriors[index]
        final_paths[-1] += reposition_line_cycle(split_path, closest_pt_distance)
    for exterior in exteriors:  # exludes anything handled above...
        final_paths = [list(exterior.coords)]
    return final_paths
