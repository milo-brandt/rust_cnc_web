import numpy as np
from matplotlib.path import Path
from matplotlib.patches import PathPatch
from matplotlib.collections import PatchCollection
from shapely.geometry import Polygon
import matplotlib.pyplot as plt
import csv
from shapely import wkt
from shapely.validation import make_valid  # Seems we get bowties sometimes. Not sure why?
# This utility here can convert svg -> wkt.
# Closed source but oh well https://mygeodata.cloud/converter/svg-to-wkt
# Can only convert 3 things per month, haha. It's not that hard to do this, though...

def wkts_from_csv(path):
    csv.field_size_limit(1000000000)
    with open(path) as f:
        return [line[0] for line in csv.reader(f)][1:]

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

def as_poly_list(poly):
    if hasattr(poly, 'geoms'):
        return list(poly.geoms)
    elif len(poly.exterior.coords) > 0:
        return [poly]
    else:
        return []

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

def cuttable(*, to_cut, to_avoid, radius):
    # Consider resolution parameter here...
    productive_locations = to_avoid.buffer(radius)
    forbidden_locations = to_cut.buffer(radius)
    return productive_locations.difference(forbidden_locations)

def expand_to_bicuttable(*, to_cut, to_avoid, radius):
    return cuttable(to_cut, cuttable(to_avoid, to_cut, radius), radius)

def refine_to_bicuttable(*, to_cut, to_avoid, radius):
    return cuttable(to_cut, expand_to_bicuttable(to_avoid, to_cut, radius), radius)

def wkt_loads(s):
    return make_valid(wkt.loads(s))

###
# Particular script...
###

# wkt_lines = wkts_from_csv("/home/milo/Documents/Modelling/CuttingBoard/separated.csv")

# # Add a miniscule buffer to make everyone less complain-y
# wkts = [wkt.loads(item) for item in wkt_lines]
# background = wkts[0]
# trunk = wkts[1]
# leaf = wkts[2]
# wind = wkts[3]
# canopy = wkts[4]

# #nonback = leaf.union(canopy).union(trunk).union(wind)
# #show_polys([background, nonback])
# show_polys(wkts)