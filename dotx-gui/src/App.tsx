import React, { useState, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/tauri'
import { open } from '@tauri-apps/api/dialog'
import { save } from '@tauri-apps/api/dialog'
import { useHotkeys } from 'react-hotkeys-hook'
import TopBar from './components/TopBar'
import PlotCanvas from './components/PlotCanvas'
import StatusBar from './components/StatusBar'
import MiniMap from './components/MiniMap'
import { PlotConfig, ViewPort, PlotStats, StrandFilter, Theme } from './types'
import { listen } from '@tauri-apps/api/event'

interface AppState {
  plotConfig: PlotConfig
  viewport: ViewPort
  plotStats: PlotStats
  isLoading: boolean
  error: string | null
  showMiniMap: boolean
}

function App() {
  const [state, setState] = useState<AppState>({
    plotConfig: {
      showForwardStrand: true,
      showReverseStrand: true,
      swapAxes: false,
      reverseComplementY: false,
      theme: 'dark' as Theme,
      strandFilter: 'both' as StrandFilter
    },
    viewport: {
      x: 0,
      y: 0,
      zoom: 1,
      width: 1200,
      height: 800
    },
    plotStats: {
      anchorCount: 0,
      fps: 0,
      visibleAnchors: 0,
      verificationStatus: 'none'
    },
    isLoading: false,
    error: null,
    showMiniMap: true
  })

  // Keyboard shortcuts according to plan
  useHotkeys('plus', () => handleZoom(1.5), [state.viewport])
  useHotkeys('minus', () => handleZoom(0.75), [state.viewport])
  useHotkeys('w', () => handlePan(0, -50), [state.viewport])
  useHotkeys('a', () => handlePan(-50, 0), [state.viewport])
  useHotkeys('s', () => handlePan(0, 50), [state.viewport])
  useHotkeys('d', () => handlePan(50, 0), [state.viewport])
  useHotkeys('ArrowUp', () => handlePan(0, -50), [state.viewport])
  useHotkeys('ArrowLeft', () => handlePan(-50, 0), [state.viewport])
  useHotkeys('ArrowDown', () => handlePan(0, 50), [state.viewport])
  useHotkeys('ArrowRight', () => handlePan(50, 0), [state.viewport])
  useHotkeys('x', () => handleSwapAxes(), [state.plotConfig])
  useHotkeys('r', () => handleReverseComplementY(), [state.plotConfig])
  useHotkeys('1', () => handleStrandFilter('both'), [])
  useHotkeys('2', () => handleStrandFilter('forward'), [])
  useHotkeys('3', () => handleStrandFilter('reverse'), [])
  useHotkeys('0', () => handleResetView(), [])

  // Update viewport in backend when it changes
  useEffect(() => {
    invoke('update_viewport', {
      x: state.viewport.x,
      y: state.viewport.y,
      zoom: state.viewport.zoom
    }).catch(console.error)
  }, [state.viewport])

  // Periodically update plot statistics
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const stats = await invoke('get_plot_statistics') as PlotStats
        setState(prev => ({ ...prev, plotStats: stats }))
      } catch (error) {
        console.error('Failed to get plot statistics:', error)
      }
    }, 1000) // Update every second

    return () => clearInterval(interval)
  }, [])

  // Refresh stats on db-opened events (file drop or CLI --db)
  useEffect(() => {
    let unlisten: (() => void) | undefined
    listen<string>('db-opened', async (_evt) => {
      try {
        const stats = await invoke('get_plot_statistics') as PlotStats
        setState(prev => ({ ...prev, plotStats: stats, error: null }))
      } catch (e) {
        console.warn('get_plot_statistics after db-opened failed', e)
      }
    }).then(f => { unlisten = f })
    return () => { if (unlisten) unlisten() }
  }, [])

  const handleOpenFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'DOTx/FASTA/Alignment files',
          extensions: ['dotxdb', 'fa', 'fasta', 'fas', 'fna', 'paf', 'maf', 'sam', 'bam']
        }]
      })

      if (selected && Array.isArray(selected)) {
        setState(prev => ({ ...prev, isLoading: true, error: null }))
        // If a .dotxdb is selected, open it directly; else fall back to FASTA handler
        const first = selected[0] as string
        if (first.endsWith('.dotxdb')) {
          await invoke('open_db', { path: first })
        } else {
          await invoke('open_fasta_files', { paths: selected })
        }
        setState(prev => ({ ...prev, isLoading: false }))
      }
    } catch (error) {
      setState(prev => ({ 
        ...prev, 
        isLoading: false, 
        error: `Failed to open files: ${error}` 
      }))
    }
  }

  const handleExport = async (format: 'svg' | 'png' | 'pdf') => {
    try {
      const savePath = await save({
        filters: [{
          name: `${format.toUpperCase()} files`,
          extensions: [format]
        }]
      })

      if (savePath) {
        setState(prev => ({ ...prev, isLoading: true }))
        await invoke('export_plot', {
          path: savePath,
          format,
          width: state.viewport.width,
          height: state.viewport.height
        })
        setState(prev => ({ ...prev, isLoading: false }))
      }
    } catch (error) {
      setState(prev => ({ 
        ...prev, 
        isLoading: false, 
        error: `Export failed: ${error}` 
      }))
    }
  }

  const handleZoom = (factor: number) => {
    setState(prev => ({
      ...prev,
      viewport: {
        ...prev.viewport,
        zoom: Math.max(0.01, Math.min(100, prev.viewport.zoom * factor))
      }
    }))
  }

  const handlePan = (deltaX: number, deltaY: number) => {
    setState(prev => ({
      ...prev,
      viewport: {
        ...prev.viewport,
        x: prev.viewport.x + deltaX / prev.viewport.zoom,
        y: prev.viewport.y + deltaY / prev.viewport.zoom
      }
    }))
  }

  const handleSwapAxes = () => {
    setState(prev => ({
      ...prev,
      plotConfig: {
        ...prev.plotConfig,
        swapAxes: !prev.plotConfig.swapAxes
      }
    }))
  }

  const handleReverseComplementY = () => {
    setState(prev => ({
      ...prev,
      plotConfig: {
        ...prev.plotConfig,
        reverseComplementY: !prev.plotConfig.reverseComplementY
      }
    }))
  }

  const handleStrandFilter = (filter: StrandFilter) => {
    setState(prev => ({
      ...prev,
      plotConfig: {
        ...prev.plotConfig,
        strandFilter: filter,
        showForwardStrand: filter === 'both' || filter === 'forward',
        showReverseStrand: filter === 'both' || filter === 'reverse'
      }
    }))
  }

  const handleResetView = () => {
    setState(prev => ({
      ...prev,
      viewport: {
        ...prev.viewport,
        x: 0,
        y: 0,
        zoom: 1
      }
    }))
  }

  const handleThemeChange = (theme: Theme) => {
    setState(prev => ({
      ...prev,
      plotConfig: {
        ...prev.plotConfig,
        theme
      }
    }))
    
    // Update document theme
    document.documentElement.setAttribute('data-theme', theme)
  }

  const handleViewportChange = (viewport: Partial<ViewPort>) => {
    setState(prev => ({
      ...prev,
      viewport: {
        ...prev.viewport,
        ...viewport
      }
    }))
  }

  return (
    <div className="app" data-theme={state.plotConfig.theme}>
      <TopBar
        config={state.plotConfig}
        onOpenFiles={handleOpenFiles}
        onSwapAxes={handleSwapAxes}
        onReverseComplementY={handleReverseComplementY}
        onStrandFilter={handleStrandFilter}
        onThemeChange={handleThemeChange}
        onExport={handleExport}
        isLoading={state.isLoading}
      />
      
      <div className="main-content" style={{ 
        display: 'flex', 
        flex: 1,
        position: 'relative',
        overflow: 'hidden'
      }}>
        <PlotCanvas
          config={state.plotConfig}
          viewport={state.viewport}
          onViewportChange={handleViewportChange}
          stats={state.plotStats}
        />
        
        {state.showMiniMap && (
          <MiniMap
            viewport={state.viewport}
            onViewportChange={handleViewportChange}
          />
        )}
      </div>
      
      <StatusBar
        viewport={state.viewport}
        stats={state.plotStats}
        error={state.error}
        isLoading={state.isLoading}
      />
    </div>
  )
}

export default App
