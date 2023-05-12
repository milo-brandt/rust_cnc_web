from xml.etree import ElementTree
import argparse

parser = argparse.ArgumentParser(description="convert XML to sycamore's style")
parser.add_argument('file', help='the file to convert')
parser.add_argument('--spaces', type=int, help='prefixed spaces on outer level', default=0)
args = parser.parse_args()

def reformat(indent, element):
    attributes = element.attrib
    if len(attributes) > 0:
        def format_item(key, value):
            return f"{key}=\"{ value }\""
        attrib_string = ", ".join(format_item(key, value) for key, value in attributes.items())
        attrib_string = f"({ attrib_string })"
    else:
        attrib_string = ""
    children = list(element)
    if len(children) > 0:
        children_string = "\n".join(reformat(indent + "    ", child) for child in children)
        children_string = f" {{\n{ children_string }\n{ indent }}}"
    else:
        children_string = " {}"
    line = f"{ indent }{ element.tag }{ attrib_string }{ children_string }"
    return line

tree = ElementTree.parse(args.file)

print(reformat(" " * args.spaces, tree.getroot()))