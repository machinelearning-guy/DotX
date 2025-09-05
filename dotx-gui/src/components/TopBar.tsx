import React, { useState } from 'react'
import { 
  FolderOpen, 
  Settings2, 
  ArrowLeftRight, 
  FlipVertical,
  Sun,
  Moon,
  Download,
  Filter,
  ChevronDown
} from 'lucide-react'
import { invoke } from '@tauri-apps/api/tauri'
import { PlotConfig, StrandFilter, Theme, ExportFormat, PRESETS } from '../types'

interface TopBarProps {
  config: PlotConfig
  onOpenFiles: () => void
  onSwapAxes: () => void
  onReverseComplementY: () => void
  onStrandFilter: (filter: StrandFilter) => void
  onThemeChange: (theme: Theme) => void
  onExport: (format: ExportFormat) => void
  isLoading: boolean
}

const TopBar: React.FC<TopBarProps> = ({
  config,
  onOpenFiles,
  onSwapAxes,
  onReverseComplementY,
  onStrandFilter,
  onThemeChange,
  onExport,
  isLoading
}) => {
  const [showPresets, setShowPresets] = useState(false)
  const [showExportMenu, setShowExportMenu] = useState(false)
  const [showRecent, setShowRecent] = useState(false)
  const [recentFiles, setRecentFiles] = useState<string[]>([])

  const fetchRecent = async () => {
    try {
      const items = await invoke('get_recent_files') as string[]
      setRecentFiles(items)
    } catch (e) {
      console.warn('get_recent_files failed', e)
    }
  }

  const getStrandFilterLabel = (filter: StrandFilter) => {
    switch (filter) {
      case 'both': return 'Both strands'
      case 'forward': return 'Forward (+)'
      case 'reverse': return 'Reverse (-)'
    }
  }

  return (
    <div className="top-bar" style={{
      display: 'flex',
      alignItems: 'center',
      padding: '8px 16px',
      borderBottom: '1px solid #333',
      background: 'var(--bg-secondary)',
      gap: '8px',
      height: '48px',
      position: 'relative',
      zIndex: 100
    }}>
      {/* Open Files */}
      <button
        onClick={onOpenFiles}
        disabled={isLoading}
        title="Open FASTA/Alignment files"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '6px 12px',
          fontSize: '14px'
        }}
      >
        <FolderOpen size={16} />
        Open
      </button>

      {/* Recent files dropdown */}
      <div style={{ position: 'relative' }}>
        <button
          onClick={async () => { setShowRecent(!showRecent); if (!showRecent) { await fetchRecent() } }}
          disabled={isLoading}
          title="Recent files"
          style={{ display: 'flex', alignItems: 'center', gap: '4px', padding: '6px 8px' }}
        >
          <ChevronDown size={14} />
        </button>
        {showRecent && (
          <div style={{ position: 'absolute', top: '100%', left: 0, background: 'var(--bg-primary)', border: '1px solid #333', borderRadius: 4, minWidth: 280, zIndex: 1000 }}>
            {recentFiles.length === 0 && (
              <div style={{ padding: 8, fontSize: 12, opacity: 0.7 }}>No recent files</div>
            )}
            {recentFiles.map((p) => (
              <button key={p}
                onClick={async () => { await invoke('open_db', { path: p }); setShowRecent(false) }}
                style={{ display: 'block', width: '100%', textAlign: 'left', padding: 8, fontSize: 12, background: 'transparent', border: 'none' }}
                onMouseEnter={e => (e.currentTarget.style.background = 'rgba(255,255,255,0.08)')}
                onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
              >{p}</button>
            ))}
            {recentFiles.length > 0 && (
              <div style={{ borderTop: '1px solid #333' }}>
                <button onClick={async () => { await invoke('clear_recent_files'); setRecentFiles([]); setShowRecent(false) }}
                  style={{ display: 'block', width: '100%', textAlign: 'left', padding: 8, fontSize: 12, background: 'transparent', border: 'none', opacity: 0.8 }}>Clear recent</button>
              </div>
            )}
          </div>
        )}
      </div>

      <div style={{ width: '1px', height: '24px', background: '#333' }} />

      {/* Presets */}
      <div style={{ position: 'relative' }}>
        <button
          onClick={() => setShowPresets(!showPresets)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            padding: '6px 12px',
            fontSize: '14px'
          }}
        >
          <Settings2 size={16} />
          Presets
          <ChevronDown size={12} />
        </button>
        
        {showPresets && (
          <div style={{
            position: 'absolute',
            top: '100%',
            left: 0,
            background: 'var(--bg-primary)',
            border: '1px solid #333',
            borderRadius: '4px',
            padding: '4px',
            minWidth: '200px',
            zIndex: 1000
          }}>
            {PRESETS.map(preset => (
              <div
                key={preset.name}
                style={{
                  padding: '8px',
                  cursor: 'pointer',
                  borderRadius: '2px',
                  fontSize: '14px'
                }}
                onClick={() => {
                  console.log('Selected preset:', preset.name)
                  setShowPresets(false)
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)'
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'transparent'
                }}
              >
                <div style={{ fontWeight: '500' }}>{preset.name}</div>
                <div style={{ fontSize: '12px', opacity: 0.7 }}>
                  {preset.description}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div style={{ width: '1px', height: '24px', background: '#333' }} />

      {/* Strand Filter */}
      <div style={{ position: 'relative' }}>
        <button
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            padding: '6px 12px',
            fontSize: '14px'
          }}
        >
          <Filter size={16} />
          {getStrandFilterLabel(config.strandFilter)}
        </button>
        
        {/* Strand filter buttons */}
        <div style={{ display: 'flex', gap: '2px', marginLeft: '8px' }}>
          <button
            onClick={() => onStrandFilter('both')}
            title="Show both strands (1)"
            style={{
              padding: '4px 8px',
              fontSize: '12px',
              background: config.strandFilter === 'both' ? '#2563eb' : 'transparent',
              border: '1px solid #333',
              borderRadius: '2px'
            }}
          >
            1
          </button>
          <button
            onClick={() => onStrandFilter('forward')}
            title="Show forward strand only (2)"
            style={{
              padding: '4px 8px',
              fontSize: '12px',
              background: config.strandFilter === 'forward' ? '#2563eb' : 'transparent',
              border: '1px solid #333',
              borderRadius: '2px',
              color: '#3b82f6'
            }}
          >
            2
          </button>
          <button
            onClick={() => onStrandFilter('reverse')}
            title="Show reverse strand only (3)"
            style={{
              padding: '4px 8px',
              fontSize: '12px',
              background: config.strandFilter === 'reverse' ? '#dc2626' : 'transparent',
              border: '1px solid #333',
              borderRadius: '2px',
              color: '#ef4444'
            }}
          >
            3
          </button>
        </div>
      </div>

      <div style={{ width: '1px', height: '24px', background: '#333' }} />

      {/* Axis Controls */}
      <button
        onClick={onSwapAxes}
        title="Swap Axes (X)"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '6px 12px',
          fontSize: '14px',
          background: config.swapAxes ? '#2563eb' : 'transparent'
        }}
      >
        <ArrowLeftRight size={16} />
        Swap
      </button>

      <button
        onClick={onReverseComplementY}
        title="Reverse-Complement Y (R)"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '6px 12px',
          fontSize: '14px',
          background: config.reverseComplementY ? '#2563eb' : 'transparent'
        }}
      >
        <FlipVertical size={16} />
        RC-Y
      </button>

      <div style={{ width: '1px', height: '24px', background: '#333' }} />

      {/* Theme Toggle */}
      <button
        onClick={() => onThemeChange(config.theme === 'dark' ? 'light' : 'dark')}
        title="Toggle theme"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '6px 12px',
          fontSize: '14px'
        }}
      >
        {config.theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
        {config.theme === 'dark' ? 'Light' : 'Dark'}
      </button>

      {/* Spacer */}
      <div style={{ flex: 1 }} />

      {/* Export */}
      <div style={{ position: 'relative' }}>
        <button
          onClick={() => setShowExportMenu(!showExportMenu)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            padding: '6px 12px',
            fontSize: '14px'
          }}
        >
          <Download size={16} />
          Export
          <ChevronDown size={12} />
        </button>
        
        {showExportMenu && (
          <div style={{
            position: 'absolute',
            top: '100%',
            right: 0,
            background: 'var(--bg-primary)',
            border: '1px solid #333',
            borderRadius: '4px',
            padding: '4px',
            minWidth: '120px',
            zIndex: 1000
          }}>
            {(['svg', 'png', 'pdf'] as ExportFormat[]).map(format => (
              <button
                key={format}
                onClick={() => {
                  onExport(format)
                  setShowExportMenu(false)
                }}
                style={{
                  display: 'block',
                  width: '100%',
                  textAlign: 'left',
                  padding: '8px',
                  fontSize: '14px',
                  background: 'transparent',
                  border: 'none',
                  borderRadius: '2px'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)'
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'transparent'
                }}
              >
                {format.toUpperCase()}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Loading indicator */}
      {isLoading && (
        <div style={{
          position: 'absolute',
          right: '16px',
          top: '50%',
          transform: 'translateY(-50%)',
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          fontSize: '12px',
          opacity: 0.8
        }}>
          <div className="status-indicator processing" />
          Loading...
        </div>
      )}

      {/* Click outside handlers */}
      {(showPresets || showExportMenu || showRecent) && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            zIndex: 999
          }}
          onClick={() => {
            setShowPresets(false)
            setShowExportMenu(false)
            setShowRecent(false)
          }}
        />
      )}
    </div>
  )
}

export default TopBar
