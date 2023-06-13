import { useEffect, useRef } from "react"
import { Maybe } from "./util/types";

export function contextCached<T>(callback: (context: WebGL2RenderingContext) => T): (context: WebGL2RenderingContext) => T {
  let lastContext: Maybe<WebGL2RenderingContext | null> = null;
  let lastResult: Maybe<T> = null;
  return (context) => {
    if(lastContext !== context) {
      lastResult = callback(context);
      lastContext = context;
    }
    // We know that callback will have been called at least once, so now lastResult: T
    return lastResult as T;
  };
}

function useRemoteGetter<T>(value: T): () => T {
  const result = useRef(value);
  result.current = value;
  return () => result.current;
}

export function Canvas({ setupCanvas, render }: { setupCanvas?: (canvas: HTMLCanvasElement) => void, render: (context: WebGL2RenderingContext, canvas: HTMLCanvasElement) => void}) {
  const trueCanvasRef = useRef<HTMLCanvasElement | null>(null);
  /*
    Forward props to the running thread
  */
  const settings = useRemoteGetter({ setupCanvas: setupCanvas ?? null, render });

  useEffect(() => {
    let setupCanvas: Maybe<HTMLCanvasElement> | null = null;
    let setupCanvasFunction: Maybe<(canvas: HTMLCanvasElement) => void> = null;
    let animationFrame = 0;
    function render() {
      if(trueCanvasRef.current === null) {
        return;
      }
      if(trueCanvasRef.current !== setupCanvas || setupCanvasFunction != settings().setupCanvas) {
        const setupFunction = settings().setupCanvas;
        setupFunction?.(trueCanvasRef.current);
        setupCanvas = trueCanvasRef.current;
        setupCanvasFunction = setupFunction;
      }
      const context = trueCanvasRef.current.getContext("webgl2");
      if(!context) {
        return;
      }
      settings().render(context, trueCanvasRef.current);
    }
    function frameCallback() {
      render();
      animationFrame = requestAnimationFrame(frameCallback);
    }
    frameCallback();
    return () => {
      cancelAnimationFrame(animationFrame)
    }
  }, []);
  return (
    <canvas ref={trueCanvasRef} style={{width: "100%", height: "100%", position: "absolute" }}/>
  )
}