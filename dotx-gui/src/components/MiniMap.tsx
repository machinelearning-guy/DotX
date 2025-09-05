import React, { useRef, useEffect, useState } from 'react'
import { ViewPort } from '../types'
import { X, Minimize2 } from 'lucide-react'

interface MiniMapProps {
  viewport: ViewPort
  onViewportChange: (viewport: Partial<ViewPort>) => void
}

const MiniMap: React.FC<MiniMapProps> = ({ viewport, onViewportChange }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [isDragging, setIsDragging] = useState(false)
  const [isMinimized, setIsMinimized] = useState(false)

  useEffect(() => {
    render()
  }, [viewport, isMinimized])

  const render = () => {
    const canvas = canvasRef.current
    if (!canvas || isMinimized) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const size = 150
    canvas.width = size
    canvas.height = size

    // Clear canvas
    ctx.fillStyle = 'rgba(20, 20, 20, 0.9)'
    ctx.fillRect(0, 0, size, size)

    // Draw border
    ctx.strokeStyle = '#555'
    ctx.lineWidth = 1
    ctx.strokeRect(0, 0, size, size)

    // Draw overview of the entire plot space
    const plotSize = size - 20 // Leave 10px margin on each side
    const plotOffset = 10

    // Draw plot area background
    ctx.fillStyle = '#1a1a1a'
    ctx.fillRect(plotOffset, plotOffset, plotSize, plotSize)

    // Draw sample data overview (simplified representation)
    drawOverviewData(ctx, plotOffset, plotSize)

    // Draw current viewport indicator
    drawViewportIndicator(ctx, plotOffset, plotSize)

    // Draw axes labels
    ctx.fillStyle = '#fff'
    ctx.font = '10px Inter, sans-serif'
    ctx.textAlign = 'center'
    ctx.fillText('X', size / 2, size - 3)
    
    ctx.save()
    ctx.translate(7, size / 2)
    ctx.rotate(-Math.PI / 2)
    ctx.fillText('Y', 0, 0)
    ctx.restore()
  }

  const drawOverviewData = (ctx: CanvasRenderingContext2D, offset: number, size: number) => {
    // Draw simplified representation of the full dataset
    // This would normally show a low-resolution version of the actual plot
    
    // Forward strand data (blue main diagonal)
    ctx.fillStyle = 'rgba(59, 130, 246, 0.6)'
    for (let i = 0; i < 50; i++) {
      const x = offset + Math.random() * size
      const y = offset + size - (x - offset) + (Math.random() - 0.5) * 20
      if (y >= offset && y <= offset + size) {
        ctx.fillRect(x, y, 1, 1)
      }
    }

    // Reverse strand data (red anti-diagonal)
    ctx.fillStyle = 'rgba(239, 68, 68, 0.6)'
    for (let i = 0; i < 30; i++) {
      const x = offset + Math.random() * size
      const y = offset + (x - offset) + (Math.random() - 0.5) * 15
      if (y >= offset && y <= offset + size) {
        ctx.fillRect(x, y, 1, 1)
      }
    }
  }

  const drawViewportIndicator = (ctx: CanvasRenderingContext2D, offset: number, size: number) => {
    // Calculate viewport position relative to the overview
    // This is a simplified calculation - in a real implementation, this would be based on actual data bounds
    const viewportSize = Math.min(50 / viewport.zoom, size)
    const x = offset + (size - viewportSize) / 2 + (viewport.x / 1000) * (size - viewportSize)
    const y = offset + (size - viewportSize) / 2 + (viewport.y / 1000) * (size - viewportSize)

    // Draw viewport rectangle
    ctx.strokeStyle = '#fbbf24'
    ctx.fillStyle = 'rgba(251, 191, 36, 0.2)'
    ctx.lineWidth = 2
    ctx.fillRect(x, y, viewportSize, viewportSize)
    ctx.strokeRect(x, y, viewportSize, viewportSize)

    // Draw center crosshair
    ctx.strokeStyle = '#fbbf24'
    ctx.lineWidth = 1
    const centerX = x + viewportSize / 2
    const centerY = y + viewportSize / 2
    ctx.beginPath()
    ctx.moveTo(centerX - 5, centerY)
    ctx.lineTo(centerX + 5, centerY)
    ctx.moveTo(centerX, centerY - 5)
    ctx.lineTo(centerX, centerY + 5)
    ctx.stroke()
  }

  const handleMouseDown = (e: React.MouseEvent) => {
    setIsDragging(true)
    handleClick(e)
  }

  const handleMouseMove = (e: React.MouseEvent) => {
    if (isDragging) {
      handleClick(e)
    }
  }

  const handleMouseUp = () => {
    setIsDragging(false)
  }

  const handleClick = (e: React.MouseEvent) => {
    const canvas = canvasRef.current
    if (!canvas) return

    const rect = canvas.getBoundingClientRect()
    const x = e.clientX - rect.left
    const y = e.clientY - rect.top

    // Convert click position to viewport coordinates
    const plotOffset = 10
    const plotSize = 130
    
    if (x < plotOffset || x > plotOffset + plotSize || 
        y < plotOffset || y > plotOffset + plotSize) {
      return
    }

    const relX = (x - plotOffset) / plotSize
    const relY = (y - plotOffset) / plotSize

    // Update viewport position (simplified calculation)
    onViewportChange({
      x: (relX - 0.5) * 1000,
      y: (relY - 0.5) * 1000
    })
  }

  if (isMinimized) {
    return (
      <div style={{
        position: 'absolute',
        top: '20px',
        right: '20px',
        width: '40px',
        height: '40px',
        background: 'rgba(0, 0, 0, 0.8)',
        border: '1px solid #555',
        borderRadius: '4px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: 'pointer',
        zIndex: 50
      }}
      onClick={() => setIsMinimized(false)}
      title="Show mini-map"
      >
        <Minimize2 size={16} color="#fff" />
      </div>
    )
  }

  return (
    <div style={{
      position: 'absolute',
      top: '20px',
      right: '20px',
      background: 'rgba(0, 0, 0, 0.9)',
      border: '1px solid #555',
      borderRadius: '4px',
      padding: '8px',
      zIndex: 50
    }}>
      {/* Header */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        marginBottom: '8px'
      }}>
        <span style={{
          color: '#fff',
          fontSize: '12px',
          fontWeight: '500'
        }}>
          Overview
        </span>
        <div style={{ display: 'flex', gap: '4px' }}>
          <button
            onClick={() => setIsMinimized(true)}
            style={{
              background: 'transparent',
              border: 'none',
              color: '#fff',
              cursor: 'pointer',
              padding: '2px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center'
            }}
            title="Minimize"
          >
            <Minimize2 size={12} />
          </button>
        </div>
      </div>

      {/* Canvas */}
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        style={{
          display: 'block',
          cursor: isDragging ? 'grabbing' : 'grab'
        }}
      />

      {/* Info */}
      <div style={{
        marginTop: '8px',
        fontSize: '10px',
        color: '#ccc'
      }}>
        <div>Zoom: {viewport.zoom.toFixed(2)}x</div>
        <div>Click to navigate</div>
      </div>
    </div>
  )
}

export default MiniMap