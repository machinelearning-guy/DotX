import React, { useRef, useEffect, useState, useCallback } from 'react'
import { PlotConfig, ViewPort, PlotStats, Tooltip, ROISelection } from '../types'

interface PlotCanvasProps {
  config: PlotConfig
  viewport: ViewPort
  onViewportChange: (viewport: Partial<ViewPort>) => void
  stats: PlotStats
}

const PlotCanvas: React.FC<PlotCanvasProps> = ({
  config,
  viewport,
  onViewportChange,
  stats
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const animationRef = useRef<number>()
  
  const [isDragging, setIsDragging] = useState(false)
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 })
  const [lastFrameTime, setLastFrameTime] = useState(0)
  const [fps, setFps] = useState(0)
  const [tooltip, setTooltip] = useState<Tooltip>({
    x: 0, y: 0, content: '', visible: false
  })
  const [roiSelection, setRoiSelection] = useState<ROISelection>({
    startX: 0, startY: 0, endX: 0, endY: 0, active: false
  })
  const [roiFinal, setRoiFinal] = useState<{ x: number, y: number, w: number, h: number } | null>(null)
  const [roiBusy, setRoiBusy] = useState(false)

  // Update canvas size when container resizes
  useEffect(() => {
    const updateSize = () => {
      const container = containerRef.current
      const canvas = canvasRef.current
      if (!container || !canvas) return

      const rect = container.getBoundingClientRect()
      const dpr = window.devicePixelRatio || 1
      
      canvas.width = rect.width * dpr
      canvas.height = rect.height * dpr
      canvas.style.width = `${rect.width}px`
      canvas.style.height = `${rect.height}px`
      
      onViewportChange({ width: rect.width, height: rect.height })
    }

    updateSize()
    const resizeObserver = new ResizeObserver(updateSize)
    if (containerRef.current) {
      resizeObserver.observe(containerRef.current)
    }

    return () => resizeObserver.disconnect()
  }, [onViewportChange])

  // Animation loop for rendering
  useEffect(() => {
    const animate = (currentTime: number) => {
      if (currentTime - lastFrameTime >= 16.67) { // ~60 FPS
        render()
        
        // Calculate FPS
        if (lastFrameTime > 0) {
          const fps = 1000 / (currentTime - lastFrameTime)
          setFps(Math.round(fps))
        }
        setLastFrameTime(currentTime)
      }
      
      animationRef.current = requestAnimationFrame(animate)
    }

    animationRef.current = requestAnimationFrame(animate)
    
    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current)
      }
    }
  }, [viewport, config])

  const render = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    ctx.scale(dpr, dpr)

    const width = canvas.width / dpr
    const height = canvas.height / dpr

    // Clear canvas
    ctx.fillStyle = config.theme === 'dark' ? '#1a1a1a' : '#ffffff'
    ctx.fillRect(0, 0, width, height)

    // Draw grid
    drawGrid(ctx, width, height)
    
    // Draw axis labels
    drawAxisLabels(ctx, width, height)
    
    // Draw plot area
    drawPlotArea(ctx, width, height)
    
    // Draw anchors/dots (placeholder for now)
    drawAnchors(ctx, width, height)
    
    // Draw ROI selection
    if (roiSelection.active) {
      drawROISelection(ctx)
    }

    // Draw stats overlay
    drawStatsOverlay(ctx, width, height)
  }, [config, viewport, stats, roiSelection])

  const drawGrid = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    ctx.strokeStyle = config.theme === 'dark' ? '#333' : '#e5e5e5'
    ctx.lineWidth = 1
    ctx.setLineDash([2, 2])

    const gridSize = 50 * viewport.zoom
    const offsetX = viewport.x % gridSize
    const offsetY = viewport.y % gridSize

    // Vertical lines
    for (let x = offsetX; x < width; x += gridSize) {
      ctx.beginPath()
      ctx.moveTo(x, 0)
      ctx.lineTo(x, height)
      ctx.stroke()
    }

    // Horizontal lines
    for (let y = offsetY; y < height; y += gridSize) {
      ctx.beginPath()
      ctx.moveTo(0, y)
      ctx.lineTo(width, y)
      ctx.stroke()
    }

    ctx.setLineDash([])
  }

  const drawAxisLabels = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    ctx.fillStyle = config.theme === 'dark' ? '#fff' : '#000'
    ctx.font = '14px Inter, sans-serif'

    const xLabel = config.swapAxes ? 'Query' : 'Target/Reference'
    const yLabel = config.swapAxes ? 'Target/Reference' : 'Query'

    // X axis label
    ctx.save()
    ctx.textAlign = 'center'
    ctx.fillText(xLabel, width / 2, height - 10)
    
    // Y axis label (rotated)
    ctx.save()
    ctx.translate(15, height / 2)
    ctx.rotate(-Math.PI / 2)
    ctx.textAlign = 'center'
    ctx.fillText(yLabel, 0, 0)
    ctx.restore()
    ctx.restore()
  }

  const drawPlotArea = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    // Draw main plot border
    ctx.strokeStyle = config.theme === 'dark' ? '#555' : '#ccc'
    ctx.lineWidth = 2
    ctx.strokeRect(40, 20, width - 60, height - 60)

    // Draw diagonal reference lines
    ctx.strokeStyle = config.theme === 'dark' ? '#444' : '#ddd'
    ctx.lineWidth = 1
    ctx.setLineDash([5, 5])

    // Main diagonal (forward strand reference)
    ctx.beginPath()
    ctx.moveTo(40, height - 40)
    ctx.lineTo(width - 20, 20)
    ctx.stroke()

    // Anti-diagonal (reverse strand reference)
    ctx.beginPath()
    ctx.moveTo(40, 20)
    ctx.lineTo(width - 20, height - 40)
    ctx.stroke()

    ctx.setLineDash([])
  }

  const drawAnchors = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    // For now, draw some sample dots to show the concept
    const plotArea = {
      x: 40,
      y: 20,
      width: width - 60,
      height: height - 60
    }

    // Forward strand anchors (blue, main diagonal tendency)
    if (config.showForwardStrand) {
      ctx.fillStyle = '#3b82f6' // Blue for forward strand
      for (let i = 0; i < 1000; i++) {
        const x = plotArea.x + Math.random() * plotArea.width
        const y = plotArea.y + plotArea.height - (x - plotArea.x) + (Math.random() - 0.5) * 50
        if (y >= plotArea.y && y <= plotArea.y + plotArea.height) {
          ctx.beginPath()
          ctx.arc(x, y, 1, 0, Math.PI * 2)
          ctx.fill()
        }
      }
    }

    // Reverse strand anchors (red, anti-diagonal tendency)
    if (config.showReverseStrand) {
      ctx.fillStyle = '#ef4444' // Red for reverse strand
      for (let i = 0; i < 500; i++) {
        const x = plotArea.x + Math.random() * plotArea.width
        const y = plotArea.y + (x - plotArea.x) + (Math.random() - 0.5) * 30
        if (y >= plotArea.y && y <= plotArea.y + plotArea.height) {
          ctx.beginPath()
          ctx.arc(x, y, 1, 0, Math.PI * 2)
          ctx.fill()
        }
      }
    }
  }

  const drawROISelection = (ctx: CanvasRenderingContext2D) => {
    ctx.strokeStyle = '#fbbf24'
    ctx.fillStyle = 'rgba(251, 191, 36, 0.1)'
    ctx.lineWidth = 2
    ctx.setLineDash([5, 5])

    const x = Math.min(roiSelection.startX, roiSelection.endX)
    const y = Math.min(roiSelection.startY, roiSelection.endY)
    const w = Math.abs(roiSelection.endX - roiSelection.startX)
    const h = Math.abs(roiSelection.endY - roiSelection.startY)

    ctx.fillRect(x, y, w, h)
    ctx.strokeRect(x, y, w, h)
    ctx.setLineDash([])
  }

  const drawStatsOverlay = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
    // FPS and anchor count in top-right corner
    ctx.fillStyle = 'rgba(0, 0, 0, 0.8)'
    ctx.fillRect(width - 160, 10, 150, 60)
    
    ctx.fillStyle = '#fff'
    ctx.font = '12px Inter, sans-serif'
    ctx.textAlign = 'left'
    ctx.fillText(`FPS: ${fps}`, width - 150, 25)
    ctx.fillText(`Anchors: ${stats.anchorCount.toLocaleString()}`, width - 150, 40)
    ctx.fillText(`Visible: ${stats.visibleAnchors.toLocaleString()}`, width - 150, 55)
  }

  // Mouse event handlers
  const handleMouseDown = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect()
    if (!rect) return

    const x = e.clientX - rect.left
    const y = e.clientY - rect.top

    if (e.shiftKey) {
      // Start ROI selection
      setRoiSelection({
        startX: x,
        startY: y,
        endX: x,
        endY: y,
        active: true
      })
    } else {
      // Start panning
      setIsDragging(true)
      setDragStart({ x: e.clientX, y: e.clientY })
    }
  }

  const handleMouseMove = (e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect()
    if (!rect) return

    const x = e.clientX - rect.left
    const y = e.clientY - rect.top

    if (roiSelection.active) {
      setRoiSelection(prev => ({
        ...prev,
        endX: x,
        endY: y
      }))
    } else if (isDragging) {
      const deltaX = e.clientX - dragStart.x
      const deltaY = e.clientY - dragStart.y

      onViewportChange({
        x: viewport.x - deltaX / viewport.zoom,
        y: viewport.y - deltaY / viewport.zoom
      })

      setDragStart({ x: e.clientX, y: e.clientY })
    } else {
      // Show tooltip on hover (placeholder)
      setTooltip({
        x: e.clientX,
        y: e.clientY,
        content: `X: ${Math.round(x)}, Y: ${Math.round(y)}`,
        visible: true
      })
    }
  }

  const handleMouseUp = () => {
    if (roiSelection.active) {
      // Complete ROI selection and show mini-toolbar
      const x = Math.min(roiSelection.startX, roiSelection.endX)
      const y = Math.min(roiSelection.startY, roiSelection.endY)
      const w = Math.abs(roiSelection.endX - roiSelection.startX)
      const h = Math.abs(roiSelection.endY - roiSelection.startY)
      if (w > 2 && h > 2) {
        setRoiFinal({ x, y, w, h })
      }
      setRoiSelection(prev => ({ ...prev, active: false }))
    }
    setIsDragging(false)
  }

  const handleMouseLeave = () => {
    setTooltip(prev => ({ ...prev, visible: false }))
    setIsDragging(false)
    setRoiSelection(prev => ({ ...prev, active: false }))
  }

  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault()
    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1
    const newZoom = Math.max(0.01, Math.min(100, viewport.zoom * zoomFactor))
    
    onViewportChange({ zoom: newZoom })
  }

  return (
    <div
      ref={containerRef}
      style={{
        flex: 1,
        position: 'relative',
        cursor: roiSelection.active ? 'crosshair' : 
                isDragging ? 'grabbing' : 'grab'
      }}
    >
      <canvas
        ref={canvasRef}
        className="plot-canvas"
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseLeave}
        onWheel={handleWheel}
        style={{
          display: 'block',
          width: '100%',
          height: '100%'
        }}
      />
      
      {/* Tooltip */}
      {tooltip.visible && (
        <div
          className="tooltip"
          style={{
            left: tooltip.x + 10,
            top: tooltip.y - 30
          }}
        >
          {tooltip.content}
        </div>
      )}

      {/* ROI mini-toolbar */}
      {roiFinal && (
        <div
          style={{
            position: 'absolute',
            left: roiFinal.x + roiFinal.w + 8,
            top: roiFinal.y,
            background: 'rgba(0,0,0,0.85)',
            color: '#fff',
            padding: '8px',
            borderRadius: 6,
            display: 'flex',
            flexDirection: 'column',
            gap: 6,
            zIndex: 20,
          }}
        >
          <button disabled={roiBusy} onClick={async () => {
            try {
              setRoiBusy(true)
              await (window as any).__TAURI__.invoke('verify_roi', {
                x: Math.round(roiFinal.x), y: Math.round(roiFinal.y),
                w: Math.round(roiFinal.w), h: Math.round(roiFinal.h),
                viewport
              })
            } finally {
              setRoiBusy(false)
            }
          }} style={{ padding: '6px 10px' }}>Verify</button>
          <button disabled={roiBusy} onClick={async () => {
            const save = await (window as any).__TAURI__.dialog.save({
              filters: [{ name: 'JSON', extensions: ['json'] }]
            })
            if (save) {
              await (window as any).__TAURI__.invoke('save_roi', {
                path: save,
                roi: { x: Math.round(roiFinal.x), y: Math.round(roiFinal.y), w: Math.round(roiFinal.w), h: Math.round(roiFinal.h), viewport }
              })
            }
          }} style={{ padding: '6px 10px' }}>Save ROI</button>
          <button disabled={roiBusy} onClick={() => setRoiFinal(null)} style={{ padding: '6px 10px' }}>Close</button>
        </div>
      )}
      
      {/* Instructions overlay */}
      <div style={{
        position: 'absolute',
        bottom: '20px',
        left: '20px',
        background: 'rgba(0, 0, 0, 0.8)',
        color: 'white',
        padding: '10px',
        borderRadius: '4px',
        fontSize: '12px',
        maxWidth: '300px'
      }}>
        <div><strong>Instructions:</strong></div>
        <div>• Drag to pan, scroll to zoom</div>
        <div>• Hold Shift + drag for ROI selection</div>
        <div>• Keyboard: +/- zoom, WASD/arrows pan, X swap, R reverse-Y</div>
        <div><strong>Axes:</strong> X = {config.swapAxes ? 'Query' : 'Target'}, Y = {config.swapAxes ? 'Target' : 'Query'}</div>
        <div><strong>Strands:</strong> Blue (+) main diagonal, Red (-) anti-diagonal</div>
        {stats.verificationStatus !== 'none' && (
          <div style={{ marginTop: '6px', opacity: 0.8 }}>
            Identity active: point opacity encodes identity
          </div>
        )}
      </div>
    </div>
  )
}

export default PlotCanvas
