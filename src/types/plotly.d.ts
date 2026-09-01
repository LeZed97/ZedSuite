declare module 'react-plotly.js' {
  import { Component } from 'react';
  import { PlotParams } from 'plotly.js';

  export interface PlotProps extends Partial<PlotParams> {
    data: any[];
    layout?: any;
    config?: any;
    frames?: any[];
    style?: React.CSSProperties;
    className?: string;
    useResizeHandler?: boolean;
    onInitialized?: (figure: any, graphDiv: HTMLElement) => void;
    onUpdate?: (figure: any, graphDiv: HTMLElement) => void;
    onPurge?: (figure: any, graphDiv: HTMLElement) => void;
    onError?: (err: any) => void;
    divId?: string;
    revision?: number;
    onClickAnnotation?: (event: any) => void;
    onLegendClick?: (event: any) => boolean;
    onLegendDoubleClick?: (event: any) => boolean;
    onRelayout?: (event: any) => void;
    onRestyle?: (event: any) => void;
    onRedraw?: () => void;
    onSelected?: (event: any) => void;
    onSelecting?: (event: any) => void;
    onDeselect?: () => void;
    onHover?: (event: any) => void;
    onUnhover?: (event: any) => void;
    onClick?: (event: any) => void;
    onDoubleClick?: () => void;
    onAnimated?: () => void;
    onAnimatingFrame?: (event: any) => void;
    onAnimationInterrupted?: () => void;
    onFramework?: () => void;
    onTransitioning?: () => void;
    onTransitionInterrupted?: () => void;
    onSliderChange?: (event: any) => void;
    onSliderEnd?: (event: any) => void;
    onSliderStart?: (event: any) => void;
    onWebGlContextLost?: () => void;
  }

  export default class Plot extends Component<PlotProps> {}
}
