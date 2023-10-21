import React from "react";
export declare function contextCached<T>(callback: (context: WebGL2RenderingContext) => T): (context: WebGL2RenderingContext) => T;
export declare function Canvas({ setupCanvas, render }: {
    setupCanvas?: (canvas: HTMLCanvasElement) => void;
    render: (context: WebGL2RenderingContext, canvas: HTMLCanvasElement) => void;
}): React.JSX.Element;
