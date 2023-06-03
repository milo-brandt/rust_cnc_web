import { useCallback, useMemo, useReducer, useRef, useState } from "react";
import { PageErrored, PageLoading } from "./ErrorState";
import { useGet } from "./api/generic";
import { useParams } from "react-router-dom";
import { Maybe } from "./util/types";
import { flatten, mapValues, max, min, range, round, sum } from "lodash";
import { Box, Checkbox, Grow, IconButton, Paper, Slider, Typography } from "@mui/material";
import { Matrix } from "ts-matrix";
import Quaternion from "quaternion";
import { Info, PlayArrow, Work } from "@mui/icons-material";
import { Canvas, contextCached } from "./Canvas";

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
  /*
    
  */
  const pointMetadata = useMemo(() => {
    if(lineResult.status == "resolved" && lineResult.data.length > 0) {
      const points = lineResult.data;
      const bounds = boundsOf(lineResult.data);
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
    } else {
      return null;
    }
  }, [lineResult]);
  const dragBounds = pointMetadata ? [pointMetadata.bounds.min[2], pointMetadata.bounds.max[2]] : null;
  function displayBound(index: number, name: string) {
    return (<>
       <Box textAlign="right">{ pointMetadata?.bounds?.min?.[index]?.toFixed(2) ?? '?' } { "<" }</Box> <Box ml={0.5}>{ name }</Box> <Box>{ "<" } { pointMetadata?.bounds?.max?.[index]?.toFixed(2) ?? '?' }</Box>
    </>)
  }
  const travelLength = pointMetadata?.totalTravel ?? null;
  const [zPosition, setZPosition] = useState<number | null>(null);
  const [travelPosition, setTravelPosition] = useState<number | null>(null);
  const [zoom, multiplyZoom] = useReducer((previous: number, next: number) => previous * next, 1);
  let [rotation, multiplyRotation] = useReducer((state: Quaternion, change: Quaternion) => {
    return change.mul(state).normalize();
  }, new Quaternion());
  const dragging = useRef(false);

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
  //
  const getCanvasInfo = useCallback(contextCached(setupContext), []);
  const activateProgram = useCallback((context: WebGL2RenderingContext) => {
    const canvasInfo = getCanvasInfo(context);
    context.useProgram(canvasInfo.program);
    return canvasInfo;  
  }, []);
  const preparePoints = useCallback(contextCached(context => {
    const canvasInfo = getCanvasInfo(context);
    const buffer = context.createBuffer();
    context.bindBuffer(context.ARRAY_BUFFER, buffer);
    const bufferData = new Float32Array(
      pointMetadata?.pointsWithTravel?.flatMap(arr => arr) ?? []
    );
    context.bufferData(context.ARRAY_BUFFER, bufferData, context.STATIC_DRAW);
    const vao = context.createVertexArray()!;
    context.bindVertexArray(vao);
    context.vertexAttribPointer(canvasInfo.attributeLocations.input.position, 3, context.FLOAT, false, 4 * 4, 0);
    context.vertexAttribPointer(canvasInfo.attributeLocations.input.distance, 1, context.FLOAT, false, 4 * 4, 3 * 4);
    context.enableVertexAttribArray(canvasInfo.attributeLocations.input.position);
    context.enableVertexAttribArray(canvasInfo.attributeLocations.input.distance);
    return {
      draw: () => {
        context.bindVertexArray(vao);
        context.bindBuffer(context.ARRAY_BUFFER, buffer);
        context.drawArrays(context.LINE_STRIP, 0, pointMetadata?.pointsWithTravel?.length ?? 0);
      }
    }
  }), [pointMetadata]);


  const setupCanvas = useCallback((canvas: HTMLCanvasElement) => {
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
    if(lineResult.status !== "resolved" || lineResult.data.length === 0) {
      return () => {};
    }
    function render(context: WebGL2RenderingContext, canvas: HTMLCanvasElement) {
      if(!pointMetadata) {
        return;
      }
      const canvasInfo = activateProgram(context);
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      const aspect = width / height;
      canvas.width = width;
      canvas.height = height;
      context.viewport(0, 0, width, height);
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

      context.uniformMatrix4fv(canvasInfo.attributeLocations.uniform.transformation, true, flattenMatrix(combined));
      context.uniform1f(canvasInfo.attributeLocations.uniform.depthCutoff, (zPosition ?? pointMetadata.bounds.max[2]) + 0.001);
      context.uniform1f(canvasInfo.attributeLocations.uniform.distanceCutoff, travelRef.current ?? pointMetadata.totalTravel);
      context.clearColor(0.0, 0.0, 0.0, 1.0);
      context.clear(context.COLOR_BUFFER_BIT);
      preparePoints(context).draw();
    }
    return render;
  }, [lineResult, activateProgram, zoom, rotation, pointMetadata, zPosition, preparePoints]);
  
  if(lineResult.status == "loading") {
    return <PageLoading/>
  } else if(lineResult.status == "rejected") {
    return <PageErrored/>
  } else {
    return <div ref={containerRef} style={{position: "relative", width: "100%", height: "90vh"}}>
      <Canvas render={render} setupCanvas={setupCanvas}/>
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
          <Box marginTop="1rem" fontWeight="bolder">
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
          <Box marginTop="1rem" fontWeight="bolder">
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
              <Typography variant="h6">Job Execution</Typography>
              <Box display="flex" alignItems="center" width="100%" justifyContent="flex-end">
              <em>Run:</em> <IconButton><PlayArrow/> </IconButton>
              </Box>
            </Paper>
          </Grow>
        </Box>
        <Box fontSize="2em">
          <Checkbox
              icon={ <Info color="inherit" fontSize="inherit"/> }
              checkedIcon={ <Info fontSize="inherit"/> }
              checked={ openTab === "info" }
              onChange={ (_, checked) => setOpenTab("info", checked) }
              sx={{color: "white"}}
            />
            <Checkbox
              icon={ <Work color="inherit" fontSize="inherit"/> }
              checkedIcon={ <Work fontSize="inherit"/> }
              checked={ openTab === "job" }
              onChange={ (_, checked) => setOpenTab("job", checked) }
              sx={{color: "white"}}
            />
        </Box>
      </Box>
        
      }
    </div>
  }
}