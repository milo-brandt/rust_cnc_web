import matplotlib.pyplot as plt
import sys
from shapely import wkt
from matplotlib.path import Path
from matplotlib.patches import PathPatch
import numpy as np
from matplotlib.collections import PatchCollection
import json

fig, ax = plt.subplots()

colors = [
    'lightblue',
    'blueviolet',
    'darkgreen',
    'darksalmon',
    'mediumpurple',
    'silver',
    'rosybrown',
    'steelblue',
    'red',
    'black',
    'yellow',
    'orange',
]
color_index = 0
def next_color():
    global color_index
    result = colors[color_index % len(colors)]
    color_index += 1
    return result

def plot_polygon(poly, **kwargs):
    path = Path.make_compound_path(
        Path(np.asarray(poly.exterior.coords)[:, :2]),
        *[Path(np.asarray(ring.coords)[:, :2]) for ring in poly.interiors])

    patch = PathPatch(path, **kwargs)
    collection = PatchCollection([patch], **kwargs)
    
    ax.add_collection(collection, autolim=True)
    ax.autoscale_view()
    return collection

def plot_line_string(poly, **kwargs):
    ax.plot(*poly.coords.xy, **kwargs)

def as_geo_list(poly):
    if hasattr(poly, 'geoms'):
        return list(poly.geoms)
    elif not poly.is_empty:
        return [poly]
    else:
        return []

def as_poly_list(poly):
    return [
        geo
        for geo in as_geo_list(poly)
        if geo.geom_type == "Polygon"
    ]

def as_line_string_list(poly):
    return [
        geo
        for geo in as_geo_list(poly)
        if geo.geom_type == "LineString" or geo.geom_type == "LinearRing"
    ]

def plot_polygon_wkt(wkt_input):
    polygon = wkt.loads(wkt_input)
    color = next_color()
    for polygon in as_poly_list(polygon):
        plot_polygon(polygon, facecolor=color, edgecolor='red', alpha=0.5)

def plot_line_wkt(wkt_input):
    lines = wkt.loads(wkt_input)
    color = next_color()
    for line in as_line_string_list(lines):
        plot_line_string(line, color=color)

input = json.load(open(sys.argv[1]))

for element in input:
    # print(f"Plotting type { element['type'] } from { element['wkt'] }")
    if element["type"] == "Polygon":
        plot_polygon_wkt(element["wkt"])
    elif element["type"] == "Line":
        plot_line_wkt(element["wkt"])
    else:
        raise RuntimeError(f"Unknown type { element['type'] }")

plt.show()