import numpy as np
from matplotlib.path import Path
from matplotlib.patches import PathPatch
from matplotlib.collections import PatchCollection
from shapely.geometry import Polygon
import matplotlib.pyplot as plt
import json
from shapely import wkt
from shapely.validation import make_valid  # Seems we get bowties sometimes. Not sure why?
from shapely.geometry import GeometryCollection, box
from dataclasses import dataclass

# Plots a Polygon to pyplot `ax`
def plot_polygon(ax, poly, **kwargs):
    path = Path.make_compound_path(
        Path(np.asarray(poly.exterior.coords)[:, :2]),
        *[Path(np.asarray(ring.coords)[:, :2]) for ring in poly.interiors])

    patch = PathPatch(path, **kwargs)
    collection = PatchCollection([patch], **kwargs)
    
    ax.add_collection(collection, autolim=True)
    ax.autoscale_view()
    return collection

colors = [
    'lightblue',
    'blueviolet',
    'darkgreen',
    'darksalmon',
    'mediumpurple',
    'silver',
    'rosybrown',
    'steelblue'
]

def show_wkt(wkt_input):
    fig, ax = plt.subplots()
    polygon = wkt.loads(wkt_input)
    for polygon in as_poly_list(polygon):
        plot_polygon(ax, polygon, facecolor='lightblue', edgecolor='red')
    fig.show()

def as_geo_list(poly):
    if hasattr(poly, 'geoms'):
        return list(poly.geoms)
    elif len(poly.exterior.coords) > 0:
        return [poly]
    else:
        return []

def as_poly_list(poly):
    return [
        geo
        for geo in as_geo_list(poly)
        if geo.geom_type == "Polygon"
    ]

def show_polys(polys):
    fig, ax = plt.subplots()
    for i, geometry in enumerate(polys):
        for polygon in as_poly_list(geometry):
            plot_polygon(ax, polygon, facecolor=colors[i%len(colors)], edgecolor='red')
    fig.show()

def show_wkts(wkt_inputs):
    fig, ax = plt.subplots()
    for i, wkt_input in enumerate(wkt_inputs):
        polygon = wkt.loads(wkt_input)
        for polygon in as_poly_list(polygon):
            plot_polygon(ax, polygon, facecolor=colors[i%len(colors)], edgecolor='red')
    fig.show()

def allowed_cutter_positions(*, to_cut, to_avoid, radius):
    # Consider resolution parameter here...
    productive_locations = to_cut.buffer(radius)
    forbidden_locations = to_avoid.buffer(radius)
    return productive_locations.difference(forbidden_locations)

def cut_avoiding(*, to_cut, to_avoid, radius):
    return allowed_cutter_positions(to_cut=to_cut, to_avoid=to_avoid, radius=radius).buffer(radius)

def wkt_loads(s):
    return make_valid(wkt.loads(s))

def union_of_list(l):
    if len(l) == 0:
        return GeometryCollection()
    total = l[0]
    for item in l[1:]:
        total = total.union(item)
    return total

def to_bicuttable(*, primary, secondary, radius):
    # Return a pair of disjoint radius-cuttable sets close to primary and secondary.
    # Will check that the result contains at least primary + secondary.
    # Prefers to enlarge primary where possible.

    total = primary.union(secondary).buffer(0.001).buffer(-0.001)  # Buffer make sure to merge close lines.
    # First: Calculate what subset of secondary can be cut without hitting primary.
    inner_secondary = cut_avoiding(to_cut=secondary, to_avoid=primary, radius=radius).intersection(total)
    # Then, calculate what of primary can be cut avoiding just the cuttable parts of secondary above.
    # This should have a suitably smooth edge, as it "presses" circles up against the boundary of inner_secondary, which must
    # also have circular arcs where it is convex.
    enlarged_primary = cut_avoiding(to_cut=primary, to_avoid=inner_secondary, radius=radius)

    mating_secondary = cut_avoiding(to_cut=secondary, to_avoid=enlarged_primary.intersection(total), radius=radius)

    if not enlarged_primary.union(mating_secondary).buffer(0.01).contains(total):
        raise RuntimeError(
            "Bicuttable region did not contain the original total region. "
            "This can happen where a single point has many components of both primary and secondary "
            "nearby - for instance, if alternating quadrants were in alternating regions. "
            "This could be developed around if needed by looking at the difference and arbitrarily "
            "adding paths to one or the other through such conflicted points."
        )
    
    return (enlarged_primary, mating_secondary)

def create_chosen_cuts(*, cut_list, radius):
    safe_parts = [
        to_bicuttable(
            primary=cut_list[i],
            secondary=union_of_list(cut_list[i+1:]).buffer(0.001),
            radius=radius
        )[0]
        for i in range(len(cut_list))
    ]
    final_parts = [
        safe_parts[i].difference(union_of_list(safe_parts[:i]))
        for i in range(len(safe_parts))
    ]
    return final_parts




# def cut_sequence(extent, cuts):
    #for i in range(len(cuts)):
        

#     box()

###
# Particular script...
###

wkt_json = json.load(open("../../svg2wkt/practice_plain.json"))
wkts = [wkt_loads(item["wkt"]) for item in wkt_json]

# background = wkts[0]
# trunk = wkts[1]
# leaf = wkts[2]
# wind = wkts[3]
# canopy = wkts[4]

# #nonback = leaf.union(canopy).union(trunk).union(wind)
# #show_polys([background, nonback])
# show_polys(wkts)

radius = 0.025 * 25.4 * 0.5

