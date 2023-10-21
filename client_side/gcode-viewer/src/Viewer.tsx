import { useCallback, useMemo, useReducer, useRef } from "react";
import { flatten, mapValues, max, min, range, sum } from "lodash";
import { Matrix } from "ts-matrix";
import Quaternion from "quaternion";
import { Canvas, contextCached } from "./Canvas";
import React from "react";

const VERTEX_SHADER_CODE = "#version 300 es\n\
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
}"

const FRAMENT_SHADER_CODE = "#version 300 es\n\
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
}"

function compileShader(context: WebGL2RenderingContext, type: number, source: string) {
  const shader = context.createShader(type);
  if (shader === null) {
    throw Error("Failed to create shader.");
  }
  context.shaderSource(shader, source);
  context.compileShader(shader);
  if (!context.getShaderParameter(shader, context.COMPILE_STATUS)) {
    throw Error("Failed to create shader:\n" + (context.getShaderInfoLog(shader) ?? "Unknown error."))    
  }
  return shader;
}
function linkProgram(context: WebGLRenderingContext, vertexShader: WebGLShader, fragmentShader: WebGLShader) {
  const program = context.createProgram();
  if (program === null) {
    throw Error("Failed to create program.");
  }
  context.attachShader(program, vertexShader);
  context.attachShader(program, fragmentShader);
  context.linkProgram(program);
  if(!context.getProgramParameter(program, context.LINK_STATUS)) {
    throw Error("Failed to create program:\n" + (context.getProgramInfoLog(program) ?? "Unknown error."))
  }
  return program;
}
function setupProgram(context: WebGL2RenderingContext) {
  const program = linkProgram(context,
    compileShader(context, context.VERTEX_SHADER, VERTEX_SHADER_CODE),
    compileShader(context, context.FRAGMENT_SHADER, FRAMENT_SHADER_CODE)  
  );
  return program
}
function setupContext(context: WebGL2RenderingContext) {
  context.enable(context.BLEND);
  context.blendFunc(context.SRC_ALPHA, context.ONE_MINUS_SRC_ALPHA)
  const program = setupProgram(context);
  const attributeLocations = {
    input: {
      position: context.getAttribLocation(program, "position")!,
      distance: context.getAttribLocation(program, "distance")!,
    },
    uniform: {
      transformation: context.getUniformLocation(program, "transformation")!,
      depthCutoff: context.getUniformLocation(program, "depth_cutoff")!,
      distanceCutoff: context.getUniformLocation(program, "distance_cutoff")!,  
    }
  }
  return {
    activate: () => { context.useProgram(program); },
    setUniforms: ({transformation, depthCutoff, distanceCutoff}: {transformation: Matrix, depthCutoff: number, distanceCutoff: number}) => {
      // With the program active, set the uniforms of it.
      context.uniformMatrix4fv(attributeLocations.uniform.transformation, true, flattenMatrix(transformation));
      context.uniform1f(attributeLocations.uniform.depthCutoff, depthCutoff);
      context.uniform1f(attributeLocations.uniform.distanceCutoff, distanceCutoff);
    },
    setAttributeLocations: ({stride, positionOffset, travelOffset}: {stride: number, positionOffset: number, travelOffset: number}) => {
      // With the program active and a VAO bound, set the attribute locations.
      context.vertexAttribPointer(attributeLocations.input.position, 3, context.FLOAT, false, stride, positionOffset);
      context.vertexAttribPointer(attributeLocations.input.distance, 1, context.FLOAT, false, stride, travelOffset);
      context.enableVertexAttribArray(attributeLocations.input.position);
      context.enableVertexAttribArray(attributeLocations.input.distance);  
    }
  }
}
function boundsOf(points: Array<[number, number, number]>): { min: [number, number, number], max: [number, number, number] } {
  if (points.length === 0) {
    return {
      min: [-1, -1, -1],
      max: [1, 1, 1],
    }
  } else {
    return mapValues({
      min,
      max
    }, fn => range(3).map(index => fn(points.map(point => point[index]))!) as [number, number, number])
  }
}
function flattenMatrix(matrix: Matrix): number[] {
  return flatten(matrix.values);
}

/*
{
  position: ...,
  setPosition: ...,
}
*/

