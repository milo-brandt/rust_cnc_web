var __spreadArray = (this && this.__spreadArray) || function (to, from, pack) {
    if (pack || arguments.length === 2) for (var i = 0, l = from.length, ar; i < l; i++) {
        if (ar || !(i in from)) {
            if (!ar) ar = Array.prototype.slice.call(from, 0, i);
            ar[i] = from[i];
        }
    }
    return to.concat(ar || Array.prototype.slice.call(from));
};
import { useCallback, useMemo, useReducer, useRef } from "react";
import { flatten, mapValues, max, min, range, sum } from "lodash";
import { Matrix } from "ts-matrix";
import Quaternion from "quaternion";
import { Canvas, contextCached } from "./Canvas";
import React from "react";
var VERTEX_SHADER_CODE = "#version 300 es\n\
uniform mat4x4 transformation;\n\
\n\
in vec3 position;\n\
in float distance;\n\
\n\
out float depth;\n\
out float frag_distance;\n\
\n\
void main() {\n\
    gl_Position = transformation * vec4(position, 1);\n\
    depth = position.z;\n\
    frag_distance = distance;\n\
}";
var FRAMENT_SHADER_CODE = "#version 300 es\n\
precision highp float;\n\
in float depth;\n\
in float frag_distance;\n\
out vec4 outColor;\n\
\n\
uniform float depth_cutoff;\n\
uniform float distance_cutoff;\n\
\n\
void main() {\n\
    outColor = vec4(1, 1, 1, depth < depth_cutoff && frag_distance < distance_cutoff ? 1.0 : 0.1);\n\
}";
function compileShader(context, type, source) {
    var _a;
    var shader = context.createShader(type);
    if (shader === null) {
        throw Error("Failed to create shader.");
    }
    context.shaderSource(shader, source);
    context.compileShader(shader);
    if (!context.getShaderParameter(shader, context.COMPILE_STATUS)) {
        throw Error("Failed to create shader:\n" + ((_a = context.getShaderInfoLog(shader)) !== null && _a !== void 0 ? _a : "Unknown error."));
    }
    return shader;
}
function linkProgram(context, vertexShader, fragmentShader) {
    var _a;
    var program = context.createProgram();
    if (program === null) {
        throw Error("Failed to create program.");
    }
    context.attachShader(program, vertexShader);
    context.attachShader(program, fragmentShader);
    context.linkProgram(program);
    if (!context.getProgramParameter(program, context.LINK_STATUS)) {
        throw Error("Failed to create program:\n" + ((_a = context.getProgramInfoLog(program)) !== null && _a !== void 0 ? _a : "Unknown error."));
    }
    return program;
}
function setupProgram(context) {
    var program = linkProgram(context, compileShader(context, context.VERTEX_SHADER, VERTEX_SHADER_CODE), compileShader(context, context.FRAGMENT_SHADER, FRAMENT_SHADER_CODE));
    return program;
}
function setupContext(context) {
    context.enable(context.BLEND);
    context.blendFunc(context.SRC_ALPHA, context.ONE_MINUS_SRC_ALPHA);
    var program = setupProgram(context);
    var attributeLocations = {
        input: {
            position: context.getAttribLocation(program, "position"),
            distance: context.getAttribLocation(program, "distance"),
        },
        uniform: {
            transformation: context.getUniformLocation(program, "transformation"),
            depthCutoff: context.getUniformLocation(program, "depth_cutoff"),
            distanceCutoff: context.getUniformLocation(program, "distance_cutoff"),
        }
    };
    return {
        activate: function () { context.useProgram(program); },
        setUniforms: function (_a) {
            var transformation = _a.transformation, depthCutoff = _a.depthCutoff, distanceCutoff = _a.distanceCutoff;
            // With the program active, set the uniforms of it.
            context.uniformMatrix4fv(attributeLocations.uniform.transformation, true, flattenMatrix(transformation));
            context.uniform1f(attributeLocations.uniform.depthCutoff, depthCutoff);
            context.uniform1f(attributeLocations.uniform.distanceCutoff, distanceCutoff);
        },
        setAttributeLocations: function (_a) {
            var stride = _a.stride, positionOffset = _a.positionOffset, travelOffset = _a.travelOffset;
            // With the program active and a VAO bound, set the attribute locations.
            context.vertexAttribPointer(attributeLocations.input.position, 3, context.FLOAT, false, stride, positionOffset);
            context.vertexAttribPointer(attributeLocations.input.distance, 1, context.FLOAT, false, stride, travelOffset);
            context.enableVertexAttribArray(attributeLocations.input.position);
            context.enableVertexAttribArray(attributeLocations.input.distance);
        }
    };
}
function boundsOf(points) {
    if (points.length === 0) {
        return {
            min: [-1, -1, -1],
            max: [1, 1, 1],
        };
    }
    else {
        return mapValues({
            min: min,
            max: max
        }, function (fn) { return range(3).map(function (index) { return fn(points.map(function (point) { return point[index]; })); }); });
    }
}
function flattenMatrix(matrix) {
    return flatten(matrix.values);
}
export default function ViewPage(_a) {
    var points = _a.points;
    /*
      
    */
    var pointMetadata = useMemo(function () {
        var bounds = boundsOf(points);
        var pointsWithTravel = points.slice(1).reduce(function (_a, next) {
            var distance = _a.distance, items = _a.items;
            var lastPoint = items[items.length - 1];
            var segmentDistance = Math.sqrt(sum(range(3).map(function (index) { return Math.pow((lastPoint[index] - next[index]), 2); })));
            var nextDistance = distance + segmentDistance;
            items.push(__spreadArray(__spreadArray([], next, true), [nextDistance], false));
            return {
                distance: nextDistance,
                items: items,
            };
        }, { distance: 0, items: [__spreadArray(__spreadArray([], points[0], true), [0], false)] });
        var maxRange = Math.sqrt(sum(range(3).map(function (index) { return Math.pow((bounds.max[index] - bounds.min[index]), 2); })));
        var center = range(3).map(function (index) { return (bounds.min[index] + bounds.max[index]) / 2; });
        return {
            bounds: bounds,
            maxRange: maxRange,
            center: center,
            pointsWithTravel: pointsWithTravel.items,
            totalTravel: pointsWithTravel.distance,
        };
    }, [points]);
    var _b = useReducer(function (previous, next) { return previous * next; }, 1), zoom = _b[0], multiplyZoom = _b[1];
    var _c = useReducer(function (state, change) {
        return change.mul(state).normalize();
    }, new Quaternion()), rotation = _c[0], multiplyRotation = _c[1];
    var dragging = useRef(false);
    var containerRef = useRef(null);
    var getCanvasInfo = useCallback(contextCached(setupContext), []);
    var preparePoints = useCallback(contextCached(function (context) {
        var _a, _b;
        var programInfo = getCanvasInfo(context);
        var buffer = context.createBuffer();
        context.bindBuffer(context.ARRAY_BUFFER, buffer);
        var bufferData = new Float32Array((_b = (_a = pointMetadata === null || pointMetadata === void 0 ? void 0 : pointMetadata.pointsWithTravel) === null || _a === void 0 ? void 0 : _a.flatMap(function (arr) { return arr; })) !== null && _b !== void 0 ? _b : []);
        context.bufferData(context.ARRAY_BUFFER, bufferData, context.STATIC_DRAW);
        var vao = context.createVertexArray();
        context.bindVertexArray(vao);
        programInfo.activate();
        programInfo.setAttributeLocations({
            stride: 4 * 4,
            positionOffset: 0,
            travelOffset: 3 * 4,
        });
        return {
            draw: function () {
                var _a, _b;
                context.bindVertexArray(vao);
                context.bindBuffer(context.ARRAY_BUFFER, buffer);
                context.drawArrays(context.LINE_STRIP, 0, (_b = (_a = pointMetadata === null || pointMetadata === void 0 ? void 0 : pointMetadata.pointsWithTravel) === null || _a === void 0 ? void 0 : _a.length) !== null && _b !== void 0 ? _b : 0);
            }
        };
    }), [pointMetadata]);
    console.log("COMPONENT FUNCTION!", points);
    var setupCanvas = useCallback(function (canvas) {
        console.log("SETUP FUNCTION!");
        canvas.onwheel = function (event) {
            if (event.deltaY > 0) {
                multiplyZoom(0.8);
            }
            else {
                multiplyZoom(1 / 0.8);
            }
        };
        canvas.onmousedown = function (event) {
            if (event.buttons & 1) {
                dragging.current = true;
            }
        };
        canvas.onmousemove = function (event) {
            if (dragging.current && (event.buttons & 1)) {
                var factor = 0.001;
                var change = new Quaternion(0, -event.movementY * factor, -event.movementX * factor, 0).exp();
                multiplyRotation(change);
            }
            else if (dragging.current) {
                // TODO: Mouse up should track globally...
                dragging.current = false;
            }
        };
    }, []);
    var render = useMemo(function () {
        if (points.length === 0) {
            console.log("NO POINTS :(");
            return function () { };
        }
        function render(context, canvas) {
            console.log("RENDER FUNCTION!");
            var programInfo = getCanvasInfo(context);
            programInfo.activate();
            var width = canvas.clientWidth;
            var height = canvas.clientHeight;
            var aspect = width / height;
            canvas.width = width;
            canvas.height = height;
            context.viewport(0, 0, width, height);
            context.clearColor(0.0, 0.0, 0.0, 1.0);
            context.clear(context.COLOR_BUFFER_BIT);
            if (!pointMetadata) {
                return;
            }
            //context.useProgram(canvasInfo.program);
            var center = pointMetadata.center;
            var maxRange = pointMetadata.maxRange;
            //
            var translateAroundZero = new Matrix(4, 4, [
                [1, 0, 0, -center[0]],
                [0, 1, 0, -center[1]],
                [0, 0, 1, -center[2]],
                [0, 0, 0, 1]
            ]);
            var shrinkToUnitSphere = new Matrix(4, 4, [
                [2 / maxRange, 0, 0, 0],
                [0, 2 / maxRange, 0, 0],
                [0, 0, -2 / maxRange, 0],
                [0, 0, 0, 1],
            ]);
            var rotate = new Matrix(4, 4, rotation.toMatrix4(true));
            var translateToOneToThree = new Matrix(4, 4, [
                [1, 0, 0, 0],
                [0, 1, 0, 0],
                [0, 0, 1, 1],
                [0, 0, 0, 1],
            ]);
            var project = new Matrix(4, 4, [
                [zoom / aspect, 0, 0, 0],
                [0, zoom, 0, 0],
                [0, 0, 1, 0],
                [0, 0, 1, 1],
            ]);
            var combined = (project
                .multiply(translateToOneToThree)
                .multiply(rotate)
                .multiply(shrinkToUnitSphere)
                .multiply(translateAroundZero));
            programInfo.setUniforms({
                transformation: combined,
                depthCutoff: (pointMetadata.bounds.max[2]) + 0.001,
                distanceCutoff: pointMetadata.totalTravel,
            });
            preparePoints(context).draw();
        }
        return render;
    }, [points, zoom, rotation, pointMetadata, preparePoints]);
    return React.createElement("div", { ref: containerRef, style: { position: "relative", width: "100%", height: "100%" } },
        React.createElement(Canvas, { render: render, setupCanvas: setupCanvas }));
}