cuts = create_chosen_cuts(cut_list=wkts, radius=radius)
# show_polys(cuts)
# show_polys([union_of_list(cuts)])
total = union_of_list(cuts)

import gcode_generator

@dataclass
class CutStep:
    step_name: "str"
    tool_radius: "float"
    step_over: "float"
    step_down: "float"
    safety_distance: "float" = 0.0
    feedrate: "float"
    simplification: "float" = 0.1  # mm

@dataclass
class CutSpecification:
    shapes: "list[Polygon or MultiPolygon]"  # first should be the background.
    facing_step: FacingStep
    cut_steps: CutStep



# Full set of info:
# * Shape that is meant to *remain* afterwards.
# * For background: 

def gcode_steps_to_cut_component(*, 
    shape,
    cut_steps,  # ordered from coarsest to finest, probably!
    buffer,
    depth,
    safe_height,
):
    convex_hull = shape.convex_hull
    # The full region to cut a hole into
    negative_extent = convex_hull.buffer(buffer + cut_step[0].safety_distance)
    # The part of the cut to treat carefully; only the coarsest step will run outside of this.
    sensitive_negative_extent = convex_hull(buffer * 0.9)
    # 

    # First: generate the coarse step.
    facing_gcode = gcode_generator.shape_to_gcode(
        shape=shape,
        inset=0,
        stepover=cut_steps[0].step_over,
        z_max=0,
        z_min=0,
        z_step=1,
        safe_height=safe_height,
        feedrate=cut_steps[0].feedrate,
    )
    results = []
    already_cut_region = GeometryCollection([])

    def gcode_for_cut(cut_step, target_region):
        nonlocal already_cut_region
        allowable_positions = target_region.buffer(-cut_step.radius - cut_step.safety_distance)
        excluded_positions = already_cut_region.buffer(-cut_step.radius)  # Places not within cutting distance of something not yet cut!
        cuttable_positions = allowable_positions.buffer(cut_step.radius)
        result = gcode_generator.shape_to_gcode(
            shape=allowable_positions,
            exclusion_region=excluded_positions,
            inset=0,
            stepover=cut_step.step_over,
            z_max=0,
            z_min=-depth,
            z_step=cut_step.step_down,
            safe_height=safe_height,
            feedrate=cut_step.feedrate,
        )
        already_cut_region = already_cut_region.union(cuttable_positions)
        return result
    
    results.append(
        facing_gcode + "\n" + gcode_for_cut(cut_step[0], negative_extent)
    )

    for cut_step in cut_steps[1:]:
        results.append(
            gcode_for_cut()
        )


safety_amount = 25.4/60

for i, cut in enumerate(cuts):
    with open(f"eighth_inch_cut_{i}.nc", 'w') as f:
        f.write(
            gcode_generator.shape_to_gcode(
                shape=total.difference(cut),
                inset=0,
                stepover=25.4/16,
                z_max=0,
                z_min=0,
                z_step=1,
                safe_height=5,
                feedrate=1000,
            ) + "\n" +
            gcode_generator.shape_to_gcode(
                shape=cut.buffer(-safety_amount),
                inset=25.4/16,
                stepover=25.4/16,
                z_max=0,
                z_min=-3,
                z_step=1,
                safe_height=5,
                feedrate=1000,
            )
        )
    with open(f"fourtieth_inch_cut_{i}.nc", 'w') as f:
        already_cut = cut.buffer(-25.4)
        show_polys([
            cut.buffer(-25.4/80),
            cut.buffer(-25.4/16 - safety_amount).buffer(25.4/16 - 25.4/80),
        ])
        f.write(
            gcode_generator.shape_to_gcode(
                shape=cut.buffer(-0.075), # 3 thousandths for interference!
                exclusion_region=cut.buffer(-25.4/16 - safety_amount).buffer(25.4/16 - 0.075),  # What was cut before unbuffered by new tool radius
                inset=25.4/80,
                stepover=25.4/80,
                z_max=0,
                z_min=-3,
                z_step=0.501,
                safe_height=5,
                feedrate=1000,
            )
        )


# final_background, remaining = to_bicuttable(
#     primary=background,
#     secondary=union_of_list(wkts[1:]).buffer(0.001),
#     radius=radius
# )
# show_polys([final_background, remaining])
# final_trunk, remaining = to_bicuttable(
#     primary=trunk,
#     secondary=union_of_list(wkts[2:]).buffer(0.001),
#     radius=radius
# )
# show_polys([final_trunk, remaining])
# final_leaf, remaining = to_bicuttable(
#     primary=leaf,
#     secondary=union_of_list(wkts[3:]).buffer(0.001),
#     radius=radius
# )
# show_polys([final_leaf, remaining])
# final_wind, final_canopy = to_bicuttable(
#     primary=wind,
#     secondary=union_of_list(wkts[4:]).buffer(0.001),
#     radius=radius
# )
# show_polys([final_wind, final_canopy])


# final_trunk = final_trunk.difference(final_background)
# final_leaf = final_leaf.difference(final_background).difference(final_trunk)
# final_wind = final_wind.difference(final_background).difference(final_trunk).difference(final_leaf)
# final_canopy = final_canopy.difference(final_background).difference(final_trunk).difference(final_leaf).difference(final_wind)

# final_pieces = [final_background, final_trunk, final_leaf, final_wind, final_canopy]

# show_polys([final_background, final_trunk, final_leaf, final_wind, final_canopy])

# show_polys([union_of_list(final_pieces).buffer(0.001)])