export default function ViewPage({ points }: {points: Array<[number, number, number]>}) {

  /*
    
  */
  const pointMetadata = useMemo(() => {
    const bounds = boundsOf(points);
    const pointsWithTravel = points.slice(1).reduce(({ distance, items }, next) => {
      const lastPoint = items[items.length - 1];
      const segmentDistance = Math.sqrt(sum(range(3).map(index => (lastPoint[index] - next[index]) ** 2)));
      const nextDistance = distance + segmentDistance;
      items.push([...next, nextDistance]);
      return {
        distance: nextDistance,
        items,
      }
    }, { distance: 0, items: [[...points[0], 0]] as Array<[number, number, number, number]> });
    const maxRange = Math.sqrt(sum(range(3).map(index => (bounds.max[index] - bounds.min[index])**2))!);
    const center = range(3).map(index => (bounds.min[index] + bounds.max[index])/2);

    return {
      bounds,
      maxRange,
      center,
      pointsWithTravel: pointsWithTravel.items,
      totalTravel: pointsWithTravel.distance,
    };
  }, [points]);
  const [zoom, multiplyZoom] = useReducer((previous: number, next: number) => previous * next, 1);
  let [rotation, multiplyRotation] = useReducer((state: Quaternion, change: Quaternion) => {
    return change.mul(state).normalize();
  }, new Quaternion());
  const dragging = useRef(false);

  const containerRef = useRef<HTMLDivElement | null>(null);
  const getCanvasInfo = useCallback(contextCached(setupContext), []);
  const preparePoints = useCallback(contextCached(context => {
    const programInfo = getCanvasInfo(context);
    const buffer = context.createBuffer();
    context.bindBuffer(context.ARRAY_BUFFER, buffer);
    const bufferData = new Float32Array(
      pointMetadata?.pointsWithTravel?.flatMap(arr => arr) ?? []
    );
    context.bufferData(context.ARRAY_BUFFER, bufferData, context.STATIC_DRAW);
    const vao = context.createVertexArray()!;
    context.bindVertexArray(vao);
    programInfo.activate();
    programInfo.setAttributeLocations({
      stride: 4 * 4,
      positionOffset: 0,
      travelOffset: 3 * 4,
    });
    return {
      draw: () => {
        context.bindVertexArray(vao);
        context.bindBuffer(context.ARRAY_BUFFER, buffer);
        context.drawArrays(context.LINE_STRIP, 0, pointMetadata?.pointsWithTravel?.length ?? 0);
      }
    }
  }), [pointMetadata]);

  console.log("COMPONENT FUNCTION!", points)

  const setupCanvas = useCallback((canvas: HTMLCanvasElement) => {
    console.log("SETUP FUNCTION!")
    canvas.onwheel = event => {
      if(event.deltaY > 0) {
        multiplyZoom(0.8);
      } else {
        multiplyZoom(1 / 0.8);
      }
    };
    canvas.onmousedown = event => {
      if (event.buttons & 1) {
        dragging.current = true;
      }
    }
    canvas.onmousemove = event => {
      if (dragging.current && (event.buttons & 1)) {
        const factor = 0.001;
        const change = new Quaternion(0, -event.movementY * factor, -event.movementX * factor, 0).exp()
        multiplyRotation(change);
      } else if(dragging.current) {
        // TODO: Mouse up should track globally...
        dragging.current = false;
      }
    };
  }, []);

  const render = useMemo(() => {
    if(points.length === 0) {
      console.log("NO POINTS :(")
      return () => {};
    }
    function render(context: WebGL2RenderingContext, canvas: HTMLCanvasElement) {
      console.log("RENDER FUNCTION!");
      const programInfo = getCanvasInfo(context);
      programInfo.activate();
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      const aspect = width / height;
      canvas.width = width;
      canvas.height = height;
      context.viewport(0, 0, width, height);
      context.clearColor(0.0, 0.0, 0.0, 1.0);
      context.clear(context.COLOR_BUFFER_BIT);
      if(!pointMetadata) {
        return;
      }
      //context.useProgram(canvasInfo.program);
      const center = pointMetadata.center;
      const maxRange = pointMetadata.maxRange;
      //
      const translateAroundZero = new Matrix(4, 4, [
        [1, 0, 0, -center[0]],
        [0, 1, 0, -center[1]],
        [0, 0, 1, -center[2]],
        [0, 0, 0, 1]
      ])
      const shrinkToUnitSphere = new Matrix(4, 4, [
        [2 / maxRange, 0, 0, 0],
        [0, 2 / maxRange, 0, 0],
        [0, 0, -2 / maxRange, 0],
        [0, 0, 0, 1],
      ])
      const rotate = new Matrix(4, 4, rotation.toMatrix4(true))
      const translateToOneToThree = new Matrix(4, 4, [
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 1],
        [0, 0, 0, 1],
      ])
      const project = new Matrix(4, 4, [
        [zoom / aspect, 0, 0, 0],
        [0, zoom, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 1, 1],
      ])
      const combined = (
        project
        .multiply(translateToOneToThree)
        .multiply(rotate)
        .multiply(shrinkToUnitSphere)
        .multiply(translateAroundZero)
      )
      programInfo.setUniforms({
        transformation: combined,
        depthCutoff: (pointMetadata.bounds.max[2]) + 0.001,
        distanceCutoff: pointMetadata.totalTravel,
      })
      preparePoints(context).draw();
    }
    return render;
  }, [points, zoom, rotation, pointMetadata, preparePoints]);
  
    return <div ref={containerRef} style={{position: "relative", width: "100%", height: "100%"}}>
      <Canvas render={render} setupCanvas={setupCanvas}/>
    </div>
}