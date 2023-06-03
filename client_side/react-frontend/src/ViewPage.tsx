import { useEffect, useRef, useState } from "react";
import { PageErrored, PageLoading } from "./ErrorState";
import { useGet } from "./api/generic";
import { useParams } from "react-router-dom";
import { Maybe } from "./util/types";
import { flatten, isEqual, mapValues, max, min, range, round, sum } from "lodash";
import { Box, Button, Checkbox, Collapse, Fade, Grow, IconButton, Paper, Slide, Slider, Typography } from "@mui/material";
import { Matrix } from "ts-matrix";
import Quaternion from "quaternion";
import { ExpandLess, Info, Work } from "@mui/icons-material";

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
function setupCanvas(context: WebGL2RenderingContext) {
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
    program,
    attributeLocations,
    buffer: context.createBuffer()!,
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

export function ViewPage() {
  let { "*": directory } = useParams() as {"*": string};
  if(directory && directory[directory.length - 1] == "/") {
    directory = directory.slice(0, directory.length - 1);
  }
  const { result: lineResult } = useGet<Array<[number, number, number]>>(`/job/examine/${directory}`);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const pointsOld: Array<[number, number, number]> = [
    [-1, -1, 2],
    [-1, 1, 2],
    [1, 1, 2],
    [1, -1, 3],
  ];
  const pointsRef = useRef<Array<[number, number, number]>>(pointsOld);
  pointsRef.current = lineResult.status == "resolved" ? lineResult.data : pointsOld;
  const [dragBounds, setDragBounds] = useState<[number, number] | null>(null);
  const [bounds, setBounds] = useState<{min: [number, number, number], max: [number, number, number]} | null>(null);
  function displayBound(index: number, name: string) {
    return (<>
       <Box textAlign="right">{ bounds?.min?.[index]?.toFixed(2) ?? '?' } { "<" }</Box> <Box ml={0.5}>{ name }</Box> <Box>{ "<" } { bounds?.max?.[index]?.toFixed(2) ?? '?' }</Box>
    </>)
  }
  const [travelLength, setTravelLength] = useState<number | null>(null);
  const [zPosition, setZPosition] = useState<number | null>(null);
  const [travelPosition, setTravelPosition] = useState<number | null>(null);
  const [showInfo, setShowInfo] = useState(false);
  const zPositionRef = useRef<number | null>(null);
  zPositionRef.current = zPosition;
  const travelRef = useRef<number | null>(null);
  travelRef.current = travelPosition;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [openTab, setOpenTabRaw] = useState<"info" | "job" | null>(null);
  function setOpenTab(name: "info" | "job", isOpen: boolean) {
    if(isOpen) {
      setOpenTabRaw(name);
    } else if(openTab == name) {
      setOpenTabRaw(null);
    }
  }

  useEffect(() => {
    let isActive = true;
    let canvasInfo: Maybe<ReturnType<typeof setupCanvas>> = null;
    let position = new Quaternion();
    let zoom = 1;
    let lastDragBounds = dragBounds;
    let lastTravelLength = travelLength;
    let dragging = false;
    let lastContext: WebGL2RenderingContext | null = null;
    function render() {
      if(canvasRef.current === null) {
        return;
      }
      const context = canvasRef.current.getContext("webgl2");
      if(!context) {
        return;
      }
      if(context != lastContext) {
        console.log("CONTEXT CHANGED!!")
        lastContext = context;
        canvasInfo = null;
      }
      if(canvasInfo === null) {
        canvasInfo = setupCanvas(context);
        canvasRef.current.onmousedown = event => {
          if (event.buttons & 1) {
            dragging = true;
          }
        }
        canvasRef.current.onmousemove = event => {
          if (dragging && (event.buttons & 1)) {
            const factor = 0.001;
            const change = new Quaternion(0, -event.movementY * factor, -event.movementX * factor, 0).exp()
            position = change.mul(position).normalize();
          } else if(dragging) {
            // TODO: Mouse up should track globally...
            dragging = false;
          }
        };
        canvasRef.current.onwheel = event => {
          if(event.deltaY > 0) {
            zoom *= 0.8;
          } else {
            zoom /= 0.8;
          }
        };
      }
      /*const points: Array<[number, number, number]> = [// pointsRef.current; 
        [0, 0, 0],
        [0, 1, 0],
        [1, 0, 0],
        [0, -1, 0],
        [-1, 0, 0],
        [0, 1, 1],
        [1, 0, 1],
        [0, -1, 1],
        [-1, 0, 1],
        [0, 1, 2],
        [1, 0, 2],
        [0, -1, 2],
        [-1, 0, 2],
        [0, 1, 3],
        [1, 0, 3],
        [0, -1, 3],
        [-1, 0, 3],
        [0, 1, 4],
        [1, 0, 4],
        [0, -1, 4],
        [-1, 0, 4],
      ];*/
      const points = pointsRef.current;
      if(points.length === 0) {
        return;
      }
      const width = canvasRef.current?.clientWidth!;
      const height = canvasRef.current?.clientHeight!;
      const aspect = width / height;
      canvasRef.current!.width = width;
      canvasRef.current!.height = height;
      context.viewport(0, 0, width, height);
      context.useProgram(canvasInfo.program);
      const bounds = boundsOf(points);
      const zBounds: [number, number] = [bounds.min[2], bounds.max[2]]
      if(!isEqual(zBounds, lastDragBounds)) {
        lastDragBounds = zBounds;
        setDragBounds(zBounds);
        setZPosition(zBounds[1]);
        setBounds(bounds);
      }

      const maxRange = Math.sqrt(sum(range(3).map(index => (bounds.max[index] - bounds.min[index])**2))!);
      const center = range(3).map(index => (bounds.min[index] + bounds.max[index])/2);
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
      const rotate = new Matrix(4, 4, position.toMatrix4(true))
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
      if(lastTravelLength != pointsWithTravel.distance) {
        setTravelLength(pointsWithTravel.distance);
        lastTravelLength = pointsWithTravel.distance;
      }

      context.uniformMatrix4fv(canvasInfo.attributeLocations.uniform.transformation, true, /*[
        1 / aspect, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 1, 1,
      ]*/ flattenMatrix(combined));
      context.uniform1f(canvasInfo.attributeLocations.uniform.depthCutoff, (zPositionRef.current ?? zBounds[1]) + 0.001);
      context.uniform1f(canvasInfo.attributeLocations.uniform.distanceCutoff, travelRef.current ?? pointsWithTravel.distance);
      context.bindBuffer(context.ARRAY_BUFFER, canvasInfo.buffer);
      const bufferData = new Float32Array(
        pointsWithTravel.items.flatMap(arr => arr)
      );
      context.bufferData(context.ARRAY_BUFFER, bufferData, context.STATIC_DRAW);
      const vao = context.createVertexArray()!;
      context.bindVertexArray(vao);
      context.vertexAttribPointer(canvasInfo.attributeLocations.input.position, 3, context.FLOAT, false, 4 * 4, 0);
      context.vertexAttribPointer(canvasInfo.attributeLocations.input.distance, 1, context.FLOAT, false, 4 * 4, 3 * 4);
      context.enableVertexAttribArray(canvasInfo.attributeLocations.input.position)
      context.enableVertexAttribArray(canvasInfo.attributeLocations.input.distance)
      context.bindVertexArray(vao);
      context.clearColor(0.0, 0.0, 0.0, 1.0);
      context.clear(context.COLOR_BUFFER_BIT);
      context.drawArrays(context.LINE_STRIP, 0, points.length);
    }
    function frameCallback() {
      render();
      if(isActive) {
        requestAnimationFrame(frameCallback)
      }
    }
    frameCallback();
    return () => { isActive = false; }
  }, []);
  
  if(lineResult.status == "loading") {
    return <PageLoading/>
  } else if(lineResult.status == "rejected") {
    return <PageErrored/>
  } else {
    return <div ref={containerRef} style={{position: "relative", width: "100%", height: "90vh"}}>
      <canvas ref={canvasRef} style={{width: "100%", height: "100%", position:"absolute"}}>
      </canvas>
      { dragBounds !== null && travelLength !== null && <Box position="absolute" bottom="2rem" top="2rem" left="1rem" display="flex" flexDirection="row" color="white">
      <Box display="flex" flexDirection="column" alignItems="center" zIndex={10}>
          <Slider
            orientation="vertical"
            valueLabelDisplay="auto"
            step={0.001}
            min={dragBounds[0] - 0.002}
            max={dragBounds[1]}
            value={zPosition ?? dragBounds[1]}
            onChange={ (_, value) => setZPosition(value as number) }
            valueLabelFormat={ value => `Z < ${ round(value, 2) }`}
            sx={{
              '& .MuiSlider-valueLabel': {
                opacity: 1.0,
                right: "auto",
                left: "30px",
              },
              '& .MuiSlider-valueLabel::before': {
                right: "auto",
                left: "0px",
              },
            }}
          />
          <Box marginTop="1rem">
            Z
          </Box>
        </Box>
        <Box display="flex" flexDirection="column" alignItems="center" zIndex={9}>
          <Slider
            orientation="vertical"
            valueLabelDisplay="auto"
            step={0.001}
            min={0}
            max={travelLength}
            value={travelPosition ?? travelLength}
            onChange={ (_, value) => setTravelPosition(value as number) }
            valueLabelFormat={ value => `T < ${ round(value) }`}
            sx={{
              '& .MuiSlider-valueLabel': {
                opacity: 1.0,
                right: "auto",
                left: "30px",
              },
              '& .MuiSlider-valueLabel::before': {
                right: "auto",
                left: "0px",
              },
            }}
          />
          <Box marginTop="1rem">
            T
          </Box>
        </Box>
      </Box> }
      { travelLength !== null && <Box position="absolute" right="1rem" bottom="0rem" display="flex" flexDirection="column" alignItems="flex-end" color="white">
        <Box position="relative">
        <Grow in={openTab === "info"} mountOnEnter={true} unmountOnExit={true}>
            <Paper sx={{p: 1, position: "absolute", bottom: "0rem", right: "0rem", whiteSpace: "nowrap"}}>
                <div>
                  <Typography variant="h6">Job Info</Typography>
                  <em>Bounds (mm):</em>
                  <Box display="grid" gridTemplateColumns="auto auto auto" ml="1rem">
                    { displayBound(0, "X") }
                    { displayBound(1, "Y") }
                    { displayBound(2, "Z") }
                  </Box>
                  <em>Travel:</em> { (travelLength / 1000).toFixed(2) } m <br/>
                </div>
            </Paper>
          </Grow>
          <Grow in={openTab === "job"} mountOnEnter={true} unmountOnExit={true}>
            <Paper sx={{p: 1, position: "absolute", bottom: "0rem", right: "0rem", whiteSpace: "nowrap"}}>
              Run Job: 
            </Paper>
          </Grow>
        </Box>
        <div>
        <Checkbox
            icon={ <Info color="inherit"/> }
            checkedIcon={ <Info/> }
            checked={ openTab === "info" }
            onChange={ (_, checked) => setOpenTab("info", checked) }
            sx={{color: "white"}}
          />
          <Checkbox
            icon={ <Work color="inherit"/> }
            checkedIcon={ <Work/> }
            checked={ openTab === "job" }
            onChange={ (_, checked) => setOpenTab("job", checked) }
            sx={{color: "white"}}
          />
        </div>
      </Box>
        
      }
    </div>
  }
}