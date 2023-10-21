import React from "react";
import { useEffect, useRef } from "react";
export function contextCached(callback) {
    var lastContext = null;
    var lastResult = null;
    return function (context) {
        if (lastContext !== context) {
            lastResult = callback(context);
            lastContext = context;
        }
        // We know that callback will have been called at least once, so now lastResult: T
        return lastResult;
    };
}
function useRemoteGetter(value) {
    var result = useRef(value);
    result.current = value;
    return function () { return result.current; };
}
export function Canvas(_a) {
    var setupCanvas = _a.setupCanvas, render = _a.render;
    var trueCanvasRef = useRef(null);
    /*
      Forward props to the running thread
    */
    var settings = useRemoteGetter({ setupCanvas: setupCanvas !== null && setupCanvas !== void 0 ? setupCanvas : null, render: render });
    useEffect(function () {
        var setupCanvas = null;
        var setupCanvasFunction = null;
        var animationFrame = 0;
        function render() {
            console.log("CANVAS RENDER!");
            if (trueCanvasRef.current === null) {
                console.log("CANVAS RENDER -- NO REF!");
                return;
            }
            if (trueCanvasRef.current !== setupCanvas || setupCanvasFunction != settings().setupCanvas) {
                var setupFunction = settings().setupCanvas;
                setupFunction === null || setupFunction === void 0 ? void 0 : setupFunction(trueCanvasRef.current);
                setupCanvas = trueCanvasRef.current;
                setupCanvasFunction = setupFunction;
            }
            var context = trueCanvasRef.current.getContext("webgl2");
            if (!context) {
                console.log("CANVAS RENDER -- NO CONTEXT!");
                return;
            }
            settings().render(context, trueCanvasRef.current);
        }
        function frameCallback() {
            render();
            animationFrame = requestAnimationFrame(frameCallback);
        }
        frameCallback();
        return function () {
            cancelAnimationFrame(animationFrame);
        };
    }, []);
    return (React.createElement("canvas", { ref: trueCanvasRef, style: { width: "100%", height: "100%", position: "absolute" } }));
}
